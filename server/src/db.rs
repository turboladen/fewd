use std::time::Duration;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sea_orm::{DatabaseConnection, DbErr, RuntimeErr, SqlxSqliteConnector};

/// Max pooled SQLite connections. >1 so the web UI and the MCP server can read
/// concurrently under WAL; kept small because this is a single-household app with light
/// write load.
const MAX_CONNECTIONS: u32 = 5;

pub async fn init(db_path: &str) -> Result<DatabaseConnection, DbErr> {
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        std::fs::create_dir_all(parent).expect("Failed to create database directory");
    }

    // Pragmas live on the connect options so *every* pooled connection — including any
    // reopened after an idle close — inherits them. `busy_timeout` in particular is
    // per-connection and ephemeral: setting it once post-connect (the old approach) left
    // every other connection at SQLite's fail-fast default of 0, turning a transient lock
    // into an immediate `SQLITE_BUSY` (fewd-4rg). `journal_mode=WAL` persists in the DB
    // header, but we set it here too so a freshly created DB is in WAL from its first open.
    let connect_options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect_with(connect_options)
        .await
        .map_err(|e| DbErr::Conn(RuntimeErr::SqlxError(e)))?;

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);

    migration::Migrator::up(&db, None).await?;

    crate::services::seed_data::seed_if_empty(&db).await?;

    Ok(db)
}
