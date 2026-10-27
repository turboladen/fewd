// End-to-end tests for the recipe write tools' clear and blank rules.
//
// These drive the production `mcp::router` over HTTP, so an explicit `null`
// has to survive rmcp's argument handling to reach the tool. Each test then
// re-reads the row through `RecipeService`, so a tool that answers success
// without writing is caught.
//
// Mirrors the harness in `mcp_unknown_fields_test.rs`.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use fewd_lib::dto::{
    CreatePersonDto, CreateRecipeDto, IngredientAmountDto, IngredientDto, PortionSizeDto,
    TimeValueDto,
};
use fewd_lib::entities::recipe;
use fewd_lib::mcp;
use fewd_lib::services::mcp_token_service::McpTokenService;
use fewd_lib::services::person_service::PersonService;
use fewd_lib::services::recipe_service::RecipeService;
use migration::MigratorTrait;
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};
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

fn mcp_request(bearer: &str, session_id: Option<&str>, body: String) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/")
        .header("host", "localhost")
        .header("authorization", bearer)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(id) = session_id {
        builder = builder.header("mcp-session-id", id);
    }
    builder.body(Body::from(body)).unwrap()
}

// Run `initialize` + `notifications/initialized` and return the session id
// the remaining requests must carry.
async fn handshake(app: &Router, bearer: &str) -> String {
    let init_body = json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1,
        "params": {
            "protocolVersion": "2025-11-05",
            "capabilities": {},
            "clientInfo": { "name": "recipe-update-test", "version": "0" }
        }
    });
    let init_resp = app
        .clone()
        .oneshot(mcp_request(bearer, None, init_body.to_string()))
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

    let notif = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let notif_resp = app
        .clone()
        .oneshot(mcp_request(bearer, Some(&session_id), notif.to_string()))
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

// A connected client: the router, its bearer header, and a live session.
struct Client {
    app: Router,
    bearer: String,
    session_id: String,
}

impl Client {
    async fn connect(db: &DatabaseConnection, token: &str) -> Self {
        let app = mcp::router(db.clone());
        let bearer = format!("Bearer {token}");
        let session_id = handshake(&app, &bearer).await;
        Self {
            app,
            bearer,
            session_id,
        }
    }

    // Call a tool and return the raw response body.
    async fn call(&self, tool: &str, arguments: Value) -> String {
        let body = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "id": 2,
            "params": { "name": tool, "arguments": arguments }
        });
        let resp = self
            .app
            .clone()
            .oneshot(mcp_request(
                &self.bearer,
                Some(&self.session_id),
                body.to_string(),
            ))
            .await
            .expect("rpc request");
        assert_eq!(resp.status(), StatusCode::OK, "rpc must 200");
        let bytes = to_bytes(resp.into_body(), 256 * 1024).await.unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

fn is_tool_error(body: &str) -> bool {
    body.contains("\"isError\":true")
}

fn ingredient(name: &str) -> IngredientDto {
    IngredientDto {
        name: name.into(),
        prep: None,
        amount: IngredientAmountDto::Single { value: 1.0 },
        unit: "cup".into(),
        notes: None,
        or_alternative: None,
    }
}

fn minutes(value: i32) -> Option<TimeValueDto> {
    Some(TimeValueDto {
        value,
        unit: "minutes".into(),
    })
}

// A recipe with every field `update_recipe` can clear set to a value.
async fn seed_full_recipe(db: &DatabaseConnection) -> recipe::Model {
    RecipeService::create(
        db,
        CreateRecipeDto {
            name: "Beef Taco Bowls".into(),
            description: Some("Weeknight staple".into()),
            source: "manual".into(),
            source_url: None,
            parent_recipe_id: None,
            prep_time: minutes(10),
            cook_time: minutes(20),
            total_time: minutes(30),
            servings: 4,
            portion_size: Some(PortionSizeDto {
                value: 1.0,
                unit: "bowl".into(),
            }),
            instructions: "Brown the beef.".into(),
            ingredients: vec![ingredient("ground beef")],
            nutrition_per_serving: None,
            tags: vec![],
            notes: Some("Double the chili powder".into()),
            icon: Some("🌮".into()),
        },
    )
    .await
    .expect("seed recipe")
}

async fn reload(db: &DatabaseConnection, slug: &str) -> recipe::Model {
    RecipeService::get_by_slug(db, slug.into())
        .await
        .expect("lookup succeeds")
        .expect("recipe exists")
}

async fn recipe_count(db: &DatabaseConnection) -> usize {
    recipe::Entity::find()
        .all(db)
        .await
        .expect("recipes load")
        .len()
}

#[tokio::test]
async fn update_recipe_null_clears_each_optional_field() {
    let (db, token) = setup_db_with_token().await;
    let seeded = seed_full_recipe(&db).await;
    let client = Client::connect(&db, &token).await;

    let body = client
        .call(
            "update_recipe",
            json!({
                "slug": seeded.slug,
                "description": null,
                "notes": null,
                "icon": null,
                "portion_size": null,
                "prep_time": null,
                "cook_time": null,
                "total_time": null,
            }),
        )
        .await;
    assert!(!is_tool_error(&body), "{body}");

    let after = reload(&db, &seeded.slug).await;
    assert_eq!(after.description, None);
    assert_eq!(after.notes, None);
    assert_eq!(after.icon, None);
    assert_eq!(after.portion_size, None);
    assert_eq!(after.prep_time, None);
    assert_eq!(after.cook_time, None);
    assert_eq!(after.total_time, None);
    assert_eq!(after.total_minutes, None, "the search column clears too");
    assert_eq!(after.instructions, seeded.instructions);
    assert_eq!(after.name, seeded.name);
}

