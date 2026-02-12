use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr};

pub async fn init(db_path: &str) -> Result<DatabaseConnection, DbErr> {
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        std::fs::create_dir_all(parent).expect("Failed to create database directory");
    }

    let db_url = format!("sqlite:{}?mode=rwc", db_path);
    let db = Database::connect(&db_url).await?;

    db.execute_unprepared("PRAGMA journal_mode=WAL").await?;
    db.execute_unprepared("PRAGMA busy_timeout=5000").await?;

    migration::Migrator::up(&db, None).await?;

    crate::services::seed_data::seed_if_empty(&db).await?;

    Ok(db)
}
