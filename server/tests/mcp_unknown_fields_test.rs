// End-to-end tests for rejecting tool arguments a tool does not declare.
//
// Unit tests in `mcp/handler.rs` pin the message text and the schema guard.
// These drive the production `mcp::router` over HTTP to show the rejection
// reaches the client as a tool-level error (`isError: true`) and never as a
// JSON-RPC `-32602`, which most clients render without its message.
//
// Mirrors the harness in `mcp_prompts_test.rs`.

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

// Run `initialize` + `notifications/initialized` and return the session id
// the remaining requests must carry.
async fn handshake(app: &Router, bearer: &str) -> String {
    let init_body = r#"{
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1,
        "params": {
            "protocolVersion": "2025-11-05",
            "capabilities": {},
            "clientInfo": { "name": "unknown-fields-test", "version": "0" }
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

// The singular `instruction` is a typo for `instructions`. It must fail the
// call with the valid field names, rather than succeed after writing nothing.
#[tokio::test]
async fn update_recipe_misspelled_field_is_a_tool_level_error_over_http() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"update_recipe","arguments":{"slug":"beef-taco-bowls","instruction":"New steps"}}}"#,
    )
    .await;

    assert!(
        body.contains("\"isError\":true"),
        "an unknown field must produce a tool-level error; got: {body}"
    );
    assert!(
        !body.contains("-32602"),
        "the rejection must not be a JSON-RPC invalid_params error; got: {body}"
    );
    assert!(
        body.contains("unknown field `instruction`") && body.contains("`instructions`"),
        "the error must name the unknown field and the valid one; got: {body}"
    );
}

// `rating` is a real recipe attribute that update_recipe does not write. The
// error must send the caller to the tool that does.
#[tokio::test]
async fn update_recipe_rating_names_rate_recipe_over_http() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"update_recipe","arguments":{"slug":"beef-taco-bowls","rating":5}}}"#,
    )
    .await;

    assert!(
        body.contains("\"isError\":true"),
        "a redirected field must produce a tool-level error; got: {body}"
    );
    assert!(
        body.contains("Call rate_recipe to set a rating"),
        "the error must name rate_recipe; got: {body}"
    );
}
