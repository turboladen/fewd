use sea_orm::*;

use crate::entities::setting::Entity as Setting;
use crate::services::begin_write;
use crate::services::claude_client::ClaudeClient;

pub struct SettingsService;

impl SettingsService {
    pub async fn get<C: ConnectionTrait>(db: &C, key: String) -> Result<Option<String>, DbErr> {
        let result = Setting::find_by_id(key).one(db).await?;
        Ok(result.map(|m| m.value))
    }

    /// Stores a setting, inserting the key if it is absent and replacing its
    /// value if it exists. Concurrent calls are safe and the last one wins.
    /// To accumulate a value, such as a counter, use one SQL statement that
    /// does the arithmetic instead (see `increment_token_usage`).
    pub async fn set<C: ConnectionTrait>(db: &C, key: String, value: String) -> Result<(), DbErr> {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key.into(), value.into()],
        ))
        .await?;
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
    /// `ClaudeClient::default_model` when the setting is missing, unreadable,
    /// or empty/whitespace. The Settings write path does not currently trim or
    /// validate this field, so the empty-string case is a real possibility —
    /// silently falling back is friendlier than letting an empty model name
    /// reach the Anthropic API and produce a confusing 400.
    pub async fn get_claude_model(db: &DatabaseConnection) -> String {
        Self::get(db, "claude_model".to_string())
            .await
            .ok()
            .flatten()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| ClaudeClient::default_model().to_string())
    }

    /// Adds to the cumulative token-usage counters. The three counters advance
    /// together or not at all, and concurrent callers never lose an update.
    /// A failure is logged, not returned.
    //
    // Each counter moves in one `INSERT … ON CONFLICT(key) DO UPDATE SET value =
    // value + delta` statement, so the read and the add happen together under
    // SQLite's write lock.
    pub async fn increment_token_usage(
        db: &DatabaseConnection,
        input_tokens: u64,
        output_tokens: u64,
    ) {
        let txn = match begin_write(db).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("Failed to begin transaction for token usage: {}", e);
                return;
            }
        };

        let result: Result<(), DbErr> = async {
            Self::increment_counter(&txn, "token_usage_input", input_tokens).await?;
            Self::increment_counter(&txn, "token_usage_output", output_tokens).await?;
            Self::increment_counter(&txn, "token_usage_requests", 1).await?;
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

    /// Atomically add `delta` to an integer-valued counter setting, creating it at
    /// `delta` if absent. The read and add happen inside one `INSERT … ON CONFLICT DO
    /// UPDATE` statement, which SQLite runs under its single-writer lock, so there is no
    /// lost-update window the way a separate `get` + `set` has.
    async fn increment_counter<C: ConnectionTrait>(
        db: &C,
        key: &str,
        delta: u64,
    ) -> Result<(), DbErr> {
        // SQLite integers are i64; reject (rather than silently wrap) a delta that
        // wouldn't fit. Unreachable for real token counts, but `as` would wrap to a
        // negative increment, corrupting the counter.
        let delta = i64::try_from(delta).map_err(|_| {
            DbErr::Custom(format!("counter '{key}' delta {delta} exceeds i64::MAX"))
        })?;
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = CAST(value AS INTEGER) + ?",
            [key.into(), delta.into(), delta.into()],
        ))
        .await?;
        Ok(())
    }
}
