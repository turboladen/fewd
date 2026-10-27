// Every recipe write path must agree on what a usable recipe is: one whose
// instructions hold more than whitespace. A person's blank write answers 400
// with a message naming the field. A blank from the model is repaired
// instead, because its tokens are already spent.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use fewd_lib::dto::CreateRecipeDto;
use fewd_lib::entities::recipe;
use fewd_lib::routes::api_routes;
use fewd_lib::services::recipe_parser::RecipeParser;
use fewd_lib::services::recipe_service::{sanitize_generated_recipe, RecipeService};
use fewd_lib::AppState;
use migration::MigratorTrait;
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, EntityTrait, Set};
use tower::ServiceExt;

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite connects");
    migration::Migrator::up(&db, None)
        .await
        .expect("migrations run on empty DB");
    db
}

fn app(db: &DatabaseConnection) -> Router {
    Router::new()
        .nest("/api", api_routes())
        .with_state(AppState { db: db.clone() })
}

fn recipe_dto(instructions: &str) -> CreateRecipeDto {
    CreateRecipeDto {
        name: "Pasta".into(),
        description: None,
        source: "manual".into(),
        source_url: None,
        parent_recipe_id: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: 4,
        portion_size: None,
        instructions: instructions.into(),
        ingredients: vec![],
        nutrition_per_serving: None,
        tags: vec![],
        notes: None,
        icon: None,
    }
}

// Send a JSON request and hand back the status plus the parsed body.
async fn send_json(
    app: Router,
    method: &str,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds");
    let response = app.oneshot(request).await.expect("router responds");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn message(body: &serde_json::Value) -> &str {
    body["message"].as_str().unwrap_or_default()
}

async fn recipe_count(db: &DatabaseConnection) -> usize {
    recipe::Entity::find()
        .all(db)
        .await
        .expect("recipes load")
        .len()
}

#[tokio::test]
async fn post_with_blank_instructions_answers_400() {
    let db = setup_db().await;
    for blank in ["", "   ", "\t\n"] {
        let body = serde_json::to_value(recipe_dto(blank)).expect("dto serializes");
        let (status, body) = send_json(app(&db), "POST", "/api/recipes", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{blank:?}: {body}");
        assert!(message(&body).contains("instructions"), "{body}");
    }
    assert_eq!(recipe_count(&db).await, 0, "nothing is created");
}

#[tokio::test]
async fn put_with_blank_instructions_answers_400_and_keeps_the_text() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Boil the pasta."))
        .await
        .expect("seed recipe");

    let (status, body) = send_json(
        app(&db),
        "PUT",
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({ "instructions": "  " }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("instructions"), "{body}");
    let row = recipe::Entity::find_by_id(recipe.id)
        .one(&db)
        .await
        .expect("lookup succeeds")
        .expect("recipe exists");
    assert_eq!(row.instructions, "Boil the pasta.");
}

#[tokio::test]
async fn put_without_instructions_on_a_row_stored_blank_succeeds() {
    // A row written outside the service can hold blank instructions.
    // Updating its other fields must not require fixing them first.
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Boil the pasta."))
        .await
        .expect("seed recipe");
    let mut row: recipe::ActiveModel = recipe.into();
    row.instructions = Set(String::new());
    let recipe = row.update(&db).await.expect("blank the instructions");

    let (status, body) = send_json(
        app(&db),
        "PUT",
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({ "servings": 6 }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["servings"], 6, "{body}");
}

#[tokio::test]
async fn markdown_import_without_instructions_names_the_heading() {
    let db = setup_db().await;
    for markdown in [
        "# Pasta\n\n## Ingredients\n- 1 lb pasta\n",
        "# Pasta\n\n## Ingredients\n- 1 lb pasta\n\n## Instructions\n\n",
    ] {
        let (status, body) = send_json(
            app(&db),
            "POST",
            "/api/recipes/import/markdown",
            serde_json::json!({ "markdown": markdown }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{markdown:?}: {body}");
        assert!(message(&body).contains("## Instructions"), "{body}");
    }
    assert_eq!(recipe_count(&db).await, 0, "nothing is imported");
}

#[test]
fn parse_markdown_without_instructions_names_the_heading() {
    let err = RecipeParser::parse_markdown("# Pasta\n\n## Ingredients\n- 1 lb pasta\n")
        .expect_err("a recipe without instructions is rejected");
    assert!(err.contains("## Instructions"), "{err}");
}

#[test]
fn parse_markdown_reports_a_missing_name_first() {
    let err = RecipeParser::parse_markdown("Just some text\nwithout a heading")
        .expect_err("a nameless recipe is rejected");
    assert!(err.contains("name"), "{err}");
}

#[test]
fn sanitize_generated_recipe_fills_blank_instructions() {
    for blank in ["", "   ", "\n"] {
        let mut dto = recipe_dto(blank);
        sanitize_generated_recipe(&mut dto);
        assert!(
            !dto.instructions.trim().is_empty(),
            "{blank:?} is replaced with text create accepts"
        );
    }

    let mut dto = recipe_dto("  Boil the pasta.\n");
    sanitize_generated_recipe(&mut dto);
    assert_eq!(
        dto.instructions, "  Boil the pasta.\n",
        "real text is kept as is"
    );
}

#[test]
fn a_model_recipe_with_null_or_missing_instructions_is_repaired() {
    let base = serde_json::json!({
        "name": "Pasta",
        "description": null,
        "source": "url_import",
        "source_url": null,
        "parent_recipe_id": null,
        "prep_time": null,
        "cook_time": null,
        "total_time": null,
        "servings": 4,
        "portion_size": null,
        "ingredients": [],
        "nutrition_per_serving": null,
        "tags": [],
        "notes": null,
        "icon": null,
    });
    let mut with_null = base.clone();
    with_null["instructions"] = serde_json::Value::Null;
    for payload in [with_null, base] {
        let mut dto: CreateRecipeDto = serde_json::from_str(&payload.to_string())
            .unwrap_or_else(|e| panic!("model output parses: {e}: {payload}"));
        sanitize_generated_recipe(&mut dto);
        assert!(!dto.instructions.trim().is_empty(), "{payload}");
    }
}

#[tokio::test]
async fn post_with_null_instructions_answers_400() {
    let db = setup_db().await;
    let mut body = serde_json::to_value(recipe_dto("Boil.")).expect("dto serializes");
    body["instructions"] = serde_json::Value::Null;
    let (status, body) = send_json(app(&db), "POST", "/api/recipes", body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("instructions"), "{body}");
}

#[tokio::test]
async fn a_sanitized_generated_recipe_saves() {
    let db = setup_db().await;
    let mut dto = recipe_dto("");
    dto.source = "url_import".into();
    sanitize_generated_recipe(&mut dto);
    RecipeService::create(&db, dto)
        .await
        .expect("a repaired model recipe saves");
}

#[test]
fn sanitize_generated_recipe_drops_blank_optional_text() {
    let mut dto = recipe_dto("Boil the pasta.");
    dto.description = Some(String::new());
    dto.notes = Some("  ".into());
    dto.icon = Some("\n".into());
    sanitize_generated_recipe(&mut dto);
    assert_eq!(dto.description, None);
    assert_eq!(dto.notes, None);
    assert_eq!(dto.icon, None);
}
