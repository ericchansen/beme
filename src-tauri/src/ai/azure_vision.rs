use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use super::{AiError, AiProvider, AudioSession, ConversationEntry, Role, TextStream};

pub struct AzureVisionClient {
    endpoint: String,
    api_key: String,
    deployment: String,
    system_prompt: String,
    client: Client,
}

impl AzureVisionClient {
    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        deployment: impl Into<String>,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            deployment: deployment.into(),
            system_prompt: system_prompt.into(),
            client: Client::new(),
        }
    }

    fn build_request_body(
        &self,
        frame_data: &str,
        system_prompt: &str,
        context: &[ConversationEntry],
    ) -> Value {
        let mut messages = Vec::new();

        // System message
        messages.push(json!({
            "role": "system",
            "content": system_prompt
        }));

        // Context messages from conversation history
        for entry in context {
            let role = match entry.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
            };
            messages.push(json!({
                "role": role,
                "content": entry.content
            }));
        }

        // User message with base64 image
        messages.push(json!({
            "role": "user",
            "content": [
                { "type": "text", "text": "What do you see?" },
                {
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/jpeg;base64,{}", frame_data),
                        "detail": "low"
                    }
                }
            ]
        }));

        json!({
            "messages": messages,
            "stream": true,
            "max_tokens": 300
        })
    }
}

#[async_trait]
impl AiProvider for AzureVisionClient {
    async fn analyze_frame(
        &self,
        frame_data: &str,
        system_prompt: &str,
        context: &[ConversationEntry],
    ) -> Result<Box<dyn TextStream>, AiError> {
        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version=2024-10-21",
            self.endpoint.trim_end_matches('/'),
            self.deployment
        );

        let body = self.build_request_body(frame_data, system_prompt, context);

        let response = self
            .client
            .post(&url)
            .header("api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AiError::ConnectionError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read error body".into());

            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(AiError::AuthError(error_body));
            }
            if status.as_u16() == 429 {
                return Err(AiError::RateLimited {
                    retry_after_ms: 1000,
                });
            }
            return Err(AiError::ConnectionError(format!(
                "HTTP {}: {}",
                status, error_body
            )));
        }

        Ok(Box::new(SseTextStream::new(response)))
    }

    async fn start_audio_stream(
        &self,
        _system_prompt: &str,
    ) -> Result<Box<dyn AudioSession>, AiError> {
        Err(AiError::ModelError(
            "Audio streaming not supported by AzureVisionClient".into(),
        ))
    }

    fn name(&self) -> &str {
        "azure-openai-vision"
    }
}

/// Streaming SSE reader for Azure OpenAI Chat Completions
pub struct SseTextStream {
    buffer: String,
    done: bool,
    response: Option<reqwest::Response>,
}

impl SseTextStream {
    fn new(response: reqwest::Response) -> Self {
        Self {
            buffer: String::new(),
            done: false,
            response: Some(response),
        }
    }
}

/// Parse a single SSE `data:` payload and extract delta content.
fn parse_sse_data(data: &str) -> Option<Result<String, AiError>> {
    let trimmed = data.trim();
    if trimmed == "[DONE]" {
        return None;
    }

    let parsed: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            return Some(Err(AiError::InvalidResponse(format!(
                "Invalid JSON in SSE: {}",
                e
            ))));
        }
    };

    if let Some(content) = parsed
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"))
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
    {
        if content.is_empty() {
            // Empty content delta — skip
            return Some(Ok(String::new()));
        }
        Some(Ok(content.to_string()))
    } else {
        // Delta without content (e.g. role-only delta) — skip
        Some(Ok(String::new()))
    }
}

#[async_trait]
impl TextStream for SseTextStream {
    async fn next_chunk(&mut self) -> Option<Result<String, AiError>> {
        if self.done {
            return None;
        }

        loop {
            // Try to extract a complete line from the buffer
            if let Some(newline_pos) = self.buffer.find('\n') {
                let line = self.buffer[..newline_pos].trim_end_matches('\r').to_string();
                self.buffer = self.buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    match parse_sse_data(data) {
                        None => {
                            // [DONE]
                            self.done = true;
                            return None;
                        }
                        Some(Ok(text)) if text.is_empty() => continue,
                        Some(result) => return Some(result),
                    }
                }

                // Skip non-data SSE lines (comments, event:, id:, retry:)
                continue;
            }

            // Need more data from the network
            let response = match self.response.as_mut() {
                Some(r) => r,
                None => {
                    self.done = true;
                    return None;
                }
            };

            match response.chunk().await {
                Ok(Some(bytes)) => {
                    let text = String::from_utf8_lossy(&bytes);
                    self.buffer.push_str(&text);
                }
                Ok(None) => {
                    // Stream ended without [DONE]
                    self.done = true;
                    // Process any remaining buffer
                    if !self.buffer.trim().is_empty() {
                        let remaining = self.buffer.trim().to_string();
                        self.buffer.clear();
                        if let Some(data) = remaining.strip_prefix("data: ") {
                            return parse_sse_data(data);
                        }
                    }
                    return None;
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(AiError::ConnectionError(format!(
                        "Stream read error: {}",
                        e
                    ))));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_body_structure() {
        let client = AzureVisionClient::new(
            "https://test.openai.azure.com",
            "test-key",
            "gpt-4o",
            "default prompt",
        );

        let context = vec![ConversationEntry {
            role: Role::User,
            content: "previous question".into(),
            timestamp: "2024-01-01T00:00:00Z".into(),
            source: super::super::CaptureSource::Screen,
        }];

        let body = client.build_request_body("base64data", "You are helpful.", &context);

        // Verify top-level fields
        assert_eq!(body["stream"], json!(true));
        assert_eq!(body["max_tokens"], json!(300));

        // Verify messages array
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3); // system + 1 context + user with image

        // System message
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "You are helpful.");

        // Context message
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "previous question");

