// stream_manager.rs — Orchestrates capture → AI → suggestion pipeline.
//
// When capture starts, frames are sent to the configured AI provider.
// AI responses are streamed back as `ai:suggestion` Tauri events.

use crate::ai::azure_vision::AzureVisionClient;
use crate::ai::{AiProvider, CaptureSource, ConversationEntry, Role};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

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

/// Shared state for the AI pipeline.
pub struct StreamManager {
    provider: Mutex<Option<Arc<dyn AiProvider>>>,
    system_prompt: Mutex<String>,
    context: Arc<Mutex<Vec<ConversationEntry>>>,
    max_context: usize,
    next_id: Arc<Mutex<u64>>,
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
            context: Arc::new(Mutex::new(Vec::new())),
            max_context: 3,
            next_id: Arc::new(Mutex::new(1)),
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
        self.context.lock().unwrap().clear();
        log::info!("StreamManager: Azure vision provider configured (bearer={})", use_bearer);
    }

    /// Check if a provider is configured.
    pub fn is_configured(&self) -> bool {
        self.provider.lock().unwrap().is_some()
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
        let context = self.context.lock().unwrap().clone();

        let suggestion_id = {
            let mut id = self.next_id.lock().unwrap();
            let current = *id;
            *id += 1;
            current
        };

        let context_ref = Arc::clone(&self.context);
        let max_context = self.max_context;

        tokio::spawn(async move {
            match provider
                .analyze_frame(&frame_data, &system_prompt, &context)
                .await
            {
                Ok(mut stream) => {
                    let mut full_text = String::new();

                    while let Some(chunk_result) = stream.next_chunk().await {
                        match chunk_result {
                            Ok(chunk) => {
                                full_text.push_str(&chunk);
                                let _ = app_handle.emit(
                                    "ai:suggestion",
                                    SuggestionPayload {
                                        text: chunk,
                                        timestamp: now_iso(),
                                        done: false,
                                        id: suggestion_id,
                                        source: "screen".into(),
                                    },
                                );
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
                    let _ = app_handle.emit(
                        "ai:suggestion",
                        SuggestionPayload {
                            text: String::new(),
                            timestamp: now_iso(),
                            done: true,
                            id: suggestion_id,
                            source: "screen".into(),
                        },
                    );

                    // Update rolling context
                    if !full_text.is_empty() {
                        if let Ok(mut ctx) = context_ref.lock() {
                            ctx.push(ConversationEntry {
                                role: Role::User,
                                content: "What do you see?".into(),
                                timestamp: now_iso(),
                                source: CaptureSource::Screen,
                            });
                            ctx.push(ConversationEntry {
                                role: Role::Assistant,
                                content: full_text,
                                timestamp: now_iso(),
                                source: CaptureSource::Screen,
                            });
                            while ctx.len() > max_context * 2 {
                                ctx.remove(0);
                            }
                        }
                    }
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

    #[test]
    fn rolling_context_caps_at_max() {
        let sm = StreamManager::new();
        {
            let mut ctx = sm.context.lock().unwrap();
            // Push 4 pairs (8 entries) — should trim to max_context*2 = 6
            for i in 0..4 {
                ctx.push(ConversationEntry {
                    role: Role::User,
                    content: format!("user-{i}"),
                    timestamp: now_iso(),
                    source: CaptureSource::Screen,
                });
                ctx.push(ConversationEntry {
                    role: Role::Assistant,
                    content: format!("assistant-{i}"),
                    timestamp: now_iso(),
                    source: CaptureSource::Screen,
                });
            }
            while ctx.len() > sm.max_context * 2 {
                ctx.remove(0);
            }
        }
        let ctx = sm.context.lock().unwrap();
        assert_eq!(ctx.len(), 6);
        // Oldest pair (user-0/assistant-0) should be evicted
        assert_eq!(ctx[0].content, "user-1");
        assert_eq!(ctx[1].content, "assistant-1");
    }

    #[test]
    fn configure_azure_clears_context() {
        let sm = StreamManager::new();
        // Seed some context
        {
            let mut ctx = sm.context.lock().unwrap();
            ctx.push(ConversationEntry {
                role: Role::User,
                content: "old".into(),
                timestamp: now_iso(),
                source: CaptureSource::Screen,
            });
        }
        assert_eq!(sm.context.lock().unwrap().len(), 1);

        // Reconfigure — context should be cleared
        sm.configure_azure(
            "https://test.openai.azure.com",
            "key",
            "gpt-4o",
            "prompt",
            false,
        );
        assert_eq!(sm.context.lock().unwrap().len(), 0);
    }
}
