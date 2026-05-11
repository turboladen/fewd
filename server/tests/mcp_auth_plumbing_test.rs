//! End-to-end auth-context plumbing test for the MCP surface (fewd-2y6.8).
//!
//! The unit-level middleware tests in `mcp/mod.rs` cover the middleware's
//! own behavior (401 on missing/empty/invalid/revoked/rotated bearer).
//! What they DON'T cover is the full chain `middleware → rmcp service →
//! tool body`: that the `AuthenticatedPerson` extension the middleware
//! inserts actually reaches the tool body's `RequestContext::extensions`
//! after rmcp's `StreamableHttpService` repacks the request.
//!
//! This test pins that chain by driving the production `mcp::router`
//! end-to-end via `tower::ServiceExt::oneshot`. If a future refactor
//! (e.g. removing the extension insertion, changing how rmcp surfaces
//! axum extensions to tools) breaks the plumbing, every authenticated
//! tool call would silently return "missing authenticated person"
//! 500s — this test surfaces that as a failure.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use fewd_lib::services::mcp_token_service::McpTokenService;
use fewd_lib::services::person_service::PersonService;
use fewd_lib::{dto::CreatePersonDto, mcp};
use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection};
use tower::ServiceExt;

async fn setup_db_with_token() -> (DatabaseConnection, String) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite connects");
    migration::Migrator::up(&db, None)
        .await
        .expect("migrations run on empty DB");

    let alice = PersonService::create(
        &db,
        CreatePersonDto {
            name: "Alice".into(),
            birthdate: "1990-01-01".into(),
            dietary_goals: None,
            dislikes: vec![],
            favorites: vec![],
            notes: None,
            drink_preferences: None,
            drink_dislikes: None,
        },
    )
    .await
    .expect("create alice");

    let issued = McpTokenService::provision(&db, &alice.id)
        .await
        .expect("provision token");

    (db, issued.plaintext)
}

/// The MCP `initialize` handshake reaches the rmcp service after the
/// auth middleware has run. If the middleware fails to plumb
/// `AuthenticatedPerson` into the context, or rmcp fails to surface the
/// axum extensions to the tool layer, the chain breaks somewhere before
/// the rmcp service sees a valid session and the response is no longer
/// 200. The bearer middleware itself is already exhaustively unit-tested;
/// this is the integration witness that the hand-off downstream works.
#[tokio::test]
async fn valid_token_completes_mcp_initialize_handshake() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);

    let body = r#"{
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1,
        "params": {
            "protocolVersion": "2025-11-05",
            "capabilities": {},
            "clientInfo": { "name": "auth-plumbing-test", "version": "0" }
        }
    }"#;

    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("host", "localhost")
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(req).await.expect("router responds");

    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    let body_str = String::from_utf8_lossy(&bytes);
    assert_eq!(
        status,
        StatusCode::OK,
        "initialize must reach rmcp and succeed when the bearer is valid; body: {body_str}"
    );
}

/// Counterpart to the happy-path test: a malformed (but well-shaped)
/// bearer must short-circuit at the middleware before any of the rmcp
/// chain runs. If a future refactor accidentally lets unauthenticated
/// requests through to the tool layer, this test catches it.
#[tokio::test]
async fn unrecognized_token_short_circuits_before_rmcp() {
    let (db, _real_token) = setup_db_with_token().await;
    let app = mcp::router(db);

    let body = r#"{
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1,
        "params": {
            "protocolVersion": "2025-11-05",
            "capabilities": {},
            "clientInfo": { "name": "auth-plumbing-test", "version": "0" }
        }
    }"#;

    let bogus = "A".repeat(43); // shape-valid base64url length, but never issued
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("authorization", format!("Bearer {}", bogus))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(Body::from(body))
        .unwrap();

    let response = app.oneshot(req).await.expect("router responds");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    let body_str = String::from_utf8_lossy(&bytes);
    assert!(
        body_str.contains("invalid or revoked token"),
        "401 body must surface the actionable token-state message: {body_str}"
    );
}
