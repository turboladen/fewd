use sea_orm::*;

use crate::entities::setting::{self, Entity as Setting};
use crate::services::claude_client::ClaudeClient;

pub struct SettingsService;

impl SettingsService {
    pub async fn get<C: ConnectionTrait>(db: &C, key: String) -> Result<Option<String>, DbErr> {
        let result = Setting::find_by_id(key).one(db).await?;
        Ok(result.map(|m| m.value))
    }

    /// Upsert a setting: insert if the key doesn't exist, update if it does.
    ///
    /// This is a non-atomic read-then-write (find + insert/update), fine for
    /// last-writer-wins settings (API key, model, …). It is NOT safe for concurrent
    /// read-modify-write accumulation — two writers can both read the old value and one
    /// update is lost. For counters, do the arithmetic in a single SQL statement instead
    /// (see `increment_token_usage` / `increment_counter`).
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

    /// Increment the cumulative token-usage counters. Each counter is bumped with one
    /// atomic `INSERT … ON CONFLICT(key) DO UPDATE SET value = value + delta` statement:
    /// the read and add happen together in SQL under the row write-lock, so concurrent
    /// callers can't lose updates (the bug fewd-4as fixed). The upsert is also
    /// write-first — no `SELECT` precedes it — so there's no read-snapshot to invalidate,
    /// and a contended call waits on the write lock (busy_timeout) instead of failing
    /// with `SQLITE_BUSY_SNAPSHOT`; that safety comes from the statement shape, not the
    /// transaction. The surrounding `db.begin()` is only for all-or-nothing grouping, so
    /// the three counters never advance partially.
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
    /// `delta` if absent. A single `INSERT … ON CONFLICT DO UPDATE` statement — the
    /// read and write happen together under SQLite's row write-lock, so there is no
    /// lost-update window the way a separate `get` + `set` has.
    async fn increment_counter<C: ConnectionTrait>(
        db: &C,
        key: &str,
        delta: u64,
    ) -> Result<(), DbErr> {
        let delta = delta as i64;
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