        // User message with image
        assert_eq!(messages[2]["role"], "user");
        let content = messages[2]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(
            content[1]["image_url"]["url"],
            "data:image/jpeg;base64,base64data"
        );
        assert_eq!(content[1]["image_url"]["detail"], "low");
    }

    #[test]
    fn test_request_body_empty_context() {
        let client = AzureVisionClient::new(
            "https://test.openai.azure.com",
            "test-key",
            "gpt-4o",
            "default prompt",
        );

        let body = client.build_request_body("img", "prompt", &[]);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2); // system + user with image
    }

    #[test]
    fn test_request_body_assistant_context() {
        let client = AzureVisionClient::new(
            "https://test.openai.azure.com",
            "test-key",
            "gpt-4o",
            "default prompt",
        );

        let context = vec![
            ConversationEntry {
                role: Role::User,
                content: "What's on screen?".into(),
                timestamp: "t1".into(),
                source: super::super::CaptureSource::Screen,
            },
            ConversationEntry {
                role: Role::Assistant,
                content: "I see a code editor.".into(),
                timestamp: "t2".into(),
                source: super::super::CaptureSource::Screen,
            },
        ];

        let body = client.build_request_body("img", "prompt", &context);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[2]["content"], "I see a code editor.");
    }

    #[test]
    fn test_parse_sse_data_content() {
        let data = r#"{"id":"123","choices":[{"delta":{"content":"Hello"},"index":0}]}"#;
        let result = parse_sse_data(data);
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), "Hello");
    }

    #[test]
    fn test_parse_sse_data_multi_word() {
        let data = r#"{"id":"123","choices":[{"delta":{"content":" world"},"index":0}]}"#;
        let result = parse_sse_data(data);
        assert_eq!(result.unwrap().unwrap(), " world");
    }

    #[test]
    fn test_parse_sse_data_done() {
        let result = parse_sse_data("[DONE]");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_data_role_only_delta() {
        let data = r#"{"id":"123","choices":[{"delta":{"role":"assistant"},"index":0}]}"#;
        let result = parse_sse_data(data);
        // Role-only delta returns empty string
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), "");
    }

    #[test]
    fn test_parse_sse_data_empty_content() {
        let data = r#"{"id":"123","choices":[{"delta":{"content":""},"index":0}]}"#;
        let result = parse_sse_data(data);
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), "");
    }

    #[test]
    fn test_parse_sse_data_invalid_json() {
        let data = "not valid json{{{";
        let result = parse_sse_data(data);
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn test_parse_sse_data_missing_choices() {
        let data = r#"{"id":"123"}"#;
        let result = parse_sse_data(data);
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), "");
    }

    #[test]
    fn test_client_name() {
        let client = AzureVisionClient::new("https://test.openai.azure.com", "k", "d", "p");
        assert_eq!(client.name(), "azure-openai-vision");
    }

    #[tokio::test]
    async fn test_sse_stream_parsing() {
        // Simulate SSE data in a buffer
        let sse_data = "data: {\"id\":\"1\",\"choices\":[{\"delta\":{\"content\":\"Hi\"},\"index\":0}]}\n\ndata: {\"id\":\"2\",\"choices\":[{\"delta\":{\"content\":\" there\"},\"index\":0}]}\n\ndata: [DONE]\n\n";

        // We can't easily mock reqwest::Response, so test parse_sse_data on each line
        let mut results = Vec::new();
        for line in sse_data.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                match parse_sse_data(data) {
                    None => break,
                    Some(Ok(text)) if !text.is_empty() => results.push(text),
                    _ => {}
                }
            }
        }
        assert_eq!(results, vec!["Hi", " there"]);
    }

    #[test]
    fn test_endpoint_url_construction() {
        let client = AzureVisionClient::new(
            "https://beme-foundry.openai.azure.com/",
            "key",
            "gpt-4o",
            "prompt",
        );
        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version=2024-10-21",
            client.endpoint.trim_end_matches('/'),
            client.deployment
        );
        assert_eq!(
            url,
            "https://beme-foundry.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-10-21"
        );
    }
}
