pub mod commands;
pub mod db;
pub mod entities;
pub mod services;

use sea_orm::DatabaseConnection;

pub struct AppState {
    pub db: DatabaseConnection,
}
