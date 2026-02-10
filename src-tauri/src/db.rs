use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr};
use std::path::PathBuf;
use tauri::Manager;

use crate::config::{self, AppConfig, LockInfo};

/// Result of database initialization, carrying both the connection and metadata.
pub struct DbInit {
    pub db: DatabaseConnection,
    pub config_dir: PathBuf,
    pub db_path: PathBuf,
    /// If a non-stale lock from another machine was found, includes that info.
    pub foreign_lock: Option<LockInfo>,
}

pub async fn init(app_handle: &tauri::AppHandle) -> Result<DbInit, DbErr> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .expect("Failed to get app config directory");

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    // Load local config (always succeeds — falls back to defaults)
    let config = AppConfig::load(&config_dir);
    let db_path = config.resolve_db_path(&app_data_dir);

    // Ensure the database directory exists (important for iCloud paths)
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create database directory");
    }

    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let db = Database::connect(&db_url).await?;

    // Apply SQLite pragmas based on whether we're using a custom (synced) location
    if config.is_custom() {
        // DELETE journal mode: single-file transactions, safe for iCloud/Dropbox sync.
        // No -wal/-shm companion files that could sync out of order.
        db.execute_unprepared("PRAGMA journal_mode=DELETE").await?;
        db.execute_unprepared("PRAGMA busy_timeout=5000").await?;
        db.execute_unprepared("PRAGMA synchronous=FULL").await?;
    } else {
        // Default local path: WAL mode for better read/write performance
        db.execute_unprepared("PRAGMA journal_mode=WAL").await?;
        db.execute_unprepared("PRAGMA busy_timeout=5000").await?;
    }

    migration::Migrator::up(&db, None).await?;

    crate::services::seed_data::seed_if_empty(&db).await?;

    // Lock file: check for foreign locks, then acquire our own (custom locations only)
    let foreign_lock = if config.is_custom() {
        let db_dir = db_path.parent().unwrap().to_path_buf();
        let lock = config::check_foreign_lock(&db_dir);
        config::acquire_lock(&db_dir);
        lock
    } else {
        None
    };

    Ok(DbInit {
        db,
        config_dir,
        db_path,
        foreign_lock,
    })
}
