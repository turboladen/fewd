//! Regression test for fewd-4rg: SQLite connection pragmas must apply to *every*
//! pooled connection, not just whichever one happened to run the `PRAGMA` at init.
//!
//! `busy_timeout` is per-connection and ephemeral (unlike `journal_mode=WAL`, which
//! lives in the DB header). Setting it once on a single pooled connection leaves every
//! other connection — and any connection reopened after an idle close — at SQLite's
//! fail-fast default of 0, so a write that lands there gets an immediate `SQLITE_BUSY`
//! instead of waiting out a transient lock.

use sea_orm::sqlx::Row;
use std::time::Duration;

mod common;

use common::TempDbPath;
use fewd_lib::db;

#[tokio::test]
async fn every_pooled_connection_has_busy_timeout() {
    let temp = TempDbPath::new("busy_timeout_test");
    let db = db::init(&temp.0).await.expect("db::init failed");
    let pool = db.get_sqlite_connection_pool();

    // Pin one connection, then open a second while the first is held. The pool must
    // therefore hand out two *distinct* physical connections. The fix's whole point is
    // that the second one — opened lazily, after init's PRAGMA already ran — still
    // carries busy_timeout. The bound only exists to turn a pool-of-1 regression (which
    // would otherwise hang forever, since `cargo test` has no per-test timeout) into a
    // fast, legible failure; it's set generously so a merely-busy CI runner opening a
    // genuine second connection never trips it.
    let mut first = pool
        .acquire()
        .await
        .expect("acquire first connection failed");
    let mut second = tokio::time::timeout(Duration::from_secs(10), pool.acquire())
        .await
        .expect(
            "could not open a second concurrent connection within 10s — the pool is almost \
             certainly capped at a single connection, so pragmas cannot be guaranteed across \
             the concurrent UI + MCP load this fix targets",
        )
        .expect("acquire second connection failed");

    for (label, conn) in [("first", &mut first), ("second", &mut second)] {
        let timeout: i32 = sea_orm::sqlx::query("PRAGMA busy_timeout")
            .fetch_one(&mut **conn)
            .await
            .expect("PRAGMA busy_timeout query failed")
            .get(0);
        assert_eq!(
            timeout, 5000,
            "{label} pooled connection has busy_timeout={timeout}, expected 5000 (the value \
             db::init configures) — the pragma did not reach this connection (or the \
             configured busy_timeout changed)",
        );
    }
}
