#![allow(dead_code)]
use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{
    Message,
    client::IntoClientRequest,
};
use url::Url;

use super::{AiError, AiProvider, AudioSession, ConversationEntry, TextStream};

/// Azure OpenAI Realtime API audio client (WebSocket).
pub struct AzureAudioClient {
    pub endpoint: String,
    pub api_key: String,
    pub deployment: String,
    pub system_prompt: String,
}

/// Live WebSocket session for bidirectional audio.
pub struct RealtimeAudioSession {
    sender: mpsc::Sender<Message>,
    receiver: mpsc::Receiver<Result<String, AiError>>,
    close_sender: Option<mpsc::Sender<()>>,
}

// ── helpers (also used by tests) ────────────────────────────────────

/// Build the session.update JSON payload.
fn build_session_config(system_prompt: &str) -> Value {
    json!({
        "type": "session.update",
        "session": {
            "modalities": ["text"],
            "instructions": system_prompt,
            "input_audio_format": "pcm16",
            "input_audio_transcription": { "model": "whisper-1" },
            "turn_detection": { "type": "server_vad" }
        }
    })
}

/// Build an `input_audio_buffer.append` message from raw PCM bytes.
fn build_audio_append(pcm: &[u8]) -> Value {
    json!({
        "type": "input_audio_buffer.append",
        "audio": BASE64.encode(pcm)
    })
}

/// Parse a single server-sent event and return:
///   Ok(Some(text))  – a text delta to forward
///   Ok(None)        – event handled but nothing to emit (skip / done)
///   Err(e)          – an error event
fn parse_event(text: &str) -> Result<Option<String>, AiError> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| AiError::InvalidResponse(format!("bad JSON: {e}")))?;

    match v.get("type").and_then(|t| t.as_str()) {
        Some("response.text.delta") => {
            let delta = v
                .get("delta")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            Ok(Some(delta))
        }
        Some("response.text.done") | Some("response.done") => Ok(None),
        Some("error") => {
            let msg = v
                .pointer("/error/message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            let code = v
                .pointer("/error/code")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            Err(AiError::ModelError(format!("[{code}] {msg}")))
        }
        _ => Ok(None), // skip unknown events
    }
}

// ── AiProvider impl ─────────────────────────────────────────────────

#[async_trait]
impl AiProvider for AzureAudioClient {
    async fn analyze_frame(
        &self,
        _frame_data: &str,
        _system_prompt: &str,
        _context: &[ConversationEntry],
    ) -> Result<Box<dyn TextStream>, AiError> {
        Err(AiError::ModelError(
            "AzureAudioClient does not support vision analysis".into(),
        ))
    }

    async fn start_audio_stream(
        &self,
        system_prompt: &str,
    ) -> Result<Box<dyn AudioSession>, AiError> {
        // Build wss URL
        let host = Url::parse(&self.endpoint)
            .map_err(|e| AiError::ConnectionError(format!("bad endpoint URL: {e}")))?
            .host_str()
            .ok_or_else(|| AiError::ConnectionError("no host in endpoint URL".into()))?
            .to_string();

        let ws_url = format!(
            "wss://{host}/openai/realtime?api-version=2025-04-01-preview&deployment={deployment}",
            deployment = self.deployment,
        );

        let mut request = ws_url
            .into_client_request()
            .map_err(|e| AiError::ConnectionError(format!("request build: {e}")))?;
        request
            .headers_mut()
            .insert("api-key", self.api_key.parse().map_err(|e| {
                AiError::AuthError(format!("invalid api-key header value: {e}"))
            })?);

        let (ws_stream, _response) =
            tokio_tungstenite::connect_async(request)
                .await
                .map_err(|e| AiError::ConnectionError(format!("WebSocket connect: {e}")))?;

        let (mut ws_sink, mut ws_source) = ws_stream.split();

        // Send session config
        let config = build_session_config(system_prompt);
        ws_sink
            .send(Message::Text(config.to_string().into()))
            .await
            .map_err(|e| AiError::ConnectionError(format!("send session config: {e}")))?;

        // Channel: caller → WebSocket sink
        let (send_tx, mut send_rx) = mpsc::channel::<Message>(64);
        // Channel: parsed events → caller
        let (resp_tx, resp_rx) = mpsc::channel::<Result<String, AiError>>(64);
        // Channel: close signal
        let (close_tx, mut close_rx) = mpsc::channel::<()>(1);

        // Writer task
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = send_rx.recv() => {
                        if ws_sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                    _ = close_rx.recv() => {
                        let _ = ws_sink.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
        });

        // Reader task
        tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_source.next().await {
                if let Message::Text(text) = msg {
                    match parse_event(&text) {
                        Ok(Some(delta)) => {
                            if resp_tx.send(Ok(delta)).await.is_err() {
                                break;
                            }
                        }
                        Ok(None) => { /* skip */ }
                        Err(e) => {
                            let _ = resp_tx.send(Err(e)).await;
                            break;
                        }
                    }
                }
            }
        });

        Ok(Box::new(RealtimeAudioSession {
            sender: send_tx,
            receiver: resp_rx,
            close_sender: Some(close_tx),
        }))
    }

    fn name(&self) -> &str {
        "azure-realtime-audio"
    }
}

