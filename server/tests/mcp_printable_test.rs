//! End-to-end test for `get_meal_planner_printable` (fewd-8yc).
//!
//! Unit tests in `services::printable_service` cover the renderer's logic
//! against synthetic `meal::Model`s and recipe maps. This file exercises
//! the full chain — bearer auth → MCP handshake → tool dispatch → DB
//! reads via `MealService` + `RecipeService` + `PersonService` → renderer
//! → response body — so the wiring can't silently break the surface even
//! if the renderer's unit tests still pass.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use fewd_lib::dto::{
    CreateMealDto, CreatePersonDto, CreateRecipeDto, IngredientAmountDto, IngredientDto,
    PersonServingDto, TimeValueDto,
};
use fewd_lib::mcp;
use fewd_lib::services::mcp_token_service::McpTokenService;
use fewd_lib::services::meal_service::MealService;
use fewd_lib::services::person_service::PersonService;
use fewd_lib::services::recipe_service::RecipeService;
use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection};
use tower::ServiceExt;

async fn setup_seeded_db_with_token() -> (DatabaseConnection, String, String) {
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

    let gyoza = RecipeService::create(
        &db,
        CreateRecipeDto {
            name: "Pan-Fried Gyoza".into(),
            description: Some("Crispy yaki-style gyoza with soy-vinegar.".into()),
            source: "manual".into(),
            source_url: None,
            parent_recipe_id: None,
            prep_time: None,
            cook_time: None,
            total_time: Some(TimeValueDto {
                value: 15,
                unit: "min".into(),
            }),
            servings: 4,
            portion_size: None,
            instructions: "Pan-fry until golden.".into(),
            ingredients: vec![IngredientDto {
                name: "frozen gyoza".into(),
                prep: None,
                amount: IngredientAmountDto::Single { value: 12.0 },
                unit: "piece".into(),
                notes: None,
                or_alternative: None,
            }],
            nutrition_per_serving: None,
            tags: vec!["dinner".into()],
            notes: None,
            icon: Some("🥟".into()),
        },
    )
    .await
    .expect("create gyoza");

    MealService::create(
        &db,
        CreateMealDto {
            date: "2026-05-11".into(),
            meal_type: "Dinner".into(),
            order_index: 2,
            servings: vec![PersonServingDto::Recipe {
                person_id: alice.id.clone(),
                recipe_id: gyoza.id.clone(),
                servings_count: 1.0,
                notes: Some("Cleo: plain gyoza + rice".into()),
            }],
        },
    )
    .await
    .expect("create monday dinner");

    let issued = McpTokenService::provision(&db, &alice.id)
        .await
        .expect("provision token");

    (db, issued.plaintext, gyoza.slug)
}

