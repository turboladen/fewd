use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "config.json";
const LOCK_FILENAME: &str = ".fewd.lock";
const LOCK_STALE_SECS: i64 = 300; // 5 minutes

/// Local app configuration stored outside the database.
/// Lives at `~/Library/Application Support/com.fewd.dev/config.json` and is
/// never synced — it tells the app where to find the (potentially shared) database.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Custom directory containing the database. When `None`, the default
    /// local app data directory is used.
    pub db_dir: Option<String>,
}

impl AppConfig {
    /// Read config from disk. Returns the default config if the file is
    /// missing, empty, or malformed — the app should always be able to start.
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(CONFIG_FILENAME);
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Write config to disk, creating the parent directory if needed.
    pub fn save(&self, config_dir: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(config_dir)?;
        let path = config_dir.join(CONFIG_FILENAME);
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Resolve the actual database file path based on this config.
    /// Uses `db_dir/fewd.db` if a custom dir is set, otherwise `default_data_dir/fewd.db`.
    pub fn resolve_db_path(&self, default_data_dir: &Path) -> PathBuf {
        match &self.db_dir {
            Some(dir) => PathBuf::from(dir).join("fewd.db"),
            None => default_data_dir.join("fewd.db"),
        }
    }

    /// Whether this config points to a non-default (custom) database location.
    pub fn is_custom(&self) -> bool {
        self.db_dir.is_some()
    }
}

/// Lock file placed alongside the database to warn about concurrent access.
#[derive(Debug, Serialize, Deserialize)]
pub struct LockInfo {
    pub machine_name: String,
    pub timestamp: i64,
}

/// Check for an active lock from a different machine. Returns the lock info
/// if a foreign, non-stale lock exists.
pub fn check_foreign_lock(db_dir: &Path) -> Option<LockInfo> {
    let lock_path = db_dir.join(LOCK_FILENAME);
    let contents = std::fs::read_to_string(&lock_path).ok()?;
    let lock: LockInfo = serde_json::from_str(&contents).ok()?;

    let now = chrono::Utc::now().timestamp();
    if now - lock.timestamp > LOCK_STALE_SECS {
        return None; // Stale lock, ignore
    }

    let local_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    if lock.machine_name == local_name {
        return None; // Our own lock
    }

    Some(lock)
}

/// Create or refresh the lock file for this machine.
pub fn acquire_lock(db_dir: &Path) {
    let lock_path = db_dir.join(LOCK_FILENAME);
    let machine_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let lock = LockInfo {
        machine_name,
        timestamp: chrono::Utc::now().timestamp(),
    };

    if let Ok(json) = serde_json::to_string_pretty(&lock) {
        let _ = std::fs::write(lock_path, json);
    }
}

/// Remove the lock file on clean shutdown.
pub fn release_lock(db_dir: &Path) {
    let lock_path = db_dir.join(LOCK_FILENAME);
    let _ = std::fs::remove_file(lock_path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = PathBuf::from("/tmp/fewd_test_config_missing");
        let _ = fs::remove_dir_all(&dir);
        let config = AppConfig::load(&dir);
        assert!(config.db_dir.is_none());
    }

    #[test]
    fn load_returns_default_when_file_malformed() {
        let dir = PathBuf::from("/tmp/fewd_test_config_malformed");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CONFIG_FILENAME), "not json!!!").unwrap();
        let config = AppConfig::load(&dir);
        assert!(config.db_dir.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = PathBuf::from("/tmp/fewd_test_config_roundtrip");
        let _ = fs::remove_dir_all(&dir);

        let config = AppConfig {
            db_dir: Some("/Users/shared/icloud/fewd".to_string()),
        };
        config.save(&dir).unwrap();

        let loaded = AppConfig::load(&dir);
        assert_eq!(loaded.db_dir, Some("/Users/shared/icloud/fewd".to_string()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_db_path_uses_default_when_no_custom_dir() {
        let config = AppConfig { db_dir: None };
        let default_dir = PathBuf::from("/default/app/data");
        assert_eq!(
            config.resolve_db_path(&default_dir),
            PathBuf::from("/default/app/data/fewd.db")
        );
    }

    #[test]
    fn resolve_db_path_uses_custom_dir() {
        let config = AppConfig {
            db_dir: Some("/custom/icloud/fewd".to_string()),
        };
        let default_dir = PathBuf::from("/default/app/data");
        assert_eq!(
            config.resolve_db_path(&default_dir),
            PathBuf::from("/custom/icloud/fewd/fewd.db")
        );
    }

    #[test]
    fn is_custom_reflects_db_dir() {
        assert!(!AppConfig::default().is_custom());
        assert!(AppConfig {
            db_dir: Some("/foo".to_string())
        }
        .is_custom());
    }
}
