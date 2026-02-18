#![allow(dead_code)]
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

use super::{AiError, AiProvider, AudioResponseRx, AudioSession, TextStream};

pub struct AzureVisionClient {
    endpoint: String,
    api_key: String,
    model: String,
    system_prompt: String,
    client: Client,
    /// When true, use `Authorization: Bearer` instead of `api-key` header.
    use_bearer: bool,
    previous_response_id: Arc<Mutex<Option<String>>>,
}

impl AzureVisionClient {
    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
            system_prompt: system_prompt.into(),
            client: Client::new(),
            use_bearer: false,
            previous_response_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a client that uses Bearer token auth (for Entra ID / AAD).
    pub fn with_bearer(mut self) -> Self {
        self.use_bearer = true;
        self
    }

    fn build_request_body(&self, frame_data: &str, system_prompt: &str) -> Value {
        let previous_id = self.previous_response_id.lock().unwrap().clone();

        let mut body = json!({
            "model": self.model,
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        { "type": "input_text", "text": "What do you see?" },
                        { "type": "input_image", "image_url": format!("data:image/jpeg;base64,{}", frame_data) }
                    ]
                }
            ],
            "instructions": system_prompt,
            "stream": true,
            "max_output_tokens": 300,
            "truncation": "auto"
        });

        if let Some(prev_id) = previous_id {
            body.as_object_mut()
                .unwrap()
                .insert("previous_response_id".into(), json!(prev_id));
        }

        body
    }
}

#[async_trait]
impl AiProvider for AzureVisionClient {
    async fn analyze_frame(
        &self,
        frame_data: &str,
        system_prompt: &str,
    ) -> Result<Box<dyn TextStream>, AiError> {
        let url = format!(
            "{}/openai/v1/responses?api-version=preview",
            self.endpoint.trim_end_matches('/'),
        );

        let body = self.build_request_body(frame_data, system_prompt);

        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        req = if self.use_bearer {
            req.header("Authorization", format!("Bearer {}", self.api_key))
        } else {
            req.header("api-key", &self.api_key)
        };

        let response = req
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

            // If the previous_response_id is stale/expired, clear it and retry once
            if status.as_u16() == 400 && error_body.contains("previous_response_not_found") {
                log::warn!("Stale previous_response_id detected, clearing and retrying");
                *self.previous_response_id.lock().unwrap() = None;
                let retry_body = self.build_request_body(frame_data, system_prompt);
                let mut retry_req = self
                    .client
                    .post(&url)
                    .header("Content-Type", "application/json");
                retry_req = if self.use_bearer {
                    retry_req.header("Authorization", format!("Bearer {}", self.api_key))
                } else {
                    retry_req.header("api-key", &self.api_key)
                };
                let retry_response = retry_req
                    .json(&retry_body)
                    .send()
                    .await
                    .map_err(|e| AiError::ConnectionError(e.to_string()))?;
                let retry_status = retry_response.status();
                if !retry_status.is_success() {
                    let retry_error = retry_response.text().await.unwrap_or_default();
                    return Err(AiError::ConnectionError(format!(
                        "HTTP {}: {}", retry_status, retry_error
                    )));
                }
                return Ok(Box::new(ResponsesTextStream::new(
                    retry_response,
                    Arc::clone(&self.previous_response_id),
                )));
            }

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

        Ok(Box::new(ResponsesTextStream::new(
            response,
            Arc::clone(&self.previous_response_id),
        )))
    }

    async fn start_audio_stream(
        &self,
        _system_prompt: &str,
    ) -> Result<(Box<dyn AudioSession>, AudioResponseRx), AiError> {
        Err(AiError::ModelError(
            "Audio streaming not supported by AzureVisionClient".into(),
        ))
    }

    fn name(&self) -> &str {
        "azure-openai-vision"
    }
}

/// Streaming SSE reader for Azure OpenAI Responses API
pub struct ResponsesTextStream {
    buffer: String,
    done: bool,
    response: Option<reqwest::Response>,
    previous_response_id: Arc<Mutex<Option<String>>>,
}

impl ResponsesTextStream {
    fn new(response: reqwest::Response, previous_response_id: Arc<Mutex<Option<String>>>) -> Self {
        Self {
            buffer: String::new(),
            done: false,
            response: Some(response),
            previous_response_id,
        }
    }
}

/// Parse a single SSE `data:` payload from the Responses API.
/// Returns:
///   `ParseResult::Delta(text)` — a text chunk to emit
///   `ParseResult::ResponseId(id)` — capture the response ID
///   `ParseResult::Done` — stream finished
///   `ParseResult::Skip` — skip this event
///   `ParseResult::Error(e)` — parse error
enum ParseResult {
    Delta(String),
    ResponseId(String),
    Done,
    Skip,
    Error(AiError),
}

fn parse_sse_data(data: &str) -> ParseResult {
    let trimmed = data.trim();
    if trimmed == "[DONE]" {
        return ParseResult::Done;
    }

    let parsed: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            return ParseResult::Error(AiError::InvalidResponse(format!(
                "Invalid JSON in SSE: {}",
                e
            )));
        }
    };

    let event_type = parsed.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match event_type {
        "response.output_text.delta" => {
            let delta = parsed
                .get("delta")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            if delta.is_empty() {
                ParseResult::Skip
            } else {
                ParseResult::Delta(delta.to_string())
            }
        }
        "response.created" => {
            if let Some(id) = parsed
                .pointer("/response/id")
                .and_then(|v| v.as_str())
            {
                ParseResult::ResponseId(id.to_string())
            } else {
                ParseResult::Skip
            }
        }
        "response.output_text.done" | "response.completed" => ParseResult::Done,
        _ => ParseResult::Skip,
    }
}

