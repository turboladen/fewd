use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const DEFAULT_MAX_TOKENS: u32 = 4096;

// --- Request types ---

#[derive(Debug, Serialize)]
struct MessageRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

// --- Response types ---

#[derive(Debug, Deserialize)]
struct MessageResponse {
    content: Vec<ContentBlock>,
    #[serde(default)]
    usage: Option<UsageInfo>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageInfo {
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ModelsListResponse {
    data: Vec<ModelEntry>,
    has_more: bool,
    #[serde(default)]
    last_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    display_name: String,
    #[serde(default)]
    created_at: Option<String>,
}

// --- Public types ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SendMessageResponse {
    pub text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

// --- Streaming event types (from Anthropic SSE API) ---

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: StreamMessageInfo },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {},
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: TextDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {},
    #[serde(rename = "message_delta")]
    MessageDelta {
        #[allow(dead_code)]
        delta: MessageDeltaInfo,
        usage: StreamOutputUsage,
    },
    #[serde(rename = "message_stop")]
    MessageStop {},
    #[serde(rename = "ping")]
    Ping {},
    #[serde(rename = "error")]
    StreamError { error: StreamErrorInfo },
}

#[derive(Debug, Deserialize)]
struct StreamMessageInfo {
    usage: StreamInputUsage,
}

