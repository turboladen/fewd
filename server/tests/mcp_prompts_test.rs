//! End-to-end test for the MCP *prompts* surface (fewd-3l2).
//!
//! Unit tests in `mcp/prompts/` cover the rendered body verbatim and the
//! prompt-router registration. What they DON'T cover is the full protocol
//! chain `middleware → rmcp StreamableHttpService → prompt_handler →
//! #[prompt] body`: that `prompts/list` actually advertises the prompt and
//! `prompts/get` returns the rendered message over the wire. This is the
//! automated stand-in for hand-verifying in Claude Desktop.
//!
//! Drives the production `mcp::router` through the MCP handshake
//! (`initialize` → `notifications/initialized`) before issuing
//! `prompts/list` / `prompts/get`, matching the sequence in
//! `mcp_auth_plumbing_test.rs`.

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
            "clientInfo": { "name": "prompts-test", "version": "0" }
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

/// `prompts/list` advertises `weekly_dinner_plan`, and `prompts/get` renders
/// the canonical workflow with the supplied context embedded and a non-Monday
/// `week_start_date` snapped to its Monday.
#[tokio::test]
async fn weekly_dinner_plan_lists_and_renders_over_http() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let list_body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"prompts/list","id":2}"#,
    )
    .await;
    assert!(
        list_body.contains("weekly_dinner_plan"),
        "prompts/list must advertise weekly_dinner_plan; got: {list_body}"
    );

    // Wednesday 2026-05-27 must snap to Monday 2026-05-25; the supplied
    // family_schedule and ingredients must be embedded verbatim.
    let get_body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"prompts/get","id":3,"params":{"name":"weekly_dinner_plan","arguments":{"week_start_date":"2026-05-27","family_schedule":"Wednesday is busy with aikido.","ingredients_to_use_up":"frozen Dover sole"}}}"#,
    )
    .await;

    assert!(
        get_body.contains("week of Monday 2026-05-25 through Sunday 2026-05-31"),
        "rendered prompt must snap to the week's Monday; got: {get_body}"
    );
    assert!(
        get_body.contains("Wednesday is busy with aikido."),
        "rendered prompt must embed family_schedule; got: {get_body}"
    );
    assert!(
        get_body.contains("frozen Dover sole"),
        "rendered prompt must embed the optional ingredients arg; got: {get_body}"
    );
    assert!(
        get_body.contains("get_meal_planner_printable"),
        "rendered prompt must carry the canonical workflow; got: {get_body}"
    );
}

/// A malformed `week_start_date` is rejected with an actionable error rather
/// than rendering a garbage week.
#[tokio::test]
async fn prompts_get_rejects_bad_date() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"prompts/get","id":4,"params":{"name":"weekly_dinner_plan","arguments":{"week_start_date":"next monday","family_schedule":"whatever"}}}"#,
    )
    .await;

    // Assert it's an actual JSON-RPC error envelope (invalid_params = -32602),
    // not a 200 with a rendered prompt that happens to contain these tokens.
    assert!(
        body.contains("\"error\"") && body.contains("-32602"),
        "bad date must return a JSON-RPC invalid_params error, not a rendered prompt; got: {body}"
    );
    assert!(
        body.contains("week_start_date") && body.contains("YYYY-MM-DD"),
        "the error must name the field and the expected format; got: {body}"
    );
}

/// A syntactically valid but extreme year (near chrono's representable range)
/// would overflow the week arithmetic. The handler must return a graceful
/// error envelope rather than panicking the request.
#[tokio::test]
async fn prompts_get_rejects_out_of_range_date() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"prompts/get","id":5,"params":{"name":"weekly_dinner_plan","arguments":{"week_start_date":"+262142-12-31","family_schedule":"whatever"}}}"#,
    )
    .await;

    assert!(
        body.contains("\"error\"") && body.contains("-32602"),
        "an out-of-range date must return a graceful invalid_params error, not panic; got: {body}"
    );
}

/// `family_schedule` is required; a whitespace-only value (which serde accepts
/// as "present") must be rejected with an actionable error rather than
/// rendering a blank schedule line.
#[tokio::test]
async fn prompts_get_rejects_blank_family_schedule() {
    let (db, token) = setup_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");
    let session_id = handshake(&app, &bearer).await;

    let body = post_rpc(
        &app,
        &bearer,
        &session_id,
        r#"{"jsonrpc":"2.0","method":"prompts/get","id":6,"params":{"name":"weekly_dinner_plan","arguments":{"week_start_date":"2026-05-25","family_schedule":"   "}}}"#,
    )
    .await;

    assert!(
        body.contains("\"error\"") && body.contains("-32602"),
        "a blank family_schedule must return invalid_params; got: {body}"
    );
    assert!(
        body.contains("family_schedule"),
        "the error must name the offending field; got: {body}"
    );
}
