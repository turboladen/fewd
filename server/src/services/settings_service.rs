use sea_orm::*;

use crate::entities::setting::{self, Entity as Setting};

pub struct SettingsService;

impl SettingsService {
    pub async fn get(db: &DatabaseConnection, key: String) -> Result<Option<String>, DbErr> {
        let result = Setting::find_by_id(key).one(db).await?;
        Ok(result.map(|m| m.value))
    }

    /// Upsert: insert if key doesn't exist, update if it does
    pub async fn set(db: &DatabaseConnection, key: String, value: String) -> Result<(), DbErr> {
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

    pub async fn delete(db: &DatabaseConnection, key: String) -> Result<(), DbErr> {
        Setting::delete_by_id(key).exec(db).await?;
        Ok(())
    }

    /// Increment cumulative token usage counters. Errors are silently ignored (non-critical).
    pub async fn increment_token_usage(
        db: &DatabaseConnection,
        input_tokens: u64,
        output_tokens: u64,
    ) {
        let current_input: u64 = Self::get(db, "token_usage_input".to_string())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let current_output: u64 = Self::get(db, "token_usage_output".to_string())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let current_requests: u64 = Self::get(db, "token_usage_requests".to_string())
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let _ = Self::set(
            db,
            "token_usage_input".to_string(),
            (current_input + input_tokens).to_string(),
        )
        .await;
        let _ = Self::set(
            db,
            "token_usage_output".to_string(),
            (current_output + output_tokens).to_string(),
        )
        .await;
        let _ = Self::set(
            db,
            "token_usage_requests".to_string(),
            (current_requests + 1).to_string(),
        )
        .await;
    }
}
