pub mod db;
pub mod dto;
pub mod entities;
pub mod error;
pub mod routes;
pub mod services;

use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
}
