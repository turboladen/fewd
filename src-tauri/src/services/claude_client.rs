use serde::{Deserialize, Serialize};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

// --- Request types ---

#[derive(Debug, Serialize)]
struct MessageRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<Message>,
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
        let client = reqwest::Client::new();

        let request = MessageRequest {
            model: model.to_string(),
            max_tokens: DEFAULT_MAX_TOKENS,
            system: system_prompt.map(|s| s.to_string()),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_message.to_string(),
            }],
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

    /// Minimal API call to verify the key and model work
    pub async fn test_connection(api_key: &str, model: &str) -> Result<String, ClaudeError> {
        let response =
            Self::send_message(api_key, model, None, "Say 'connected' and nothing else.").await?;
        Ok(response.text)
    }
}
