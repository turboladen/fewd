pub mod commands;
pub mod config;
pub mod db;
pub mod entities;
pub mod services;

use config::LockInfo;
use sea_orm::DatabaseConnection;
use std::path::PathBuf;

pub struct AppState {
    pub db: DatabaseConnection,
    pub config_dir: PathBuf,
    pub db_path: PathBuf,
    pub foreign_lock: Option<LockInfo>,
}
