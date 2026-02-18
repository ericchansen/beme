#![allow(dead_code)]
use async_trait::async_trait;

pub mod azure_audio;
pub mod azure_vision;
pub mod types;
pub use types::*;

/// Trait for AI providers (Azure OpenAI, Gemini, etc.)
/// Each provider implements analyze_frame for vision and audio streaming.
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Analyze a screen capture frame and return a text suggestion.
    /// `frame_data` is base64-encoded JPEG.
    /// `system_prompt` is the user's configured prompt.
    /// Returns a stream of text chunks (for SSE/streaming responses).
    async fn analyze_frame(
        &self,
        frame_data: &str,
        system_prompt: &str,
    ) -> Result<Box<dyn TextStream>, AiError>;

    /// Start an audio streaming session.
    /// Returns a session handle for sending audio, plus a receiver for text responses.
    async fn start_audio_stream(
        &self,
        system_prompt: &str,
    ) -> Result<(Box<dyn AudioSession>, AudioResponseRx), AiError>;

    /// Provider name for logging/display
    fn name(&self) -> &str;
}