#[tokio::test]
async fn update_recipe_omitted_fields_survive() {
    let (db, token) = setup_db_with_token().await;
    let seeded = seed_full_recipe(&db).await;
    let client = Client::connect(&db, &token).await;

    let body = client
        .call(
            "update_recipe",
            json!({"slug": seeded.slug, "name": "Taco Bowls"}),
        )
        .await;
    assert!(!is_tool_error(&body), "{body}");

    let after = reload(&db, &seeded.slug).await;
    assert_eq!(after.name, "Taco Bowls");
    assert_eq!(after.description, seeded.description);
    assert_eq!(after.notes, seeded.notes);
    assert_eq!(after.icon, seeded.icon);
    assert_eq!(after.portion_size, seeded.portion_size);
    assert_eq!(after.prep_time, seeded.prep_time);
    assert_eq!(after.cook_time, seeded.cook_time);
    assert_eq!(after.total_time, seeded.total_time);
}

#[tokio::test]
async fn update_recipe_blank_clearable_string_is_a_tool_error() {
    let (db, token) = setup_db_with_token().await;
    let seeded = seed_full_recipe(&db).await;
    let client = Client::connect(&db, &token).await;

    for field in ["description", "notes", "icon"] {
        for blank in ["", "   "] {
            let body = client
                .call("update_recipe", json!({"slug": seeded.slug, field: blank}))
                .await;
            assert!(is_tool_error(&body), "{field} = {blank:?}: {body}");
            assert!(
                body.contains(field) && body.contains("null"),
                "the error names {field} and the null clear: {body}"
            );
        }
    }

    let after = reload(&db, &seeded.slug).await;
    assert_eq!(after.description, seeded.description);
    assert_eq!(after.notes, seeded.notes);
    assert_eq!(after.icon, seeded.icon);
}

#[tokio::test]
async fn update_recipe_blank_instructions_is_a_tool_error() {
    let (db, token) = setup_db_with_token().await;
    let seeded = seed_full_recipe(&db).await;
    let client = Client::connect(&db, &token).await;

    for blank in ["", "   ", "\t\n"] {
        let body = client
            .call(
                "update_recipe",
                json!({"slug": seeded.slug, "instructions": blank}),
            )
            .await;
        assert!(is_tool_error(&body), "{blank:?}: {body}");
        assert!(body.contains("instructions"), "{body}");
    }

    let after = reload(&db, &seeded.slug).await;
    assert_eq!(after.instructions, "Brown the beef.");
}

#[tokio::test]
async fn create_recipe_blank_instructions_is_a_tool_error() {
    let (db, token) = setup_db_with_token().await;
    let client = Client::connect(&db, &token).await;

    for blank in ["", "   "] {
        let body = client
            .call(
                "create_recipe",
                json!({
                    "name": "Beef Taco Bowls",
                    "source": "manual",
                    "servings": 4,
                    "instructions": blank,
                    "ingredients": [],
                }),
            )
            .await;
        assert!(is_tool_error(&body), "{blank:?}: {body}");
        assert!(body.contains("instructions"), "{body}");
    }

    assert_eq!(recipe_count(&db).await, 0, "nothing is created");
}

#[tokio::test]
async fn adapt_recipe_from_a_parent_without_instructions_asks_for_them() {
    let (db, token) = setup_db_with_token().await;
    // Every write path rejects blank instructions, so write the row
    // directly to reach the variant builder's own guard.
    let mut parent: recipe::ActiveModel = seed_full_recipe(&db).await.into();
    parent.instructions = Set(String::new());
    let parent = parent.update(&db).await.expect("blank the instructions");
    let before = recipe_count(&db).await;
    let client = Client::connect(&db, &token).await;

    let change = json!([{"op": "add", "ingredient":
        {"name": "jalapeño", "amount": {"kind": "single", "value": 1.0}, "unit": ""}}]);
    let body = client
        .call(
            "adapt_recipe",
            json!({
                "parent_recipe_slug": parent.slug,
                "name": "Spicy Taco Bowls",
                "ingredient_changes": change,
            }),
        )
        .await;
    assert!(is_tool_error(&body), "{body}");
    assert!(
        body.contains("would have no instructions"),
        "the error says why and what to send: {body}"
    );
    assert_eq!(recipe_count(&db).await, before, "nothing is saved");

    let body = client
        .call(
            "adapt_recipe",
            json!({
                "parent_recipe_slug": parent.slug,
                "name": "Spicy Taco Bowls",
                "ingredient_changes": change,
                "instructions": "Brown the beef with the jalapeño.",
            }),
        )
        .await;
    assert!(
        !is_tool_error(&body),
        "sending instructions recovers: {body}"
    );
    assert_eq!(recipe_count(&db).await, before + 1);
}