#[async_trait]
impl TextStream for ResponsesTextStream {
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
                        ParseResult::Delta(text) => return Some(Ok(text)),
                        ParseResult::ResponseId(id) => {
                            *self.previous_response_id.lock().unwrap() = Some(id);
                            continue;
                        }
                        ParseResult::Done => {
                            self.done = true;
                            return None;
                        }
                        ParseResult::Skip => continue,
                        ParseResult::Error(e) => return Some(Err(e)),
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
                    // Stream ended
                    self.done = true;
                    if !self.buffer.trim().is_empty() {
                        let remaining = self.buffer.trim().to_string();
                        self.buffer.clear();
                        if let Some(data) = remaining.strip_prefix("data: ") {
                            match parse_sse_data(data) {
                                ParseResult::Delta(text) => return Some(Ok(text)),
                                ParseResult::ResponseId(id) => {
                                    *self.previous_response_id.lock().unwrap() = Some(id);
                                }
                                ParseResult::Error(e) => return Some(Err(e)),
                                _ => {}
                            }
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

        let body = client.build_request_body("base64data", "You are helpful.");

        // Verify top-level fields
        assert_eq!(body["stream"], json!(true));
        assert_eq!(body["max_output_tokens"], json!(300));
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["instructions"], "You are helpful.");
        assert_eq!(body["truncation"], "auto");

        // previous_response_id should be absent when None
        assert!(body.get("previous_response_id").is_none());

        // Verify input array
        let input = body["input"].as_array().unwrap();
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["type"], "message");
        assert_eq!(input[0]["role"], "user");

        let content = input[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "input_text");
        assert_eq!(content[0]["text"], "What do you see?");
        assert_eq!(content[1]["type"], "input_image");
        assert_eq!(
            content[1]["image_url"],
            "data:image/jpeg;base64,base64data"
        );
    }

    #[test]
    fn test_request_body_with_previous_response_id() {
        let client = AzureVisionClient::new(
            "https://test.openai.azure.com",
            "test-key",
            "gpt-4o",
            "default prompt",
        );

        *client.previous_response_id.lock().unwrap() = Some("resp_abc123".into());

        let body = client.build_request_body("img", "prompt");
        assert_eq!(body["previous_response_id"], "resp_abc123");
    }

    #[test]
    fn test_parse_sse_data_delta() {
        let data =
            r#"{"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"Hello"}"#;
        match parse_sse_data(data) {
            ParseResult::Delta(text) => assert_eq!(text, "Hello"),
            other => panic!("expected Delta, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[test]
    fn test_parse_sse_data_multi_word() {
        let data = r#"{"type":"response.output_text.delta","delta":" world"}"#;
        match parse_sse_data(data) {
            ParseResult::Delta(text) => assert_eq!(text, " world"),
            other => panic!("expected Delta, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[test]
    fn test_parse_sse_data_done() {
        let data = "[DONE]";
        assert!(matches!(parse_sse_data(data), ParseResult::Done));
    }

    #[test]
    fn test_parse_sse_data_response_created() {
        let data = r#"{"type":"response.created","response":{"id":"resp_xyz"}}"#;
        match parse_sse_data(data) {
            ParseResult::ResponseId(id) => assert_eq!(id, "resp_xyz"),
            other => panic!(
                "expected ResponseId, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn test_parse_sse_data_output_text_done() {
        let data = r#"{"type":"response.output_text.done","text":"Hello world"}"#;
        assert!(matches!(parse_sse_data(data), ParseResult::Done));
    }

    #[test]
    fn test_parse_sse_data_completed() {
        let data = r#"{"type":"response.completed","response":{"id":"resp_abc123"}}"#;
        assert!(matches!(parse_sse_data(data), ParseResult::Done));
    }

    #[test]
    fn test_parse_sse_data_unknown_event_skipped() {
        let data = r#"{"type":"response.output_item.added","item":{}}"#;
        assert!(matches!(parse_sse_data(data), ParseResult::Skip));
    }

    #[test]
    fn test_parse_sse_data_invalid_json() {
        let data = "not valid json{{{";
        assert!(matches!(parse_sse_data(data), ParseResult::Error(_)));
    }

    #[test]
    fn test_client_name() {
        let client = AzureVisionClient::new("https://test.openai.azure.com", "k", "d", "p");
        assert_eq!(client.name(), "azure-openai-vision");
    }

    #[tokio::test]
    async fn test_sse_stream_parsing() {
        let sse_data = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hi\"}\n\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\" there\"}\n\ndata: {\"type\":\"response.output_text.done\",\"text\":\"Hi there\"}\n\n";

        let mut results = Vec::new();
        for line in sse_data.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                match parse_sse_data(data) {
                    ParseResult::Delta(text) => results.push(text),
                    ParseResult::Done => break,
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
            "{}/openai/v1/responses?api-version=preview",
            client.endpoint.trim_end_matches('/'),
        );
        assert_eq!(
            url,
            "https://beme-foundry.openai.azure.com/openai/v1/responses?api-version=preview"
        );
    }
}