// ── AudioSession impl ───────────────────────────────────────────────

#[async_trait]
impl AudioSession for RealtimeAudioSession {
    async fn send_audio(&mut self, audio_data: &[u8]) -> Result<(), AiError> {
        let payload = build_audio_append(audio_data);
        self.sender
            .send(Message::Text(payload.to_string().into()))
            .await
            .map_err(|e| AiError::ConnectionError(format!("send audio: {e}")))
    }

    async fn next_response(&mut self) -> Option<Result<String, AiError>> {
        self.receiver.recv().await
    }

    async fn close(&mut self) -> Result<(), AiError> {
        if let Some(tx) = self.close_sender.take() {
            let _ = tx.send(()).await;
        }
        Ok(())
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_config_json_structure() {
        let cfg = build_session_config("You are a helpful assistant.");
        assert_eq!(cfg["type"], "session.update");
        let session = &cfg["session"];
        assert_eq!(session["modalities"][0], "text");
        assert_eq!(session["instructions"], "You are a helpful assistant.");
        assert_eq!(session["input_audio_format"], "pcm16");
        assert_eq!(
            session["input_audio_transcription"]["model"],
            "whisper-1"
        );
        assert_eq!(session["turn_detection"]["type"], "server_vad");
    }

    #[test]
    fn audio_append_message_construction() {
        let pcm: &[u8] = &[0x01, 0x02, 0xFF, 0x00];
        let msg = build_audio_append(pcm);
        assert_eq!(msg["type"], "input_audio_buffer.append");
        let audio_b64 = msg["audio"].as_str().unwrap();
        let decoded = BASE64.decode(audio_b64).unwrap();
        assert_eq!(decoded, pcm);
    }

    #[test]
    fn parse_text_delta_event() {
        let event = r#"{"type":"response.text.delta","delta":"Hello"}"#;
        let result = parse_event(event).unwrap();
        assert_eq!(result, Some("Hello".to_string()));
    }

    #[test]
    fn parse_text_done_event() {
        let event = r#"{"type":"response.text.done","text":"Hello world"}"#;
        let result = parse_event(event).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn parse_response_done_event() {
        let event = r#"{"type":"response.done","response":{}}"#;
        let result = parse_event(event).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn parse_error_event() {
        let event =
            r#"{"type":"error","error":{"message":"rate limit exceeded","code":"rate_limit"}}"#;
        let result = parse_event(event);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            AiError::ModelError(msg) => {
                assert!(msg.contains("rate limit exceeded"));
                assert!(msg.contains("rate_limit"));
            }
            other => panic!("expected ModelError, got: {other:?}"),
        }
    }

    #[test]
    fn parse_unknown_event_is_skipped() {
        let event = r#"{"type":"session.created","session":{"id":"abc"}}"#;
        let result = parse_event(event).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let result = parse_event("not json");
        assert!(result.is_err());
        match result.unwrap_err() {
            AiError::InvalidResponse(msg) => assert!(msg.contains("bad JSON")),
            other => panic!("expected InvalidResponse, got: {other:?}"),
        }
    }
}
