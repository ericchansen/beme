// stream_manager.rs — Orchestrates capture → AI → suggestion pipeline.
//
// When capture starts, frames are sent to the configured AI provider.
// AI responses are streamed back as `ai:suggestion` Tauri events.

use crate::ai::azure_audio::AzureAudioClient;
use crate::ai::azure_vision::AzureVisionClient;
use crate::ai::{AiProvider, AudioSession};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as TokioMutex;

/// Payload emitted on `ai:suggestion` events.
#[derive(Clone, Serialize)]
pub struct SuggestionPayload {
    pub text: String,
    pub timestamp: String,
    pub done: bool,
    pub id: u64,
    pub source: String,
}

/// Payload emitted on `ai:error` events.
#[derive(Clone, Serialize)]
pub struct AiErrorPayload {
    pub message: String,
    pub timestamp: String,
}

/// Payload emitted on `ai:audio-status` events.
#[derive(Clone, Serialize)]
pub struct AudioStatusPayload {
    pub status: String,  // "connecting", "connected", "disconnected", "error"
    pub message: Option<String>,
}

/// Shared state for the AI pipeline.
pub struct StreamManager {
    provider: Mutex<Option<Arc<dyn AiProvider>>>,
    system_prompt: Mutex<String>,
    next_id: Arc<Mutex<u64>>,
    // Audio pipeline
    audio_provider: Mutex<Option<Arc<dyn AiProvider>>>,
    audio_session: Arc<TokioMutex<Option<Box<dyn AudioSession>>>>,
    audio_prompt: Mutex<String>,
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            provider: Mutex::new(None),
            system_prompt: Mutex::new(String::new()),
            next_id: Arc::new(Mutex::new(1)),
            audio_provider: Mutex::new(None),
            audio_session: Arc::new(TokioMutex::new(None)),
            audio_prompt: Mutex::new(String::new()),
        }
    }

    /// Configure the AI provider with Azure OpenAI credentials.
    pub fn configure_azure(
        &self,
        endpoint: &str,
        api_key: &str,
        deployment: &str,
        system_prompt: &str,
        use_bearer: bool,
    ) {
        let mut client = AzureVisionClient::new(endpoint, api_key, deployment, system_prompt);
        if use_bearer {
            client = client.with_bearer();
        }
        *self.provider.lock().unwrap() = Some(Arc::new(client));
        *self.system_prompt.lock().unwrap() = system_prompt.to_string();
        log::info!("StreamManager: Azure vision provider configured (bearer={})", use_bearer);
    }

    /// Check if a provider is configured.
    pub fn is_configured(&self) -> bool {
        self.provider.lock().unwrap().is_some()
    }

    /// Configure the audio AI provider with Azure OpenAI Realtime credentials.
    pub fn configure_audio(
        &self,
        endpoint: &str,
        api_key: &str,
        deployment: &str,
        system_prompt: &str,
    ) {
        let client = AzureAudioClient {
            endpoint: endpoint.to_string(),
            api_key: api_key.to_string(),
            deployment: deployment.to_string(),
            system_prompt: system_prompt.to_string(),
        };
        *self.audio_provider.lock().unwrap() = Some(Arc::new(client));
        *self.audio_prompt.lock().unwrap() = system_prompt.to_string();
        log::info!("StreamManager: Azure audio provider configured");
    }

    /// Start the audio AI WebSocket session and spawn a reader task.
    pub async fn start_audio_session(&self, app_handle: AppHandle) -> Result<(), String> {
        log::info!("Starting audio AI session...");
        let provider = {
            let p = self.audio_provider.lock().unwrap();
            match p.as_ref() {
                Some(p) => Arc::clone(p),
                None => return Err("Audio provider not configured".into()),
            }
        };
        let prompt = self.audio_prompt.lock().unwrap().clone();

        emit_audio_status(&app_handle, "connecting", None);

        let (session, audio_rx) = provider
            .start_audio_stream(&prompt)
            .await
            .map_err(|e| format!("Failed to start audio session: {}", e))?;

        {
            let mut sess = self.audio_session.lock().await;
            *sess = Some(session);
        }

        emit_audio_status(&app_handle, "connected", None);

        // Spawn reader task — owns audio_rx directly, no mutex needed
        let next_id = Arc::clone(&self.next_id);
        let reader_app_handle = app_handle.clone();

        tokio::spawn(async move {
            let mut audio_rx = audio_rx;
            // Allocate one suggestion ID per response turn
            let mut suggestion_id = {
                let mut id = next_id.lock().unwrap();
                let current = *id;
                *id += 1;
                current
            };
            let mut is_first_response = true;
            loop {
                match audio_rx.recv().await {
                    Some(Ok(text)) if text.is_empty() => {
                        // Empty string = turn done signal
                        let payload = SuggestionPayload {
                            text: String::new(),
                            timestamp: now_iso(),
                            done: true,
                            id: suggestion_id,
                            source: "audio".into(),
                        };
                        log_event_for_testing("ai:suggestion", &payload);
                        let _ = reader_app_handle.emit("ai:suggestion", payload);
                        // Allocate a new ID for the next turn
                        suggestion_id = {
                            let mut id = next_id.lock().unwrap();
                            let current = *id;
                            *id += 1;
                            current
                        };
                    }
                    Some(Ok(text)) => {
                        if is_first_response {
                            log::info!("Audio AI: first response delta received");
                            is_first_response = false;
                        }
                        let payload = SuggestionPayload {
                            text,
                            timestamp: now_iso(),
                            done: false,
                            id: suggestion_id,
                            source: "audio".into(),
                        };
                        log_event_for_testing("ai:suggestion", &payload);
                        let _ = reader_app_handle.emit("ai:suggestion", payload);
                    }
                    Some(Err(e)) => {
                        log::error!("Audio AI error: {}", e);
                        let _ = reader_app_handle.emit(
                            "ai:error",
                            AiErrorPayload {
                                message: e.to_string(),
                                timestamp: now_iso(),
                            },
                        );
                        emit_audio_status(&reader_app_handle, "error", Some(e.to_string()));
                        break;
                    }
                    None => break,
                }
            }
            emit_audio_status(&reader_app_handle, "disconnected", None);
            log::info!("Audio AI reader task ended");
        });

        log::info!("Audio AI session started");
        Ok(())
    }

    /// Send a chunk of audio PCM data to the active AI session.
    pub async fn process_audio_chunk(&self, audio_data: &[u8]) -> Result<(), String> {
        let mut sess = self.audio_session.lock().await;
        match sess.as_mut() {
            Some(s) => s.send_audio(audio_data).await.map_err(|e| e.to_string()),
            None => Err("No active audio session".into()),
        }
    }

    /// Check if an audio session is active.
    pub async fn has_audio_session(&self) -> bool {
        self.audio_session.lock().await.is_some()
    }

    /// Inject a pre-built audio session (for testing without AppHandle).
    pub async fn inject_audio_session(&self, session: Box<dyn AudioSession>) {
        *self.audio_session.lock().await = Some(session);
    }

    /// Remove the active audio session without emitting events.
    pub async fn clear_audio_session(&self) {
        *self.audio_session.lock().await = None;
    }

    /// Get both prompts.
    pub fn get_prompts(&self) -> (String, String) {
        let vision = self.system_prompt.lock().unwrap().clone();
        let audio = self.audio_prompt.lock().unwrap().clone();
        (vision, audio)
    }

    /// Update a prompt. Source is "vision" or "audio".
    pub fn update_prompt(&self, source: &str, text: &str) {
        match source {
            "vision" => {
                *self.system_prompt.lock().unwrap() = text.to_string();
                log::info!("Vision prompt updated ({} chars)", text.len());
            }
            "audio" => {
                *self.audio_prompt.lock().unwrap() = text.to_string();
                log::info!("Audio prompt updated ({} chars)", text.len());
            }
            _ => log::warn!("Unknown prompt source: {}", source),
        }
    }

    /// Close the audio AI WebSocket session.
    pub async fn stop_audio_session(&self, app_handle: &AppHandle) -> Result<(), String> {
        let mut sess = self.audio_session.lock().await;
        if let Some(ref mut s) = *sess {
            s.close().await.map_err(|e| e.to_string())?;
        }
        *sess = None;
        emit_audio_status(app_handle, "disconnected", None);
        log::info!("Audio AI session stopped");
        Ok(())
    }

    /// Analyze a frame and emit streaming suggestions.
    /// Called from the capture loop when a new frame is available.
    pub fn analyze_frame(&self, frame_data: String, app_handle: AppHandle) {
        let provider = {
            let p = self.provider.lock().unwrap();
            match p.as_ref() {
                Some(p) => Arc::clone(p),
                None => return,
            }
        };

        let system_prompt = self.system_prompt.lock().unwrap().clone();

        let suggestion_id = {
            let mut id = self.next_id.lock().unwrap();
            let current = *id;
            *id += 1;
            current
        };

        tokio::spawn(async move {
            match provider
                .analyze_frame(&frame_data, &system_prompt)
                .await
            {
                Ok(mut stream) => {
                    while let Some(chunk_result) = stream.next_chunk().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let payload = SuggestionPayload {
                                    text: chunk,
                                    timestamp: now_iso(),
                                    done: false,
                                    id: suggestion_id,
                                    source: "screen".into(),
                                };
                                log_event_for_testing("ai:suggestion", &payload);
                                let _ = app_handle.emit("ai:suggestion", payload);
                            }
                            Err(e) => {
                                log::error!("AI stream error: {}", e);
                                let _ = app_handle.emit(
                                    "ai:error",
                                    AiErrorPayload {
                                        message: e.to_string(),
                                        timestamp: now_iso(),
                                    },
                                );
                                break;
                            }
                        }
                    }

                    // Final done event
                    let payload = SuggestionPayload {
                        text: String::new(),
                        timestamp: now_iso(),
                        done: true,
                        id: suggestion_id,
                        source: "screen".into(),
                    };
                    log_event_for_testing("ai:suggestion", &payload);
                    let _ = app_handle.emit("ai:suggestion", payload);
                }
                Err(e) => {
                    log::error!("AI analyze_frame error: {}", e);
                    let _ = app_handle.emit(
                        "ai:error",
                        AiErrorPayload {
                            message: e.to_string(),
                            timestamp: now_iso(),
                        },
                    );
                }
            }
        });
    }
}

