//! Regression test for fewd-4as: `SettingsService::increment_token_usage` must not
//! lose updates under concurrent callers.
//!
//! Once the SQLite pool went to max_connections=5 (fewd-4rg), two MCP requests
//! finishing near-simultaneously could both read counter=N and both write N+delta —
//! a classic read-modify-write lost update (or a `SQLITE_BUSY_SNAPSHOT` that the old
//! code swallowed, dropping the increment entirely). This drives many concurrent
//! increments against a real file-backed, multi-connection pool and asserts every
//! one is counted.

use fewd_lib::db;
use fewd_lib::services::settings_service::SettingsService;

/// File-backed (not `:memory:`) so the 5-connection pool shares one database — the
/// condition under which the lost-update race exists.
struct TempDbPath(String);

impl TempDbPath {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .join(format!("fewd_token_usage_test_{}.db", std::process::id()))
            .to_string_lossy()
            .into_owned();
        Self(path)
    }
}

impl Drop for TempDbPath {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.0, suffix));
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_increments_sum_without_lost_updates() {
    let temp = TempDbPath::new();
    let db = db::init(&temp.0).await.expect("db::init failed");

    const N: u64 = 50;
    const INPUT_PER_CALL: u64 = 1;
    const OUTPUT_PER_CALL: u64 = 2;

    let mut handles = Vec::with_capacity(N as usize);
    for _ in 0..N {
        let db = db.clone();
        handles.push(tokio::spawn(async move {
            SettingsService::increment_token_usage(&db, INPUT_PER_CALL, OUTPUT_PER_CALL).await;
        }));
    }
    for h in handles {
        h.await.expect("increment task panicked");
    }

    let counter = |key: &'static str| {
        let db = db.clone();
        async move {
            SettingsService::get(&db, key.to_string())
                .await
                .expect("read counter")
                .unwrap_or_default()
                .parse::<u64>()
                .unwrap_or(0)
        }
    };

    assert_eq!(
        counter("token_usage_input").await,
        N * INPUT_PER_CALL,
        "every input increment must be counted — a lower value means an update was lost",
    );
    assert_eq!(
        counter("token_usage_output").await,
        N * OUTPUT_PER_CALL,
        "output counter"
    );
    assert_eq!(counter("token_usage_requests").await, N, "requests counter");
}
