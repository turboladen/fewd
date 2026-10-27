pub mod ai_suggestion_service;
pub mod bar_item_service;
pub mod claude_client;
pub mod cocktail_suggestion_service;
pub mod drink_recipe_import_service;
pub mod drink_recipe_service;
pub mod ingredient_amount;
pub mod ingredient_splitter;
pub mod mcp_token_service;
pub mod meal_service;
pub mod meal_template_service;
pub mod normalize;
pub mod pantry_classifier;
pub mod paren_notes;
pub mod person_service;
pub mod printable_service;
pub mod prompt_builder;
pub mod recipe_adapter;
pub mod recipe_enhancer;
pub mod recipe_import_service;
pub mod recipe_parser;
pub mod recipe_scaler;
pub mod recipe_service;
pub mod recipe_times;
pub mod recipe_variant;
pub mod seed_data;
pub mod service_error;
pub mod settings_service;
pub mod shopping_service;
pub mod suggestion_service;
pub mod unit_converter;

pub fn to_json<T: serde::Serialize>(value: &T) -> Result<String, sea_orm::DbErr> {
    serde_json::to_string(value)
        .map_err(|e| sea_orm::DbErr::Custom(format!("JSON serialization error: {}", e)))
}

/// Begins a transaction that holds SQLite's write lock from the start, so
/// its reads and writes may come in any order. Use it for every write
/// transaction. The lock covers the whole database, so every other writer
/// waits until this transaction ends: keep slow work, such as hashing or
/// network calls, outside it.
//
// SeaORM's `begin()` issues BEGIN DEFERRED, which takes the write lock only at
// the first write. In WAL mode a deferred transaction that has already read
// gets SQLITE_BUSY or SQLITE_BUSY_SNAPSHOT at that write when another
// connection holds or has just committed a write, and busy_timeout does not
// retry it. SeaORM cannot issue BEGIN IMMEDIATE on SQLite, so a no-op UPDATE
// takes the lock instead. It takes it even when it matches no row, so any
// table works. `settings` comes from m20260208_000006_create_settings, so
// this needs migrations through that one applied.
pub async fn begin_write(
    db: &sea_orm::DatabaseConnection,
) -> Result<sea_orm::DatabaseTransaction, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

    let txn = db.begin().await?;
    txn.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "UPDATE settings SET key = key WHERE 0",
    ))
    .await?;
    Ok(txn)
}