/// When `BEME_TEST_LOG` is set, append the event as a JSONL line to the specified file.
/// No-op when the env var is absent — zero overhead in production.
fn log_event_for_testing(event_name: &str, payload: &SuggestionPayload) {
    if let Ok(path) = std::env::var("BEME_TEST_LOG") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let json = serde_json::to_string(payload).unwrap_or_default();
            let _ = writeln!(
                file,
                r#"{{"event":"{}","timestamp":"{}","payload":{}}}"#,
                event_name,
                payload.timestamp,
                json
            );
        }
    }
}

fn emit_audio_status(app_handle: &AppHandle, status: &str, message: Option<String>) {
    let _ = app_handle.emit("ai:audio-status", AudioStatusPayload {
        status: status.to_string(),
        message,
    });
}

fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let days = secs / 86400;
    let t = secs % 86400;
    let (y, m, d) = crate::capture::screen::epoch_days_to_ymd(days as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        t / 3600,
        (t % 3600) / 60,
        t % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_manager_starts_unconfigured() {
        let sm = StreamManager::new();
        assert!(!sm.is_configured());
    }

    #[test]
    fn configure_azure_sets_provider() {
        let sm = StreamManager::new();
        sm.configure_azure(
            "https://test.openai.azure.com",
            "test-key",
            "gpt-4o",
            "You are helpful.",
            false,
        );
        assert!(sm.is_configured());
    }

    #[test]
    fn suggestion_id_increments() {
        let sm = StreamManager::new();
        let id1 = {
            let mut id = sm.next_id.lock().unwrap();
            let v = *id;
            *id += 1;
            v
        };
        let id2 = {
            let mut id = sm.next_id.lock().unwrap();
            let v = *id;
            *id += 1;
            v
        };
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn now_iso_format() {
        let ts = now_iso();
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
    }

}