#[derive(Debug, Deserialize)]
struct StreamInputUsage {
    input_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct TextDelta {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageDeltaInfo {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamOutputUsage {
    output_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct StreamErrorInfo {
    message: String,
}

// --- Progress events (sent from backend to frontend via SSE) ---

/// Lightweight progress events streamed to the browser while Claude generates
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "phase")]
pub enum ProgressEvent {
    #[serde(rename = "thinking")]
    Thinking { message: String },
    #[serde(rename = "generating")]
    Generating { message: String, tokens: u64 },
}

/// Send progress every N output tokens to avoid flooding the frontend
const PROGRESS_TOKEN_INTERVAL: u64 = 50;

// --- Error type ---

#[derive(Debug)]
pub enum ClaudeError {
    InvalidApiKey,
    RateLimited(String),
    NetworkError(String),
    ApiError { status: u16, message: String },
    ParseError(String),
}

impl std::fmt::Display for ClaudeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaudeError::InvalidApiKey => write!(f, "Invalid API key"),
            ClaudeError::RateLimited(msg) => write!(f, "Rate limited: {}", msg),
            ClaudeError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ClaudeError::ApiError { status, message } => {
                write!(f, "API error ({}): {}", status, message)
            }
            ClaudeError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for ClaudeError {}

// --- Client ---

pub struct ClaudeClient;

impl ClaudeClient {
    pub fn default_model() -> &'static str {
        "claude-sonnet-4-20250514"
    }

    /// Hardcoded fallback models when the API is unavailable
    pub fn fallback_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                display_name: "Claude Sonnet 4".to_string(),
                created_at: None,
            },
            ModelInfo {
                id: "claude-sonnet-4-5-20250514".to_string(),
                display_name: "Claude Sonnet 4.5".to_string(),
                created_at: None,
            },
            ModelInfo {
                id: "claude-haiku-4-5-20250514".to_string(),
                display_name: "Claude Haiku 4.5".to_string(),
                created_at: None,
            },
            ModelInfo {
                id: "claude-opus-4-20250514".to_string(),
                display_name: "Claude Opus 4".to_string(),
                created_at: None,
            },
        ]
    }

    /// Fetch available models from the Anthropic API
    pub async fn list_models(api_key: &str) -> Result<Vec<ModelInfo>, ClaudeError> {
        let client = reqwest::Client::new();

        let response = client
            .get(ANTHROPIC_MODELS_URL)
            .query(&[("limit", "1000")])
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .send()
            .await
            .map_err(|e| ClaudeError::NetworkError(e.to_string()))?;

        let status = response.status().as_u16();

        if status == 401 {
            return Err(ClaudeError::InvalidApiKey);
        }
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(ClaudeError::ApiError {
                status,
                message: body,
            });
        }

        let list_response: ModelsListResponse = response
            .json()
            .await
            .map_err(|e| ClaudeError::ParseError(e.to_string()))?;

        Ok(list_response
            .data
            .into_iter()
            .map(|entry| ModelInfo {
                id: entry.id,
                display_name: entry.display_name,
                created_at: entry.created_at,
            })
            .collect())
    }

    /// Send a message to the Claude API and return the response text + token usage
    pub async fn send_message(
        api_key: &str,
        model: &str,
        system_prompt: Option<&str>,
        user_message: &str,
    ) -> Result<SendMessageResponse, ClaudeError> {
        Self::send_message_with_max_tokens(
            api_key,
            model,
            system_prompt,
            user_message,
            DEFAULT_MAX_TOKENS,
        )
        .await
    }

    /// Send a message with a custom max_tokens limit
    pub async fn send_message_with_max_tokens(
        api_key: &str,
        model: &str,
        system_prompt: Option<&str>,
        user_message: &str,
        max_tokens: u32,
    ) -> Result<SendMessageResponse, ClaudeError> {
        let client = reqwest::Client::new();

        let request = MessageRequest {
            model: model.to_string(),
            max_tokens,
            system: system_prompt.map(|s| s.to_string()),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_message.to_string(),
            }],
            stream: None,
        };

        let response = client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ClaudeError::NetworkError(e.to_string()))?;

        let status = response.status().as_u16();

        if status == 401 {
            return Err(ClaudeError::InvalidApiKey);
        }
        if status == 429 {
            let body = response.text().await.unwrap_or_default();
            return Err(ClaudeError::RateLimited(body));
        }
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(ClaudeError::ApiError {
                status,
                message: body,
            });
        }

        let msg_response: MessageResponse = response
            .json()
            .await
            .map_err(|e| ClaudeError::ParseError(e.to_string()))?;

        if msg_response.stop_reason.as_deref() == Some("max_tokens") {
            eprintln!(
                "Warning: Claude response was truncated (hit max_tokens={})",
                max_tokens
            );
        }

        let text = msg_response
            .content
            .into_iter()
            .find_map(|block| {
                if block.content_type == "text" {
                    block.text
                } else {
                    None
                }
            })
            .ok_or_else(|| ClaudeError::ParseError("No text content in response".to_string()))?;

        let (input_tokens, output_tokens) = match msg_response.usage {
            Some(usage) => (usage.input_tokens, usage.output_tokens),
            None => (0, 0),
        };

        Ok(SendMessageResponse {
            text,
            input_tokens,
            output_tokens,
        })
    }

    /// Send a message using the streaming API, forwarding progress events via the channel.
    /// Returns the same `SendMessageResponse` as the non-streaming methods (full buffered text + usage).
    pub async fn send_message_streaming(
        api_key: &str,
        model: &str,
        system_prompt: Option<&str>,
        user_message: &str,
        max_tokens: u32,
        progress_tx: mpsc::Sender<ProgressEvent>,
    ) -> Result<SendMessageResponse, ClaudeError> {
        let client = reqwest::Client::new();

        let request = MessageRequest {
            model: model.to_string(),
            max_tokens,
            system: system_prompt.map(|s| s.to_string()),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_message.to_string(),
            }],
            stream: Some(true),
        };

        let response = client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ClaudeError::NetworkError(e.to_string()))?;

        let status = response.status().as_u16();

        if status == 401 {
            return Err(ClaudeError::InvalidApiKey);
        }
        if status == 429 {
            let body = response.text().await.unwrap_or_default();
            return Err(ClaudeError::RateLimited(body));
        }
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(ClaudeError::ApiError {
                status,
                message: body,
            });
        }

        // Parse the streaming SSE response
        let mut text_buffer = String::new();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut last_progress_tokens: u64 = 0;

        // SSE line parser state
        let mut line_buffer = String::new();
        let mut current_event_type = String::new();
        let mut current_data = String::new();

        let mut byte_stream = response.bytes_stream();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result
                .map_err(|e| ClaudeError::NetworkError(format!("Stream read error: {}", e)))?;

            line_buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete lines from the buffer
            while let Some(newline_pos) = line_buffer.find('\n') {
                let line = line_buffer[..newline_pos]
                    .trim_end_matches('\r')
                    .to_string();
                line_buffer = line_buffer[newline_pos + 1..].to_string();

                if let Some(event_type) = line.strip_prefix("event: ") {
                    current_event_type = event_type.to_string();
                } else if let Some(data) = line.strip_prefix("data: ") {
                    current_data = data.to_string();
                } else if line.is_empty() && !current_event_type.is_empty() {
                    // Blank line = end of SSE event, process it
                    if !current_data.is_empty() {
                        match serde_json::from_str::<StreamEvent>(&current_data) {
                            Ok(event) => match event {
                                StreamEvent::MessageStart { message } => {
                                    input_tokens = message.usage.input_tokens;
                                }
                                StreamEvent::ContentBlockDelta { delta } => {
                                    if let Some(text) = delta.text {
                                        text_buffer.push_str(&text);
                                    }
                                    // Estimate output tokens from text length (rough: 1 token ≈ 4 chars)
                                    let estimated_tokens = (text_buffer.len() as u64) / 4;
                                    if estimated_tokens
                                        >= last_progress_tokens + PROGRESS_TOKEN_INTERVAL
                                    {
                                        last_progress_tokens = estimated_tokens;
                                        let _ = progress_tx
                                            .send(ProgressEvent::Generating {
                                                message: "Generating...".to_string(),
                                                tokens: estimated_tokens,
                                            })
                                            .await;
                                    }
                                }
                                StreamEvent::MessageDelta { usage, .. } => {
                                    output_tokens = usage.output_tokens;
                                }
                                StreamEvent::StreamError { error } => {
                                    return Err(ClaudeError::ApiError {
                                        status: 500,
                                        message: error.message,
                                    });
                                }
                                // MessageStop, ContentBlockStart/Stop, Ping — no action needed
                                _ => {}
                            },
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to parse streaming event ({}): {}",
                                    current_event_type,
                                    e
                                );
                            }
                        }
                    }
                    current_event_type.clear();
                    current_data.clear();
                }
            }
        }

        if text_buffer.is_empty() {
            return Err(ClaudeError::ParseError(
                "No text content in streaming response".to_string(),
            ));
        }

        Ok(SendMessageResponse {
            text: text_buffer,
            input_tokens,
            output_tokens,
        })
    }

    /// Minimal API call to verify the key and model work
    pub async fn test_connection(api_key: &str, model: &str) -> Result<String, ClaudeError> {
        let response =
            Self::send_message(api_key, model, None, "Say 'connected' and nothing else.").await?;
        Ok(response.text)
    }
}
