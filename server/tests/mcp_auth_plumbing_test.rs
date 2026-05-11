//! End-to-end auth-context plumbing test for the MCP surface (fewd-2y6.8).
//!
//! The unit-level middleware tests in `mcp/mod.rs` cover the middleware's
//! own behavior (401 on missing/empty/invalid/revoked/rotated bearer).
//! What they DON'T cover is the full chain `middleware → rmcp service →
//! tool body`: that the `AuthenticatedPerson` extension the middleware
//! inserts actually reaches the tool body's `RequestContext::extensions`
//! after rmcp's `StreamableHttpService` repacks the request.
//!
//! This file drives the production `mcp::router` end-to-end through
//! the full handshake (`initialize` → `notifications/initialized` →
//! `tools/call whoami`) via `tower::ServiceExt::oneshot`. The `whoami`
//! tool is specifically the tool that reads `AuthenticatedPerson` —
//! its response includes the authenticated person's name, so the
//! assertion `"Alice" in response` directly witnesses the full
//! middleware → axum Parts → rmcp RequestContext → tool body chain.
//!
//! If a future refactor (e.g. removing the extension insertion,
//! changing how rmcp surfaces axum extensions to tools) breaks any
//! link in the chain, `whoami` returns "missing authenticated person"
//! instead of the name, and this test fails.

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

/// Drives the full MCP handshake + a real `#[tool]` call end-to-end:
/// `initialize` → `notifications/initialized` → `tools/call` for
/// `whoami`. The `whoami` tool is the canonical reader of the auth
/// context — it goes through [`fewd_lib::mcp::handler::authenticated_name`]
/// → `authenticated_person` → `RequestContext::extensions` → the axum
/// `Parts::extensions` → the [`fewd_lib::mcp::AuthenticatedPerson`]
/// inserted by the bearer middleware. If ANY link in that chain breaks,
/// the response no longer contains the authenticated name.
///
/// Why this drives the full sequence rather than stopping at
/// `initialize`: rmcp's `initialize` is handled by
/// `ServerHandler::get_info(&self)` (a plain `&self` method that takes
/// no `RequestContext`), so a 200 on initialize proves only the
/// middleware → rmcp envelope handoff, not the
/// `extensions`-into-tool-body propagation. Driving `tools/call`
/// closes that gap because the tool body explicitly reads the
/// extension to produce its response.
#[tokio::test]
async fn whoami_round_trips_authenticated_identity_through_tool_body() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");

    // 1) initialize — captures the session id we need for subsequent
    //    requests on the same logical session.
    let init_body = r#"{
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1,
        "params": {
            "protocolVersion": "2025-11-05",
            "capabilities": {},
            "clientInfo": { "name": "auth-plumbing-test", "version": "0" }
        }
    }"#;
    let init_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", &bearer)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(init_body))
                .unwrap(),
        )
        .await
        .expect("init request");
    assert_eq!(init_resp.status(), StatusCode::OK, "initialize must 200");
    let session_id = init_resp
        .headers()
        .get("mcp-session-id")
        .expect("rmcp sets mcp-session-id on initialize response")
        .to_str()
        .expect("session id is ASCII")
        .to_string();
    drop(to_bytes(init_resp.into_body(), 32 * 1024).await.unwrap());

    // 2) notifications/initialized — required by the MCP spec before
    //    the client may issue tool calls. No response body (202).
    let notif_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", &bearer)
                .header("mcp-session-id", &session_id)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                ))
                .unwrap(),
        )
        .await
        .expect("initialized notification request");
    assert!(
        notif_resp.status().is_success(),
        "notifications/initialized must be accepted; got {}",
        notif_resp.status()
    );
    drop(to_bytes(notif_resp.into_body(), 32 * 1024).await.unwrap());

    // 3) tools/call whoami — the actual chain-exercising call. The
    //    response body (whether JSON or SSE-framed) must contain the
    //    authenticated person's name. If the auth context were lost
    //    anywhere between middleware insertion and tool-body read,
    //    we'd see "missing authenticated person" instead.
    let call_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", &bearer)
                .header("mcp-session-id", &session_id)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"whoami","arguments":{}}}"#,
                ))
                .unwrap(),
        )
        .await
        .expect("tools/call request");
    assert_eq!(call_resp.status(), StatusCode::OK, "tools/call must 200");
    let bytes = to_bytes(call_resp.into_body(), 32 * 1024).await.unwrap();
    let body_str = String::from_utf8_lossy(&bytes);
    assert!(
        body_str.contains("Alice"),
        "whoami must echo the authenticated person's name end-to-end \
         (chain: middleware → axum Parts → rmcp RequestContext → tool body); \
         got: {body_str}"
    );
    assert!(
        !body_str.contains("missing authenticated person"),
        "the redacted auth-context-missing error must not appear; got: {body_str}"
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
