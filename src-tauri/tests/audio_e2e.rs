//! E2E test: sends real audio to Azure OpenAI Realtime API and verifies text responses.
//!
//! Requires environment variables:
//!   AZURE_OPENAI_ENDPOINT - e.g. "https://dev-beme-ai.cognitiveservices.azure.com/"
//!   AZURE_OPENAI_API_KEY  - API key for the resource
//!   AZURE_OPENAI_AUDIO_DEPLOYMENT - e.g. "gpt-4o-realtime-preview"
//!
//! Run: cargo test --test audio_e2e -- --ignored

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

fn get_env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("Set {} env var to run this test", key))
}

#[tokio::test]
#[ignore] // Only run manually with --ignored flag
async fn realtime_api_responds_to_audio() {
    // 1. Load env vars
    let endpoint = get_env("AZURE_OPENAI_ENDPOINT");
    let api_key = get_env("AZURE_OPENAI_API_KEY");
    let deployment = get_env("AZURE_OPENAI_AUDIO_DEPLOYMENT");

    // 2. Build WebSocket URL (must use openai.azure.com domain)
    let parsed = url::Url::parse(&endpoint).expect("bad endpoint URL");
    let host = parsed.host_str().expect("no host").to_string();
    let ws_host = host.replace(".cognitiveservices.azure.com", ".openai.azure.com");
    let ws_url = format!(
        "wss://{ws_host}/openai/realtime?api-version=2025-04-01-preview&deployment={deployment}"
    );
    println!("Connecting to: {ws_url}");

    // 3. Connect
    let mut request = ws_url.into_client_request().expect("request build failed");
    request
        .headers_mut()
        .insert("api-key", api_key.parse().unwrap());

    let (ws_stream, response) = tokio_tungstenite::connect_async(request)
        .await
        .expect("WebSocket connect failed");
    println!("Connected! HTTP status: {}", response.status());

    let (mut sink, mut source) = ws_stream.split();

    // 4. Send session config
    let config = json!({
        "type": "session.update",
        "session": {
            "modalities": ["text"],
            "instructions": "You are listening to a conversation. Provide a brief summary of what was said.",
            "input_audio_format": "pcm16",
            "input_audio_transcription": { "model": "whisper-1" },
            "turn_detection": null
        }
    });
    sink.send(Message::Text(config.to_string().into()))
        .await
        .expect("send session config failed");
    println!("Session config sent");

    // 5. Load and send audio in 250ms chunks (matching app behavior)
    let pcm_data = std::fs::read("tests/fixtures/test-speech-24khz.pcm")
        .expect("Failed to read test audio fixture");
    println!("Loaded {} bytes of test audio", pcm_data.len());

    let samples_per_chunk = 24000 * 250 / 1000; // 6000 samples per 250ms
    let bytes_per_chunk = samples_per_chunk * 2; // 16-bit = 2 bytes per sample

    for chunk in pcm_data.chunks(bytes_per_chunk) {
        let b64 = BASE64.encode(chunk);
        let msg = json!({
            "type": "input_audio_buffer.append",
            "audio": b64
        });
        sink.send(Message::Text(msg.to_string().into()))
            .await
            .expect("send audio chunk failed");
    }
    println!("All audio chunks sent");

    // 6. Commit the audio buffer and explicitly request a response (no server VAD)
    let commit = json!({ "type": "input_audio_buffer.commit" });
    sink.send(Message::Text(commit.to_string().into()))
        .await
        .expect("send commit failed");
    println!("Audio buffer committed");

    let create_response = json!({ "type": "response.create" });
    sink.send(Message::Text(create_response.to_string().into()))
        .await
        .expect("send response.create failed");
    println!("Response requested");

    // 7. Read responses with timeout — collect deltas and wait for response.done
    let mut deltas = Vec::new();
    let mut got_response_done = false;
    let timeout = Duration::from_secs(30);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let msg = tokio::time::timeout_at(deadline, source.next()).await;
        match msg {
            Ok(Some(Ok(Message::Text(text)))) => {
                let v: Value = serde_json::from_str(&text).unwrap_or_default();
                let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match event_type {
                    "response.text.delta" | "response.audio_transcript.delta" => {
                        let delta = v.get("delta").and_then(|d| d.as_str()).unwrap_or("");
                        deltas.push(delta.to_string());
                        if deltas.len() <= 3 {
                            println!("Delta: {:?}", delta);
                        }
                    }
                    "response.done" => {
                        println!("Got response.done!");
                        got_response_done = true;
                        break;
                    }
                    "error" => {
                        let err_msg = v
                            .pointer("/error/message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown");
                        panic!("API error: {err_msg}");
                    }
                    _ => {
                        println!("Event: {event_type}");
                    }
                }
            }
            Ok(Some(Ok(_))) => {} // skip non-text
            Ok(Some(Err(e))) => panic!("WebSocket error: {e}"),
            Ok(None) => {
                println!("WebSocket closed");
                break;
            }
            Err(_) => {
                println!("Timeout after {timeout:?}");
                break;
            }
        }
    }

    // 8. Assertions
    println!("Total deltas received: {}", deltas.len());
    let full_text: String = deltas.join("");
    println!("Full response: {full_text}");

    assert!(
        got_response_done,
        "Expected response.done event but didn't receive one"
    );
    assert!(
        !deltas.is_empty(),
        "Expected at least one text delta but got none"
    );
    assert!(!full_text.is_empty(), "Expected non-empty response text");

    // Close gracefully
    let _ = sink.send(Message::Close(None)).await;
}
