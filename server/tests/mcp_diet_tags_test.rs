//! End-to-end tests for the MCP diet-tag surface (fewd-08x).
//!
//! Unit tests in `mcp/handler.rs` pin the `DIET_TAGS` vocabulary and the
//! `list_diet_tags` tool body. What they DON'T cover is the full protocol
//! chain `middleware → rmcp StreamableHttpService → tool_handler` / the
//! `resources/*` handlers — `read_resource` had zero coverage before this.
//! These drive the production `mcp::router` through the handshake, then
//! exercise both the tool and the `fewd://diet-tags` resource over the wire.
//!
//! Mirrors the harness in `mcp_prompts_test.rs`.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
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

/// Run `initialize` + `notifications/initialized` and return the session id
/// the remaining requests must carry.
async fn handshake(app: &Router, bearer: &str) -> String {
    let init_body = r#"{
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1,
        "params": {
            "protocolVersion": "2025-11-05",
            "capabilities": {},
            "clientInfo": { "name": "diet-tags-test", "version": "0" }
        }
    }"#;
    let init_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", bearer)
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

    let notif_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", bearer)
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

    session_id
}

async fn post_rpc(app: &Router, bearer: &str, session_id: &str, body: &'static str) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", bearer)
                .header("mcp-session-id", session_id)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .expect("rpc request");
    assert_eq!(resp.status(), StatusCode::OK, "rpc must 200");
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `tools/list` advertises `list_diet_tags`, and `tools/call` returns the
/// canonical vocabulary (tags + meanings) so the LLM can translate a
/// person's free-form dietary goals into `search_recipes` tag filters.
#[tokio::test]
async fn list_diet_tags_advertised_and_returns_vocabulary_over_http() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let list_body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"tools/list","id":2}"#,
    )
    .await;
    assert!(
        list_body.contains("list_diet_tags"),
        "tools/list must advertise list_diet_tags; got: {list_body}"
    );

    let call_body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"list_diet_tags","arguments":{}}}"#,
    )
    .await;
    for tag in [
        "vegetarian",
        "gluten-free",
        "pescatarian",
        "low-carb",
        "keto",
    ] {
        assert!(
            call_body.contains(tag),
            "tools/call result must list the {tag:?} diet tag; got: {call_body}"
        );
    }
    assert!(
        !call_body.contains("\"isError\":true"),
        "listing the vocabulary must be a success result; got: {call_body}"
    );
}

/// `resources/list` advertises `fewd://diet-tags`, and `resources/read`
/// returns the Markdown vocabulary. This is the first coverage of the
/// `read_resource` routing for a second URI.
#[tokio::test]
async fn diet_tags_resource_advertised_and_readable_over_http() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let list_body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"resources/list","id":2}"#,
    )
    .await;
    assert!(
        list_body.contains("fewd://diet-tags"),
        "resources/list must advertise the diet-tags resource; got: {list_body}"
    );

    let read_body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"resources/read","id":3,"params":{"uri":"fewd://diet-tags"}}"#,
    )
    .await;
    assert!(
        read_body.contains("diet-tag vocabulary") && read_body.contains("gluten-free"),
        "resources/read must return the diet-tag Markdown; got: {read_body}"
    );
}

/// An unknown resource URI returns a JSON-RPC invalid_params error rather
/// than a 200 with empty contents — the `other =>` arm of `read_resource`.
#[tokio::test]
async fn read_resource_unknown_uri_returns_invalid_params() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"resources/read","id":4,"params":{"uri":"fewd://does-not-exist"}}"#,
    )
    .await;
    assert!(
        body.contains("\"error\"") && body.contains("-32602"),
        "an unknown resource uri must return invalid_params; got: {body}"
    );
    assert!(
        body.contains("fewd://does-not-exist"),
        "the error must echo the offending uri; got: {body}"
    );
    assert!(
        body.contains("resources/list"),
        "the error must point at resources/list so the caller can recover; got: {body}"
    );
}
