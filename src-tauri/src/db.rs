use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection, DbErr};
use tauri::Manager;

pub async fn init(app_handle: &tauri::AppHandle) -> Result<DatabaseConnection, DbErr> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

    let db_path = app_data_dir.join("fewd.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let db = Database::connect(&db_url).await?;

    migration::Migrator::up(&db, None).await?;

    Ok(db)
}
