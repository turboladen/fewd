// HTTP-level tests for recipe and drink-recipe writes: a broken domain
// rule must answer 400 with a message the UI can show, never a 500.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use fewd_lib::dto::{CreateDrinkRecipeDto, CreateRecipeDto};
use fewd_lib::routes::api_routes;
use fewd_lib::services::drink_recipe_service::DrinkRecipeService;
use fewd_lib::services::recipe_service::RecipeService;
use fewd_lib::AppState;
use migration::MigratorTrait;
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, IntoActiveModel, Set};
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

fn recipe_dto(name: &str) -> CreateRecipeDto {
    CreateRecipeDto {
        name: name.into(),
        description: None,
        source: "manual".into(),
        source_url: None,
        parent_recipe_id: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: 4,
        portion_size: None,
        instructions: "Mix and cook".into(),
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

#[tokio::test]
async fn update_recipe_with_out_of_range_rating_answers_400() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");

    let (status, body) = send_json(
        app(&db),
        "PUT",
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({ "rating": 7 }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("1 to 5"), "{body}");
}

#[tokio::test]
async fn update_recipe_with_unrecognized_time_unit_answers_400() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");

    let (status, body) = send_json(
        app(&db),
        "PUT",
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({ "total_time": { "value": 3, "unit": "sols" } }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    for fragment in ["total_time", "'sols'", "minutes", "hours", "days"] {
        assert!(message(&body).contains(fragment), "{fragment:?} in {body}");
    }
}

#[tokio::test]
async fn create_recipe_with_unrecognized_time_unit_answers_400() {
    let db = setup_db().await;
    let mut body = serde_json::to_value(recipe_dto("Pasta")).expect("dto serializes");
    body["prep_time"] = serde_json::json!({ "value": 10, "unit": "fortnights" });

    let (status, response) = send_json(app(&db), "POST", "/api/recipes", body).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert!(message(&response).contains("prep_time"), "{response}");
    let all = RecipeService::get_all(&db).await.expect("list recipes");
    assert!(all.is_empty(), "nothing may be created");
}

#[tokio::test]
async fn name_only_edit_resending_a_legacy_time_unit_succeeds() {
    // The web form resends every time field on each save, including a
    // stored unit it cannot represent, so validating the untouched field
    // would make this recipe impossible to rename.
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");
    let legacy_total = r#"{"value":3,"unit":"fortnights"}"#;
    let mut row = recipe.into_active_model();
    row.total_time = Set(Some(legacy_total.to_string()));
    let recipe = row.update(&db).await.expect("seed legacy unit");

    let (status, body) = send_json(
        app(&db),
        "PUT",
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({
            "name": "Pasta Night",
            "total_time": { "value": 3, "unit": "fortnights" },
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "Pasta Night");
    assert_eq!(
        body["total_time"], legacy_total,
        "legacy value is untouched"
    );
    assert!(body["total_minutes"].is_null());
}

#[tokio::test]
async fn update_drink_recipe_with_out_of_range_rating_answers_400() {
    let db = setup_db().await;
    let drink = DrinkRecipeService::create(
        &db,
        CreateDrinkRecipeDto {
            name: "Negroni".into(),
            description: None,
            source: "manual".into(),
            source_url: None,
            servings: 1,
            instructions: "Stir".into(),
            ingredients: vec![],
            technique: None,
            glassware: None,
            garnish: None,
            tags: vec![],
            notes: None,
            icon: None,
            is_non_alcoholic: None,
        },
    )
    .await
    .expect("seed drink");

    let (status, body) = send_json(
        app(&db),
        "PUT",
        &format!("/api/drink-recipes/{}", drink.id),
        serde_json::json!({ "rating": 0 }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("1 to 5"), "{body}");
}