/// Drive the full MCP handshake then call `get_meal_planner_printable`.
/// Verifies that the seeded meal's recipe name + per-person note land
/// in the rendered HTML and that the response uses the canonical print
/// CSS rules (single-page constraint pinned here as a regression guard).
#[tokio::test]
async fn get_meal_planner_printable_renders_seeded_week_through_mcp_chain() {
    let (db, token, _gyoza_slug) = setup_seeded_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");

    // 1) initialize
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
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-11-05","capabilities":{},"clientInfo":{"name":"printable-test","version":"0"}}}"#,
                ))
                .unwrap(),
        )
        .await
        .expect("init request");
    assert_eq!(init_resp.status(), StatusCode::OK);
    let session_id = init_resp
        .headers()
        .get("mcp-session-id")
        .expect("session id")
        .to_str()
        .expect("ascii")
        .to_string();
    drop(to_bytes(init_resp.into_body(), 32 * 1024).await.unwrap());

    // 2) notifications/initialized
    let notif = app
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
        .expect("notif");
    assert!(notif.status().is_success());
    drop(to_bytes(notif.into_body(), 32 * 1024).await.unwrap());

    // 3) tools/call get_meal_planner_printable — exercise overlays so
    //    we cover the schema's optional fields (and confirm they don't
    //    cause deserialization issues end-to-end).
    let call_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 2,
        "params": {
            "name": "get_meal_planner_printable",
            "arguments": {
                "start_date": "2026-05-11",
                "end_date": "2026-05-17",
                "week_theme": "Freezer Clear",
                "use_up_notes": ["Frozen gyoza → Monday"],
                "dont_forget": [
                    {"prefix": "Mon:", "body": "Girl Scouts at 6, gyoza ready by 7:15"}
                ],
                "day_overlays": [
                    {"date": "2026-05-11", "tag": "Time Crunch",
                     "prep_notes": ["Defrost gyoza at lunch"]}
                ],
                "foot_note": "Back-friendly week"
            }
        }
    })
    .to_string();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", &bearer)
                .header("mcp-session-id", &session_id)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(call_body))
                .unwrap(),
        )
        .await
        .expect("printable call");
    assert_eq!(resp.status(), StatusCode::OK, "tools/call must 200");

    let bytes = to_bytes(resp.into_body(), 256 * 1024).await.unwrap();
    let body = String::from_utf8_lossy(&bytes);

    // The response is JSON-RPC (potentially SSE-framed), so the rendered
    // HTML is escaped inside the "text" content field. Substring checks
    // still work because the escape leaves angle brackets as `<`
    // OR raw `<`, depending on rmcp's serializer settings. Cover both.
    let raw_or_escaped = |needle: &str, escaped_needle: &str| -> bool {
        body.contains(needle) || body.contains(escaped_needle)
    };

    assert!(
        raw_or_escaped("<html", "\\u003chtml"),
        "response should contain HTML markup; body was: {body}"
    );
    assert!(
        body.contains("Pan-Fried Gyoza"),
        "seeded recipe name must appear: {body}"
    );
    assert!(
        body.contains("Cleo: plain gyoza + rice"),
        "per-person note from the seeded meal must appear: {body}"
    );
    assert!(
        body.contains("Freezer Clear"),
        "week_theme overlay must reach the header: {body}"
    );
    assert!(
        body.contains("Defrost gyoza at lunch"),
        "day_overlay prep_notes must appear: {body}"
    );
    assert!(
        body.contains("Girl Scouts at 6, gyoza ready by 7:15"),
        "dont_forget body must appear: {body}"
    );
    assert!(
        body.contains("Back-friendly week"),
        "explicit foot_note must override default: {body}"
    );
    // Single-page-fit CSS is load-bearing — pin its presence end-to-end
    // (the unit test covers the renderer; this confirms the template
    // and the chain don't strip / rewrite the rules along the way).
    assert!(
        raw_or_escaped("size: letter portrait", "size: letter portrait"),
        "print CSS @page rule must reach the client"
    );
}

/// 14-day cap rejection must reach the client as an actionable tool-level
/// error message — not a generic JSON-RPC failure.
#[tokio::test]
async fn get_meal_planner_printable_rejects_oversized_range() {
    let (db, token, _) = setup_seeded_db_with_token().await;
    let app = mcp::router(db);
    let bearer = format!("Bearer {token}");

    // initialize + initialized
    let init = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", &bearer)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-11-05","capabilities":{},"clientInfo":{"name":"printable-cap-test","version":"0"}}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let session_id = init
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    drop(to_bytes(init.into_body(), 32 * 1024).await.unwrap());
    let n = app
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
        .unwrap();
    drop(to_bytes(n.into_body(), 32 * 1024).await.unwrap());

    let call_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 2,
        "params": {
            "name": "get_meal_planner_printable",
            "arguments": {
                "start_date": "2026-05-01",
                "end_date": "2026-06-15"
            }
        }
    })
    .to_string();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "localhost")
                .header("authorization", &bearer)
                .header("mcp-session-id", &session_id)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(call_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.to_lowercase().contains("printable")
            || body.contains("13-day cap")
            || body.contains("sheet"),
        "rejection message should explain the printable cap: {body}"
    );
}
