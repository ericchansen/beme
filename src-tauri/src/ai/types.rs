#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Type alias for the channel that delivers parsed text responses from an audio session.
pub type AudioResponseRx = mpsc::Receiver<Result<String, AiError>>;

/// A previous interaction for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub role: Role,
    pub content: String,
    pub timestamp: String,
    pub source: CaptureSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureSource {
    Screen,
    Audio,
}

/// Error type for AI operations
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("Connection failed: {0}")]
    ConnectionError(String),
    #[error("Authentication failed: {0}")]
    AuthError(String),
    #[error("Rate limited — retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
    #[error("Model error: {0}")]
    ModelError(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Trait for streaming text responses (chunk by chunk)
#[async_trait::async_trait]
pub trait TextStream: Send {
    /// Get the next text chunk. Returns None when the stream is complete.
    async fn next_chunk(&mut self) -> Option<Result<String, AiError>>;
}

/// Trait for bidirectional audio sessions
#[async_trait::async_trait]
pub trait AudioSession: Send {
    /// Send an audio chunk (raw PCM bytes) to the AI
    async fn send_audio(&mut self, audio_data: &[u8]) -> Result<(), AiError>;

    /// Close the audio session
    async fn close(&mut self) -> Result<(), AiError>;
}

/// Configuration for an AI provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub endpoint: String,
    pub api_key: String,
    pub vision_deployment: String,
    pub audio_deployment: String,
    pub vision_prompt: String,
    pub audio_prompt: String,
}
