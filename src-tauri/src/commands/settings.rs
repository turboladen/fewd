use serde::{Deserialize, Serialize};
use tauri::State;

use crate::services::claude_client::ClaudeClient;
use crate::services::settings_service::SettingsService;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelOption {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_requests: u64,
}

#[tauri::command]
pub async fn get_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    SettingsService::get(&state.db, key).await.map_err(|e| {
        eprintln!("Failed to get setting: {}", e);
        format!("Could not get setting: {}", e)
    })
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    SettingsService::set(&state.db, key, value)
        .await
        .map_err(|e| {
            eprintln!("Failed to set setting: {}", e);
            format!("Could not set setting: {}", e)
        })
}

#[tauri::command]
pub async fn get_available_models(state: State<'_, AppState>) -> Result<Vec<ModelOption>, String> {
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(|e| format!("Failed to read API key: {}", e))?;

    let models = match api_key {
        Some(key) if !key.is_empty() => ClaudeClient::list_models(&key).await.unwrap_or_else(|e| {
            eprintln!("Failed to fetch models from API, using fallback: {}", e);
            ClaudeClient::fallback_models()
        }),
        _ => ClaudeClient::fallback_models(),
    };

    Ok(models
        .into_iter()
        .map(|m| ModelOption {
            id: m.id,
            name: m.display_name,
        })
        .collect())
}

#[tauri::command]
pub async fn test_claude_connection(state: State<'_, AppState>) -> Result<String, String> {
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(|e| format!("Failed to read API key: {}", e))?
        .ok_or_else(|| "No API key configured".to_string())?;

    if api_key.is_empty() {
        return Err("No API key configured".to_string());
    }

    let model = SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .map_err(|e| format!("Failed to read model setting: {}", e))?
        .unwrap_or_else(|| ClaudeClient::default_model().to_string());

    ClaudeClient::test_connection(&api_key, &model)
        .await
        .map_err(|e| format!("Connection test failed: {}", e))
}

#[tauri::command]
pub async fn get_token_usage(state: State<'_, AppState>) -> Result<TokenUsage, String> {
    let input_tokens: u64 = SettingsService::get(&state.db, "token_usage_input".to_string())
        .await
        .map_err(|e| format!("Failed to read token_usage_input: {}", e))?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let output_tokens: u64 = SettingsService::get(&state.db, "token_usage_output".to_string())
        .await
        .map_err(|e| format!("Failed to read token_usage_output: {}", e))?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let total_requests: u64 = SettingsService::get(&state.db, "token_usage_requests".to_string())
        .await
        .map_err(|e| format!("Failed to read token_usage_requests: {}", e))?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    Ok(TokenUsage {
        input_tokens,
        output_tokens,
        total_requests,
    })
}
