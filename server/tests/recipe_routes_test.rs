// HTTP-level tests for recipe and drink-recipe writes: a broken domain
// rule must answer 400 with a message the UI can show, never a 500.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use fewd_lib::dto::{CreateDrinkRecipeDto, CreateRecipeDto, IngredientAmountDto, IngredientDto};
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
async fn enhance_recipe_answers_text_and_injection_count() {
    let db = setup_db().await;
    let mut dto = recipe_dto("Guacamole");
    dto.instructions = "Mash the avocados with a fork.".into();
    dto.ingredients = vec![IngredientDto {
        name: "ripe avocados".into(),
        prep: None,
        amount: IngredientAmountDto::Single { value: 2.0 },
        unit: String::new(),
        notes: None,
        or_alternative: None,
    }];
    let recipe = RecipeService::create(&db, dto).await.expect("seed recipe");

    let (status, body) = send_json(
        app(&db),
        "POST",
        &format!("/api/recipes/{}/enhance", recipe.id),
        serde_json::json!({}),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        serde_json::json!({
            "enhanced_text": "Mash the **2 avocados** with a fork.",
            "injection_count": 1,
        })
    );
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
async fn update_recipe_with_nonpositive_servings_answers_400() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");

    let (status, body) = send_json(
        app(&db),
        "PUT",
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({ "servings": 0 }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("servings"), "{body}");
}

#[tokio::test]
async fn update_missing_recipe_with_nonpositive_servings_answers_400() {
    // The servings check runs before the recipe lookup, so a bad count is
    // reported even when the id does not exist.
    let db = setup_db().await;

    let (status, body) = send_json(
        app(&db),
        "PUT",
        "/api/recipes/no-such-recipe",
        serde_json::json!({ "servings": 0 }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("servings"), "{body}");
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

#[tokio::test]
async fn create_recipe_with_nonpositive_servings_answers_400() {
    let db = setup_db().await;
    for servings in [0, -1] {
        let mut body = serde_json::to_value(recipe_dto("Pasta")).expect("dto serializes");
        body["servings"] = serde_json::json!(servings);

        let (status, response) = send_json(app(&db), "POST", "/api/recipes", body).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert!(message(&response).contains("servings"), "{response}");
    }
    let all = RecipeService::get_all(&db).await.expect("list recipes");
    assert!(all.is_empty(), "nothing may be created");
}

#[tokio::test]
async fn scale_preview_of_a_stored_zero_serving_recipe_answers_400() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");
    // The row is written directly, because create rejects a count below 1.
    let mut row = recipe.into_active_model();
    row.servings = Set(0);
    let recipe = row.update(&db).await.expect("force servings to 0");

    let (status, body) = send_json(
        app(&db),
        "POST",
        &format!("/api/recipes/{}/scale", recipe.id),
        serde_json::json!({ "new_servings": 4 }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("servings"), "{body}");
}

#[tokio::test]
async fn scale_preview_to_nonpositive_servings_answers_400() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");

    for new_servings in [0, -1] {
        let (status, body) = send_json(
            app(&db),
            "POST",
            &format!("/api/recipes/{}/scale", recipe.id),
            serde_json::json!({ "new_servings": new_servings }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
}

fn meal_body(recipe_id: &str, servings_count: f64) -> serde_json::Value {
    serde_json::json!({
        "date": "2025-06-10",
        "meal_type": "Dinner",
        "order_index": 2,
        "servings": [{
            "food_type": "recipe",
            "person_id": "p1",
            "recipe_id": recipe_id,
            "servings_count": servings_count,
            "notes": null,
        }],
    })
}

#[tokio::test]
async fn meal_writes_with_nonpositive_servings_count_answer_400() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");
    let (status, created) =
        send_json(app(&db), "POST", "/api/meals", meal_body(&recipe.id, 1.0)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let meal_uri = format!("/api/meals/{}", created["id"].as_str().expect("meal id"));

    for count in [0.0, -1.0] {
        for (method, uri) in [("POST", "/api/meals"), ("PUT", meal_uri.as_str())] {
            let (status, body) =
                send_json(app(&db), method, uri, meal_body(&recipe.id, count)).await;

            assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {count}: {body}");
            assert!(
                message(&body).starts_with("servings_count"),
                "{method} {count}: {body}"
            );
        }
    }
}

#[tokio::test]
async fn meal_template_writes_with_nonpositive_servings_count_answer_400() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");
    let template_body = |count: f64| {
        let mut body = meal_body(&recipe.id, count);
        body["name"] = serde_json::json!("Weeknight Pasta");
        body
    };
    let (status, created) =
        send_json(app(&db), "POST", "/api/meal-templates", template_body(1.0)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let template_uri = format!(
        "/api/meal-templates/{}",
        created["id"].as_str().expect("template id")
    );

    for count in [0.0, -1.0] {
        for (method, uri) in [
            ("POST", "/api/meal-templates"),
            ("PUT", template_uri.as_str()),
        ] {
            let (status, body) = send_json(app(&db), method, uri, template_body(count)).await;

            assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {count}: {body}");
            assert!(
                message(&body).starts_with("servings_count"),
                "{method} {count}: {body}"
            );
        }
    }
}

#[tokio::test]
async fn meal_writes_with_a_malformed_date_answer_400() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");
    let (status, created) =
        send_json(app(&db), "POST", "/api/meals", meal_body(&recipe.id, 1.0)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let meal_uri = format!("/api/meals/{}", created["id"].as_str().expect("meal id"));

    for (method, uri) in [("POST", "/api/meals"), ("PUT", meal_uri.as_str())] {
        let mut body = meal_body(&recipe.id, 1.0);
        body["date"] = serde_json::json!("06/10/2025");
        let (status, response) = send_json(app(&db), method, uri, body).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{method}: {response}");
        assert!(
            message(&response).starts_with("date"),
            "{method}: {response}"
        );
    }
}

#[tokio::test]
async fn markdown_import_drops_a_time_with_an_unrecognized_unit() {
    let db = setup_db().await;
    let markdown =
        "# Slow Stew\nPrep time: 2 fortnights\nCook time: 3 hours\n\n## Instructions\nSimmer";

    let (status, body) = send_json(
        app(&db),
        "POST",
        "/api/recipes/import/markdown",
        serde_json::json!({ "markdown": markdown }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body["prep_time"].is_null(), "{body}");
    assert_eq!(body["cook_time"], r#"{"value":3,"unit":"hours"}"#, "{body}");
}

#[tokio::test]
async fn markdown_import_reads_abbreviated_time_units() {
    let db = setup_db().await;
    let markdown = "# Braise\nCook time: 2 h\n\n## Instructions\nBraise";

    let (status, body) = send_json(
        app(&db),
        "POST",
        "/api/recipes/import/markdown",
        serde_json::json!({ "markdown": markdown }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["cook_time"], r#"{"value":2,"unit":"hours"}"#, "{body}");
    assert_eq!(body["total_minutes"], 120, "{body}");
}
