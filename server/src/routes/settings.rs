use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use crate::dto::{ModelOption, SetSettingBody, TokenUsage};
use crate::error::AppError;
use crate::services::claude_client::ClaudeClient;
use crate::services::settings_service::SettingsService;
use crate::AppState;

/// Allowed setting keys — reject anything not in this list
const ALLOWED_KEYS: &[&str] = &[
    "anthropic_api_key",
    "claude_model",
    "token_usage_input",
    "token_usage_output",
    "token_usage_requests",
    "database_path",
    "cost_input_price_per_mtok",
    "cost_output_price_per_mtok",
];

fn validate_key(key: &str) -> Result<(), AppError> {
    if ALLOWED_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!("Unknown setting: {}", key)))
    }
}

/// Mask a secret so the client can see it's configured without exposing the full value
fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        return "*".repeat(key.len());
    }
    let suffix = &key[key.len() - 4..];
    format!("{}...{}", &key[..8], suffix)
}

pub async fn get_setting(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Option<String>>, AppError> {
    validate_key(&key)?;

    let value = SettingsService::get(&state.db, key.clone())
        .await
        .map_err(AppError::from)?;

    // Never return the raw API key
    let value = if key == "anthropic_api_key" {
        value.map(|v| mask_api_key(&v))
    } else {
        value
    };

    Ok(Json(value))
}

pub async fn set_setting(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<SetSettingBody>,
) -> Result<StatusCode, AppError> {
    validate_key(&key)?;

    let value = if key == "anthropic_api_key" {
        body.value.trim().to_string()
    } else {
        body.value
    };
    SettingsService::set(&state.db, key, value)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub async fn available_models(
    State(state): State<AppState>,
) -> Result<Json<Vec<ModelOption>>, AppError> {
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(AppError::from)?;

    let models = match api_key {
        Some(key) if !key.is_empty() => ClaudeClient::list_models(&key).await.unwrap_or_else(|e| {
            tracing::warn!("Failed to fetch models from API, using fallback: {}", e);
            ClaudeClient::fallback_models()
        }),
        _ => ClaudeClient::fallback_models(),
    };

    Ok(Json(
        models
            .into_iter()
            .map(|m| ModelOption {
                id: m.id,
                name: m.display_name,
            })
            .collect(),
    ))
}

pub async fn test_connection(State(state): State<AppState>) -> Result<Json<String>, AppError> {
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::BadRequest("No API key configured".into()))?;

    if api_key.is_empty() {
        return Err(AppError::BadRequest("No API key configured".into()));
    }

    let model = SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| ClaudeClient::default_model().to_string());

    ClaudeClient::test_connection(&api_key, &model)
        .await
        .map(Json)
        .map_err(|e| AppError::Internal(format!("Connection test failed: {}", e)))
}

pub async fn token_usage(State(state): State<AppState>) -> Result<Json<TokenUsage>, AppError> {
    let input_tokens: u64 = SettingsService::get(&state.db, "token_usage_input".to_string())
        .await
        .map_err(AppError::from)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let output_tokens: u64 = SettingsService::get(&state.db, "token_usage_output".to_string())
        .await
        .map_err(AppError::from)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let total_requests: u64 = SettingsService::get(&state.db, "token_usage_requests".to_string())
        .await
        .map_err(AppError::from)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    Ok(Json(TokenUsage {
        input_tokens,
        output_tokens,
        total_requests,
    }))
}
