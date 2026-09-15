// End-to-end tests for session reattachment on the production `mcp::router`.
//
// A fresh router over the same database stands in for a restarted server: the
// new router's session manager starts empty, so any session id from the first
// router is unknown to it. Dropping the first router does not stop its
// workers, because their spawned tasks own the manager, so no test asserts
// that an old session is gone.

use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use fewd_lib::services::mcp_token_service::McpTokenService;
use fewd_lib::services::person_service::PersonService;
use fewd_lib::{dto::CreatePersonDto, mcp};
use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection};
use tower::ServiceExt;

const WHOAMI: &str =
    r#"{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"whoami","arguments":{}}}"#;

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
    (db, format!("Bearer {}", issued.plaintext))
}

fn request(
    method: &str,
    bearer: Option<&str>,
    session_id: Option<&str>,
    body: &str,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri("/")
        .header("host", "localhost")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(bearer) = bearer {
        builder = builder.header("authorization", bearer);
    }
    if let Some(id) = session_id {
        builder = builder.header("mcp-session-id", id);
    }
    builder.body(Body::from(body.to_owned())).unwrap()
}

async fn initialize(app: &Router, bearer: &str) -> String {
    let init = r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"reattach-test","version":"0"}}}"#;
    let response = app
        .clone()
        .oneshot(request("POST", Some(bearer), None, init))
        .await
        .expect("initialize request");
    assert_eq!(response.status(), StatusCode::OK, "initialize must 200");
    let id = response
        .headers()
        .get("mcp-session-id")
        .expect("initialize sets mcp-session-id")
        .to_str()
        .expect("session id is ASCII")
        .to_owned();
    to_bytes(response.into_body(), 64 * 1024).await.unwrap();

    let ack = app
        .clone()
        .oneshot(request(
            "POST",
            Some(bearer),
            Some(&id),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ))
        .await
        .expect("initialized notification");
    assert_eq!(ack.status(), StatusCode::ACCEPTED);
    id
}

async fn call_whoami(app: &Router, bearer: Option<&str>, id: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(request("POST", bearer, Some(id), WHOAMI))
        .await
        .expect("tools/call request");
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

async fn delete(app: &Router, bearer: &str, id: &str) -> StatusCode {
    app.clone()
        .oneshot(request("DELETE", Some(bearer), Some(id), ""))
        .await
        .expect("delete request")
        .status()
}

fn assert_whoami_alice(status: StatusCode, body: &str) {
    assert_eq!(status, StatusCode::OK, "tools/call must 200; body: {body}");
    assert!(body.contains("\"result\""), "expected a result: {body}");
    assert!(!body.contains("\"error\""), "expected no error: {body}");
    assert!(body.contains("Alice"), "whoami must name Alice: {body}");
}

#[tokio::test]
async fn session_survives_router_rebuild() {
    let (db, bearer) = setup_db_with_token().await;
    let first = mcp::router(db.clone());
    let id = initialize(&first, &bearer).await;
    let (status, body) = call_whoami(&first, Some(&bearer), &id).await;
    assert_whoami_alice(status, &body);

    let restarted = mcp::router(db);
    let (status, body) = call_whoami(&restarted, Some(&bearer), &id).await;
    assert_whoami_alice(status, &body);
}

#[tokio::test]
async fn unknown_session_without_auth_is_rejected_before_rebuild() {
    let (db, _bearer) = setup_db_with_token().await;
    let app = mcp::router(db);
    let id = uuid::Uuid::new_v4().to_string();

    let (status, _body) = call_whoami(&app, None, &id).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn malformed_session_id_returns_404() {
    let (db, bearer) = setup_db_with_token().await;
    let app = mcp::router(db);
    let uppercase = uuid::Uuid::new_v4().to_string().to_uppercase();

    for id in ["not-a-uuid", uppercase.as_str()] {
        let (status, body) = call_whoami(&app, Some(&bearer), id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "id {id:?}; body: {body}");
    }
}

#[tokio::test]
async fn deleted_session_stays_gone_in_same_process() {
    let (db, bearer) = setup_db_with_token().await;
    let app = mcp::router(db);
    let id = initialize(&app, &bearer).await;

    assert_eq!(delete(&app, &bearer, &id).await, StatusCode::ACCEPTED);

    let (status, body) = call_whoami(&app, Some(&bearer), &id).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
}

#[tokio::test]
async fn delete_through_fresh_router_tombstones_id() {
    let (db, bearer) = setup_db_with_token().await;
    let first = mcp::router(db.clone());
    let id = initialize(&first, &bearer).await;

    let restarted = mcp::router(db);
    assert_eq!(delete(&restarted, &bearer, &id).await, StatusCode::ACCEPTED);

    let (status, body) = call_whoami(&restarted, Some(&bearer), &id).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
}

#[tokio::test]
async fn resume_on_rebuilt_session_returns_empty_stream() {
    let (db, bearer) = setup_db_with_token().await;
    let first = mcp::router(db.clone());
    let id = initialize(&first, &bearer).await;

    let restarted = mcp::router(db);
    // The slash marks a request-wise stream, which the rebuilt worker does
    // not have. A bare index names the standalone stream, which stays open.
    let resume = Request::builder()
        .method("GET")
        .uri("/")
        .header("host", "localhost")
        .header("authorization", &bearer)
        .header("accept", "text/event-stream")
        .header("mcp-session-id", &id)
        .header("last-event-id", "0/0")
        .body(Body::empty())
        .unwrap();
    let response = restarted.oneshot(resume).await.expect("resume request");
    assert_eq!(response.status(), StatusCode::OK);

    let body = tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), 64 * 1024),
    )
    .await
    .expect("the resumed stream ends")
    .unwrap();
    assert!(body.is_empty(), "expected an empty stream: {body:?}");
}
