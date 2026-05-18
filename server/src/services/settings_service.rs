use sea_orm::*;

use crate::entities::setting::{self, Entity as Setting};
use crate::services::claude_client::ClaudeClient;

pub struct SettingsService;

impl SettingsService {
    pub async fn get<C: ConnectionTrait>(db: &C, key: String) -> Result<Option<String>, DbErr> {
        let result = Setting::find_by_id(key).one(db).await?;
        Ok(result.map(|m| m.value))
    }

    /// Upsert: insert if key doesn't exist, update if it does
    pub async fn set<C: ConnectionTrait>(db: &C, key: String, value: String) -> Result<(), DbErr> {
        let existing = Setting::find_by_id(key.clone()).one(db).await?;

        match existing {
            Some(model) => {
                let mut active: setting::ActiveModel = model.into();
                active.value = Set(value);
                active.update(db).await?;
            }
            None => {
                let new_setting = setting::ActiveModel {
                    key: Set(key),
                    value: Set(value),
                };
                new_setting.insert(db).await?;
            }
        }

        Ok(())
    }

    pub async fn delete<C: ConnectionTrait>(db: &C, key: String) -> Result<(), DbErr> {
        Setting::delete_by_id(key).exec(db).await?;
        Ok(())
    }

    /// Returns the configured Anthropic API key, or `None` if it is unset or empty.
    /// Callers map `None` to whatever error fits their surface (HTTP route -> 400,
    /// MCP -> tool_user_error).
    pub async fn get_anthropic_api_key(db: &DatabaseConnection) -> Result<Option<String>, DbErr> {
        let key = Self::get(db, "anthropic_api_key".to_string()).await?;
        Ok(key.filter(|k| !k.is_empty()))
    }

    /// Returns the configured Claude model, falling back to
    /// `ClaudeClient::default_model` when the setting is missing or unreadable.
    pub async fn get_claude_model(db: &DatabaseConnection) -> String {
        Self::get(db, "claude_model".to_string())
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| ClaudeClient::default_model().to_string())
    }

    /// Increment cumulative token usage counters atomically within a transaction.
    pub async fn increment_token_usage(
        db: &DatabaseConnection,
        input_tokens: u64,
        output_tokens: u64,
    ) {
        let txn = match db.begin().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("Failed to begin transaction for token usage: {}", e);
                return;
            }
        };

        let result: Result<(), DbErr> = async {
            let current_input: u64 = Self::get(&txn, "token_usage_input".to_string())
                .await?
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let current_output: u64 = Self::get(&txn, "token_usage_output".to_string())
                .await?
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let current_requests: u64 = Self::get(&txn, "token_usage_requests".to_string())
                .await?
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);

            Self::set(
                &txn,
                "token_usage_input".to_string(),
                (current_input + input_tokens).to_string(),
            )
            .await?;
            Self::set(
                &txn,
                "token_usage_output".to_string(),
                (current_output + output_tokens).to_string(),
            )
            .await?;
            Self::set(
                &txn,
                "token_usage_requests".to_string(),
                (current_requests + 1).to_string(),
            )
            .await?;

            Ok(())
        }
        .await;

        match result {
            Ok(()) => {
                if let Err(e) = txn.commit().await {
                    tracing::warn!("Failed to commit token usage update: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Token usage update failed, rolling back: {}", e);
                let _ = txn.rollback().await;
            }
        }
    }
}
