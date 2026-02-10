use crate::config::AppConfig;
use crate::AppState;
use sea_orm::ConnectionTrait;
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct DbConfig {
    pub custom_dir: Option<String>,
    pub active_path: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub has_existing_db: bool,
    pub is_icloud: bool,
    pub warning: Option<String>,
}

#[tauri::command]
pub async fn get_db_config(state: State<'_, AppState>) -> Result<DbConfig, String> {
    let config = AppConfig::load(&state.config_dir);
    Ok(DbConfig {
        custom_dir: config.db_dir.clone(),
        active_path: state.db_path.display().to_string(),
        is_default: !config.is_custom(),
    })
}

#[tauri::command]
pub async fn set_db_location(
    state: State<'_, AppState>,
    dir_path: Option<String>,
) -> Result<(), String> {
    let mut config = AppConfig::load(&state.config_dir);
    config.db_dir = dir_path;
    config.save(&state.config_dir).map_err(|e| {
        eprintln!("Failed to save config: {}", e);
        format!("Could not save configuration: {}", e)
    })
}

#[tauri::command]
pub async fn validate_db_location(dir_path: String) -> Result<ValidationResult, String> {
    let path = PathBuf::from(&dir_path);

    // Check if directory exists
    if !path.exists() {
        return Ok(ValidationResult {
            valid: false,
            has_existing_db: false,
            is_icloud: false,
            warning: Some("Directory does not exist.".to_string()),
        });
    }

    if !path.is_dir() {
        return Ok(ValidationResult {
            valid: false,
            has_existing_db: false,
            is_icloud: false,
            warning: Some("Path is not a directory.".to_string()),
        });
    }

    // Check writability by creating a temp file
    let test_file = path.join(".fewd_write_test");
    let writable = std::fs::write(&test_file, "test").is_ok();
    let _ = std::fs::remove_file(&test_file);

    if !writable {
        return Ok(ValidationResult {
            valid: false,
            has_existing_db: false,
            is_icloud: false,
            warning: Some("Directory is not writable.".to_string()),
        });
    }

    let has_existing_db = path.join("fewd.db").exists();

    // Detect iCloud paths (macOS iCloud Drive uses "Mobile Documents" in the path)
    let path_str = dir_path.to_lowercase();
    let is_icloud = path_str.contains("mobile documents") || path_str.contains("icloud");

    let warning = if is_icloud {
        Some(
            "iCloud folder detected. The database will sync across your Macs. \
             For best results, avoid using the app on two computers at the exact same time."
                .to_string(),
        )
    } else {
        None
    };

    Ok(ValidationResult {
        valid: true,
        has_existing_db,
        is_icloud,
        warning,
    })
}

#[tauri::command]
pub async fn copy_db_to_location(
    state: State<'_, AppState>,
    dest_dir: String,
) -> Result<(), String> {
    // Checkpoint any WAL data into the main database file before copying
    state
        .db
        .execute_unprepared("PRAGMA wal_checkpoint(TRUNCATE)")
        .await
        .map_err(|e| format!("Failed to checkpoint database: {}", e))?;

    let dest_path = PathBuf::from(&dest_dir).join("fewd.db");

    // Ensure destination directory exists
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create destination directory: {}", e))?;
    }

    std::fs::copy(&state.db_path, &dest_path)
        .map_err(|e| format!("Failed to copy database: {}", e))?;

    eprintln!(
        "Database copied from {} to {}",
        state.db_path.display(),
        dest_path.display()
    );

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct LockWarning {
    pub machine_name: String,
}

/// Returns lock warning info if another machine is actively using the shared database.
#[tauri::command]
pub async fn get_lock_warning(state: State<'_, AppState>) -> Result<Option<LockWarning>, String> {
    Ok(state.foreign_lock.as_ref().map(|lock| LockWarning {
        machine_name: lock.machine_name.clone(),
    }))
}
