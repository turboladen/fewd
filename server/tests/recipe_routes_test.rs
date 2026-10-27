// HTTP-level tests for write routes: a broken domain rule answers 400 with a
// message the UI can show, a missing id answers 404, never a 500, and an
// explicit null in an update clears the field.

use axum::body::{to_bytes, Body};
use axum::http::{HeaderValue, Request, StatusCode};
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

// A `Header` holds a request header's name and its raw value bytes, so a test
// can send a value that is not valid UTF-8.
type Header = (&'static str, &'static [u8]);

const JSON: Header = ("content-type", b"application/json");
const FORM: Header = ("content-type", b"application/x-www-form-urlencoded");
const TEXT: Header = ("content-type", b"text/plain;charset=UTF-8");
const MULTIPART: Header = ("content-type", b"multipart/form-data; boundary=XyZ");
const XRW: Header = ("x-requested-with", b"fetch");

// Send a request with exactly the given headers and body, and hand back the
// status plus the parsed body.
async fn send_raw(
    app: Router,
    method: &str,
    uri: &str,
    headers: &[Header],
    body: Vec<u8>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(
            *name,
            HeaderValue::from_bytes(value).expect("header value builds"),
        );
    }
    let request = builder.body(Body::from(body)).expect("request builds");
    let response = app.oneshot(request).await.expect("router responds");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

// Send a JSON request and hand back the status plus the parsed body.
async fn send_json(
    app: Router,
    method: &str,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    send_raw(app, method, uri, &[JSON], body.to_string().into_bytes()).await
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

// PUT `body` to `uri` and hand back the updated row, failing on any status
// but 200.
async fn put_ok(db: &DatabaseConnection, uri: &str, body: serde_json::Value) -> serde_json::Value {
    let (status, response) = send_json(app(db), "PUT", uri, body).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    response
}

// Read a stored time column back as (value, unit), or `None` for NULL.
fn stored_time(row: &serde_json::Value, field: &str) -> Option<(i64, String)> {
    let raw = row[field].as_str()?;
    let time: serde_json::Value = serde_json::from_str(raw).expect("stored time is JSON");
    Some((
        time["value"].as_i64().expect("value"),
        time["unit"].as_str().expect("unit").to_string(),
    ))
}

fn minutes(value: i64) -> Option<(i64, String)> {
    Some((value, "minutes".to_string()))
}

fn time_json(value: i32) -> serde_json::Value {
    serde_json::json!({ "value": value, "unit": "minutes" })
}

// Seed a recipe whose prep, cook, and total columns hold exactly the given
// minutes, NULL for `None`. Create would derive a missing total, so the
// columns are written directly.
async fn seed_times(
    db: &DatabaseConnection,
    prep: Option<i32>,
    cook: Option<i32>,
    total: Option<i32>,
) -> String {
    let recipe = RecipeService::create(db, recipe_dto("Timed"))
        .await
        .expect("seed recipe");
    let column = |m: Option<i32>| m.map(|value| time_json(value).to_string());
    let mut row = recipe.into_active_model();
    row.prep_time = Set(column(prep));
    row.cook_time = Set(column(cook));
    row.total_time = Set(column(total));
    row.total_minutes = Set(total);
    let recipe = row.update(db).await.expect("seed times");
    format!("/api/recipes/{}", recipe.id)
}

#[tokio::test]
async fn update_recipe_null_clears_each_optional_field() {
    let db = setup_db().await;
    let mut dto = recipe_dto("Pasta");
    dto.description = Some("Weeknight staple".into());
    dto.notes = Some("Double the garlic".into());
    dto.icon = Some("🍝".into());
    dto.portion_size = Some(fewd_lib::dto::PortionSizeDto {
        value: 1.0,
        unit: "bowl".into(),
    });
    let recipe = RecipeService::create(&db, dto).await.expect("seed recipe");
    let uri = format!("/api/recipes/{}", recipe.id);
    put_ok(&db, &uri, serde_json::json!({ "rating": 4 })).await;

    let row = put_ok(
        &db,
        &uri,
        serde_json::json!({
            "description": null,
            "notes": null,
            "icon": null,
            "portion_size": null,
            "rating": null,
        }),
    )
    .await;

    for field in ["description", "notes", "icon", "portion_size", "rating"] {
        assert!(row[field].is_null(), "{field} is cleared: {row}");
    }
    assert_eq!(row["name"], "Pasta", "unsent fields are untouched");
}

#[tokio::test]
async fn update_recipe_omitting_optional_fields_leaves_them_unchanged() {
    let db = setup_db().await;
    let mut dto = recipe_dto("Pasta");
    dto.description = Some("Weeknight staple".into());
    dto.notes = Some("Double the garlic".into());
    let recipe = RecipeService::create(&db, dto).await.expect("seed recipe");
    let uri = format!("/api/recipes/{}", recipe.id);
    put_ok(&db, &uri, serde_json::json!({ "rating": 4 })).await;

    let row = put_ok(&db, &uri, serde_json::json!({ "name": "Pasta Night" })).await;

    assert_eq!(row["description"], "Weeknight staple");
    assert_eq!(row["notes"], "Double the garlic");
    assert_eq!(row["rating"], 4.0);
}

#[tokio::test]
async fn update_recipe_stores_a_blank_string_verbatim() {
    // The web client sends null for a blank field; a client that sends ""
    // gets the string it sent.
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");

    let row = put_ok(
        &db,
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({ "notes": "" }),
    )
    .await;

    assert_eq!(row["notes"], "");
}

#[tokio::test]
async fn clearing_the_total_nulls_total_time_and_total_minutes() {
    let db = setup_db().await;
    let uri = seed_times(&db, Some(10), Some(20), Some(30)).await;

    // The web form resends prep and cook unchanged beside the cleared total.
    let row = put_ok(
        &db,
        &uri,
        serde_json::json!({
            "prep_time": time_json(10),
            "cook_time": time_json(20),
            "total_time": null,
        }),
    )
    .await;

    assert_eq!(stored_time(&row, "total_time"), None);
    assert!(row["total_minutes"].is_null(), "{row}");
    assert_eq!(stored_time(&row, "prep_time"), minutes(10));
}

#[tokio::test]
async fn clearing_prep_leaves_a_resent_total_unchanged() {
    let db = setup_db().await;
    let uri = seed_times(&db, Some(10), Some(20), Some(30)).await;

    let row = put_ok(
        &db,
        &uri,
        serde_json::json!({
            "prep_time": null,
            "cook_time": time_json(20),
            "total_time": time_json(30),
        }),
    )
    .await;

    assert_eq!(stored_time(&row, "prep_time"), None);
    assert_eq!(stored_time(&row, "total_time"), minutes(30));
    assert_eq!(row["total_minutes"], 30);
}

#[tokio::test]
async fn clearing_prep_while_cook_changes_moves_the_total_by_the_cook_change() {
    // prep 10 + cook 20 inside a 40-minute total. Cook gains 10 minutes;
    // the cleared prep moves nothing.
    let db = setup_db().await;
    let uri = seed_times(&db, Some(10), Some(20), Some(40)).await;

    let row = put_ok(
        &db,
        &uri,
        serde_json::json!({ "prep_time": null, "cook_time": time_json(30) }),
    )
    .await;

    assert_eq!(stored_time(&row, "prep_time"), None);
    assert_eq!(stored_time(&row, "cook_time"), minutes(30));
    assert_eq!(stored_time(&row, "total_time"), minutes(50));
    assert_eq!(row["total_minutes"], 50);
}

#[tokio::test]
async fn clearing_the_total_wins_over_a_prep_change() {
    let db = setup_db().await;
    let uri = seed_times(&db, Some(10), Some(20), Some(30)).await;

    let row = put_ok(
        &db,
        &uri,
        serde_json::json!({ "prep_time": time_json(15), "total_time": null }),
    )
    .await;

    assert_eq!(stored_time(&row, "prep_time"), minutes(15));
    assert_eq!(stored_time(&row, "total_time"), None);
    assert!(row["total_minutes"].is_null(), "{row}");
}

#[tokio::test]
async fn null_for_an_already_missing_total_still_lets_prep_derive_it() {
    let db = setup_db().await;
    let uri = seed_times(&db, Some(10), Some(20), None).await;

    let row = put_ok(
        &db,
        &uri,
        serde_json::json!({ "prep_time": time_json(15), "total_time": null }),
    )
    .await;

    assert_eq!(stored_time(&row, "total_time"), minutes(35));
    assert_eq!(row["total_minutes"], 35);
}

#[tokio::test]
async fn clearing_the_only_phase_keeps_the_total() {
    // A total equal to the cleared phase must not become 0 minutes.
    let db = setup_db().await;
    let uri = seed_times(&db, Some(30), None, Some(30)).await;

    let row = put_ok(&db, &uri, serde_json::json!({ "prep_time": null })).await;

    assert_eq!(stored_time(&row, "prep_time"), None);
    assert_eq!(stored_time(&row, "total_time"), minutes(30));
    assert_eq!(row["total_minutes"], 30);
}

#[tokio::test]
async fn clearing_an_unparsable_stored_time_writes_null() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");
    let mut row = recipe.into_active_model();
    row.cook_time = Set(Some(r#"{"value":3,"unit":"fortnights"}"#.to_string()));
    let recipe = row.update(&db).await.expect("seed legacy unit");

    let row = put_ok(
        &db,
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({ "cook_time": null }),
    )
    .await;

    assert!(row["cook_time"].is_null(), "{row}");
}

async fn seed_drink(db: &DatabaseConnection) -> String {
    let drink = DrinkRecipeService::create(
        db,
        CreateDrinkRecipeDto {
            name: "Negroni".into(),
            description: Some("Bitter and bright".into()),
            source: "manual".into(),
            source_url: None,
            servings: 1,
            instructions: "Stir".into(),
            ingredients: vec![],
            technique: Some("stirred".into()),
            glassware: Some("rocks".into()),
            garnish: Some("orange peel".into()),
            tags: vec![],
            notes: Some("Use Campari".into()),
            icon: Some("🍸".into()),
            is_non_alcoholic: None,
        },
    )
    .await
    .expect("seed drink");
    format!("/api/drink-recipes/{}", drink.id)
}

const DRINK_CLEARABLE: [&str; 7] = [
    "description",
    "technique",
    "glassware",
    "garnish",
    "notes",
    "icon",
    "rating",
];

#[tokio::test]
async fn update_drink_recipe_null_clears_each_optional_field() {
    let db = setup_db().await;
    let uri = seed_drink(&db).await;
    put_ok(&db, &uri, serde_json::json!({ "rating": 5 })).await;

    let body = DRINK_CLEARABLE
        .iter()
        .map(|field| (field.to_string(), serde_json::Value::Null))
        .collect::<serde_json::Map<_, _>>();
    let row = put_ok(&db, &uri, serde_json::Value::Object(body)).await;

    for field in DRINK_CLEARABLE {
        assert!(row[field].is_null(), "{field} is cleared: {row}");
    }
    assert_eq!(row["name"], "Negroni");
}

#[tokio::test]
async fn update_drink_recipe_omitting_optional_fields_leaves_them_unchanged() {
    let db = setup_db().await;
    let uri = seed_drink(&db).await;
    put_ok(&db, &uri, serde_json::json!({ "rating": 5 })).await;

    let row = put_ok(&db, &uri, serde_json::json!({ "name": "Boulevardier" })).await;

    for field in DRINK_CLEARABLE {
        assert!(!row[field].is_null(), "{field} is kept: {row}");
    }
}

async fn seed_person(db: &DatabaseConnection) -> String {
    let (status, person) = send_json(
        app(db),
        "POST",
        "/api/people",
        serde_json::json!({
            "name": "Alice",
            "birthdate": "2015-04-01",
            "dietary_goals": "More iron",
            "dislikes": [],
            "favorites": [],
            "notes": "Prefers mild food",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{person}");
    format!("/api/people/{}", person["id"].as_str().expect("id"))
}

#[tokio::test]
async fn update_person_null_clears_notes_and_dietary_goals() {
    let db = setup_db().await;
    let uri = seed_person(&db).await;

    let row = put_ok(
        &db,
        &uri,
        serde_json::json!({ "notes": null, "dietary_goals": null }),
    )
    .await;

    assert!(row["notes"].is_null(), "{row}");
    assert!(row["dietary_goals"].is_null(), "{row}");
    assert_eq!(row["name"], "Alice");
}

#[tokio::test]
async fn update_person_omitting_notes_leaves_them_unchanged() {
    let db = setup_db().await;
    let uri = seed_person(&db).await;

    let row = put_ok(&db, &uri, serde_json::json!({ "is_active": false })).await;

    assert_eq!(row["notes"], "Prefers mild food");
    assert_eq!(row["dietary_goals"], "More iron");
}

#[tokio::test]
async fn writes_to_a_missing_id_answer_404() {
    let db = setup_db().await;
    let puts = [
        ("/api/recipes/missing", "Recipe not found"),
        ("/api/drink-recipes/missing", "Drink recipe not found"),
        ("/api/people/missing", "Person not found"),
        ("/api/meals/missing", "Meal not found"),
        ("/api/meal-templates/missing", "Meal template not found"),
    ];
    for (uri, expected) in puts {
        let (status, body) = send_json(app(&db), "PUT", uri, serde_json::json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "PUT {uri}: {body}");
        assert_eq!(message(&body), expected, "PUT {uri}");
    }

    for uri in [
        "/api/recipes/missing/favorite",
        "/api/drink-recipes/missing/favorite",
    ] {
        let (status, body) = send_raw(app(&db), "POST", uri, &[XRW], Vec::new()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "POST {uri}: {body}");
    }

    let (status, body) =
        send_raw(app(&db), "DELETE", "/api/meals/missing", &[XRW], Vec::new()).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "DELETE: {body}");

    let (status, body) = send_json(
        app(&db),
        "POST",
        "/api/meal-templates/from-meal",
        serde_json::json!({ "meal_id": "missing", "name": "Taco night" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(message(&body), "Meal not found");
}

// --- Write normalization: blank names answer 400 ---

fn recipe_body(name: &str, ingredient_names: &[&str]) -> serde_json::Value {
    let ingredients: Vec<serde_json::Value> = ingredient_names
        .iter()
        .map(|n| {
            serde_json::json!({
                "name": n,
                "amount": { "type": "single", "value": 1 },
                "unit": "cup",
                "notes": null,
            })
        })
        .collect();
    serde_json::json!({
        "name": name,
        "source": "manual",
        "servings": 4,
        "instructions": "Mix and cook",
        "ingredients": ingredients,
        "tags": [],
    })
}

#[tokio::test]
async fn recipe_writes_with_a_blank_name_answer_400() {
    let db = setup_db().await;
    let (status, body) = send_json(
        app(&db),
        "POST",
        "/api/recipes",
        recipe_body("  ", &["garlic"]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).starts_with("name"), "{body}");

    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");
    for uri in [
        format!("/api/recipes/{}", recipe.id),
        "/api/recipes/no-such-recipe".to_string(),
    ] {
        let (status, body) =
            send_json(app(&db), "PUT", &uri, serde_json::json!({ "name": "\t" })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert!(message(&body).starts_with("name"), "{uri}: {body}");
    }
}

#[tokio::test]
async fn recipe_writes_with_a_blank_ingredient_name_answer_400() {
    let db = setup_db().await;
    let (status, body) = send_json(
        app(&db),
        "POST",
        "/api/recipes",
        recipe_body("Pasta", &["garlic", ""]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).starts_with("ingredients[2].name"), "{body}");

    let recipe = RecipeService::create(&db, recipe_dto("Pasta"))
        .await
        .expect("seed recipe");
    let (status, body) = send_json(
        app(&db),
        "PUT",
        &format!("/api/recipes/{}", recipe.id),
        serde_json::json!({ "ingredients": recipe_body("x", &[" "])["ingredients"] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).starts_with("ingredients[1].name"), "{body}");
}

#[tokio::test]
async fn drink_writes_with_a_blank_name_answer_400() {
    let db = setup_db().await;
    let (status, body) = send_json(
        app(&db),
        "POST",
        "/api/drink-recipes",
        recipe_body(" ", &["rum"]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).starts_with("name"), "{body}");

    let (status, body) = send_json(
        app(&db),
        "POST",
        "/api/drink-recipes",
        recipe_body("Daiquiri", &["rum", "  "]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).starts_with("ingredients[2].name"), "{body}");

    let drink_uri = seed_drink(&db).await;
    for uri in [drink_uri.as_str(), "/api/drink-recipes/no-such-drink"] {
        let (status, body) =
            send_json(app(&db), "PUT", uri, serde_json::json!({ "name": "" })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert!(message(&body).starts_with("name"), "{uri}: {body}");
    }
}

#[tokio::test]
async fn update_missing_drink_recipe_with_out_of_range_rating_answers_400() {
    // The rating rule does not depend on the row, so a bad rating is
    // reported even when the id names no drink.
    let db = setup_db().await;
    let (status, body) = send_json(
        app(&db),
        "PUT",
        "/api/drink-recipes/no-such-drink",
        serde_json::json!({ "rating": 9 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).starts_with("rating"), "{body}");
}

#[tokio::test]
async fn meal_writes_with_a_blank_adhoc_item_name_answer_400() {
    let db = setup_db().await;
    let body = |item_name: &str| {
        serde_json::json!({
            "date": "2025-06-10",
            "meal_type": "Dinner",
            "order_index": 2,
            "servings": [{
                "food_type": "adhoc",
                "person_id": "p1",
                "adhoc_items": recipe_body("x", &["bread", item_name])["ingredients"],
                "notes": null,
            }],
        })
    };
    let (status, created) = send_json(app(&db), "POST", "/api/meals", body("butter")).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let meal_uri = format!("/api/meals/{}", created["id"].as_str().expect("meal id"));

    for (method, uri) in [("POST", "/api/meals"), ("PUT", meal_uri.as_str())] {
        let (status, response) = send_json(app(&db), method, uri, body(" ")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method}: {response}");
        assert!(
            message(&response).starts_with("servings[1].adhoc_items[2].name"),
            "{method}: {response}"
        );
    }

    let mut template = body("butter");
    template["name"] = serde_json::json!("  ");
    let (status, response) = send_json(app(&db), "POST", "/api/meal-templates", template).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert!(message(&response).starts_with("name"), "{response}");
}

#[tokio::test]
async fn update_missing_recipe_with_out_of_range_rating_answers_400() {
    let db = setup_db().await;
    let (status, body) = send_json(
        app(&db),
        "PUT",
        "/api/recipes/no-such-recipe",
        serde_json::json!({ "rating": 9 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).starts_with("rating"), "{body}");
}

#[tokio::test]
async fn drink_writes_with_nonpositive_servings_answer_400() {
    let db = setup_db().await;
    let mut body = recipe_body("Daiquiri", &["rum"]);
    body["servings"] = serde_json::json!(0);
    let (status, response) = send_json(app(&db), "POST", "/api/drink-recipes", body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert!(message(&response).starts_with("servings"), "{response}");

    // The rule does not depend on the row, so a missing id answers 400.
    let drink_uri = seed_drink(&db).await;
    for uri in [drink_uri.as_str(), "/api/drink-recipes/no-such-drink"] {
        let (status, response) =
            send_json(app(&db), "PUT", uri, serde_json::json!({ "servings": 0 })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {response}");
        assert!(
            message(&response).starts_with("servings"),
            "{uri}: {response}"
        );
    }
}

// --- Cross-site POSTs: /api answers 403 to a POST a browser need not preflight ---

// Builds a multipart body with one non-empty `file` part, using the boundary
// that `MULTIPART` declares.
fn multipart_file_body() -> Vec<u8> {
    b"--XyZ\r\n\
Content-Disposition: form-data; name=\"file\"; filename=\"r.pdf\"\r\n\
Content-Type: application/pdf\r\n\r\n\
%PDF-1.4 not really\r\n\
--XyZ--\r\n"
        .to_vec()
}

fn assert_rejected(status: StatusCode, body: &serde_json::Value, what: &str) {
    assert_eq!(status, StatusCode::FORBIDDEN, "{what}: {body}");
    let message = message(body);
    assert!(
        message.contains("Content-Type: application/json") && message.contains("X-Requested-With"),
        "{what}: the message names the remedy: {body}"
    );
}

// Seed one recipe and one drink, and hand back the four POST routes that take
// no body: recipe favorite, enhance, drink favorite and the connection test.
async fn seed_bodiless_post_uris(db: &DatabaseConnection) -> [String; 4] {
    let recipe = RecipeService::create(db, recipe_dto("Toast"))
        .await
        .expect("seed recipe");
    let drink_uri = seed_drink(db).await;
    [
        format!("/api/recipes/{}/favorite", recipe.id),
        format!("/api/recipes/{}/enhance", recipe.id),
        format!("{drink_uri}/favorite"),
        "/api/settings/test-connection".to_string(),
    ]
}

// Reports whether the only seeded recipe and the only seeded drink are
// favorites.
async fn favorites(db: &DatabaseConnection) -> (bool, bool) {
    let recipes = RecipeService::get_all(db).await.expect("recipes load");
    let drinks = DrinkRecipeService::get_all(db).await.expect("drinks load");
    (recipes[0].is_favorite, drinks[0].is_favorite)
}

#[tokio::test]
async fn cors_simple_posts_to_bodiless_routes_answer_403() {
    let db = setup_db().await;
    let uris = seed_bodiless_post_uris(&db).await;

    for uri in &uris {
        let attempts: [(&str, &[Header], &[u8]); 3] = [
            ("form", &[FORM], b"a=1"),
            ("text/plain", &[TEXT], b"{}"),
            ("no content type", &[], b""),
        ];
        for (what, headers, body) in attempts {
            let (status, response) = send_raw(app(&db), "POST", uri, headers, body.to_vec()).await;
            assert_rejected(status, &response, &format!("POST {uri} as {what}"));
        }
    }
    assert_eq!(favorites(&db).await, (false, false), "nothing was toggled");
}

#[tokio::test]
async fn bodiless_posts_from_the_web_client_reach_their_handlers() {
    let db = setup_db().await;
    let [recipe_favorite, enhance, drink_favorite, test_connection] =
        seed_bodiless_post_uris(&db).await;

    // The web client sends X-Requested-With and no body. Scripted clients may
    // send the JSON content type and no body instead.
    for headers in [&[XRW][..], &[JSON]] {
        for uri in [&recipe_favorite, &enhance, &drink_favorite] {
            let (status, body) = send_raw(app(&db), "POST", uri, headers, Vec::new()).await;
            assert_eq!(status, StatusCode::OK, "POST {uri}: {body}");
        }
        // No key is stored, so the handler answers before any network call.
        let (status, body) =
            send_raw(app(&db), "POST", &test_connection, headers, Vec::new()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(message(&body), "No API key configured");
    }
    // Each favorite route ran twice, so each flag is back where it started.
    assert_eq!(favorites(&db).await, (false, false));

    let (status, body) = send_raw(app(&db), "POST", &recipe_favorite, &[XRW], Vec::new()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(favorites(&db).await, (true, false));
}

#[tokio::test]
async fn file_import_without_x_requested_with_answers_403() {
    let db = setup_db().await;
    let attempts: [(&str, &[Header], Vec<u8>); 3] = [
        ("multipart", &[MULTIPART], multipart_file_body()),
        ("form", &[FORM], b"file=x".to_vec()),
        ("text/plain", &[TEXT], b"x".to_vec()),
    ];
    for (what, headers, body) in attempts {
        let (status, response) =
            send_raw(app(&db), "POST", "/api/recipes/import/file", headers, body).await;
        assert_rejected(status, &response, what);
    }
}

#[tokio::test]
async fn file_import_with_x_requested_with_reaches_the_handler() {
    let db = setup_db().await;
    let (status, body) = send_raw(
        app(&db),
        "POST",
        "/api/recipes/import/file",
        &[MULTIPART, XRW],
        multipart_file_body(),
    )
    .await;
    // No key is stored, so the handler stops after reading the file.
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(message(&body), "No API key configured. Set it in Settings.");
}

#[tokio::test]
async fn the_guard_reads_the_content_type_essence_and_x_requested_with() {
    let db = setup_db().await;
    let [recipe_favorite, ..] = seed_bodiless_post_uris(&db).await;

    // Each row is the request's headers and whether the guard lets it through.
    let cases: [(&[Header], bool); 10] = [
        (
            &[("content-type", b"Multipart/Form-Data; boundary=XyZ")],
            false,
        ),
        (&[("content-type", b"TEXT/PLAIN")], false),
        (&[("content-type", b"text/plain;\tcharset=utf-8")], false),
        (&[("content-type", b"text/plain; charset=\x80")], false),
        (&[("content-type", b"")], false),
        (&[("content-type", b"application/jsonx")], false),
        (
            &[("content-type", b"application/json; charset=utf-8")],
            true,
        ),
        (&[("content-type", b"Application/JSON")], true),
        (
            &[("content-type", b" application/json ;charset=utf-8")],
            true,
        ),
        (&[TEXT, XRW], true),
    ];
    for (headers, passes) in cases {
        let what: Vec<String> = headers
            .iter()
            .map(|(name, value)| format!("{name}: {}", String::from_utf8_lossy(value)))
            .collect();
        let what = what.join(", ");
        let (status, body) =
            send_raw(app(&db), "POST", &recipe_favorite, headers, Vec::new()).await;
        if passes {
            assert_eq!(status, StatusCode::OK, "{what}: {body}");
        } else {
            assert_rejected(status, &body, &what);
        }
    }
}

#[tokio::test]
async fn the_guard_leaves_routing_errors_and_other_methods_alone() {
    let db = setup_db().await;

    // The test router has no SPA fallback, so an unknown path is a 404.
    let (status, _) = send_raw(app(&db), "POST", "/api/nope", &[TEXT], Vec::new()).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown path");
    // On a path that exists, axum runs the route layer before its 405, so
    // only a request that passes the guard sees the method error.
    let (status, body) = send_raw(app(&db), "POST", "/api/version", &[TEXT], Vec::new()).await;
    assert_rejected(status, &body, "text/plain POST to a GET-only path");
    let (status, _) = send_raw(app(&db), "POST", "/api/version", &[JSON], Vec::new()).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "GET-only path");
    let (status, _) = send_raw(app(&db), "GET", "/api/recipes", &[], Vec::new()).await;
    assert_eq!(status, StatusCode::OK, "GET without a content type");
    let (status, _) = send_raw(app(&db), "DELETE", "/api/meals/missing", &[], Vec::new()).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "DELETE without a content type"
    );
}

#[tokio::test]
async fn form_post_to_a_json_route_answers_403() {
    let db = setup_db().await;
    let (status, body) = send_raw(
        app(&db),
        "POST",
        "/api/recipes",
        &[FORM],
        b"name=Toast".to_vec(),
    )
    .await;
    assert_rejected(status, &body, "form POST to /api/recipes");
}

#[tokio::test]
async fn token_provisioning_takes_the_client_shapes_and_rejects_a_form() {
    let db = setup_db().await;
    let uri = format!("{}/mcp-token", seed_person(&db).await);

    let (status, body) = send_raw(app(&db), "POST", &uri, &[FORM], b"a=1".to_vec()).await;
    assert_rejected(status, &body, "form POST to the token route");

    // The web client sends X-Requested-With with a `{}` JSON body, and a
    // script may send either one alone.
    let shapes: [(&[Header], &[u8]); 3] = [(&[XRW, JSON], b"{}"), (&[XRW], b""), (&[JSON], b"")];
    for (headers, body) in shapes {
        let (status, response) = send_raw(app(&db), "POST", &uri, headers, body.to_vec()).await;
        assert_eq!(status, StatusCode::OK, "{headers:?}: {response}");
        assert!(response["token"].is_string(), "{response}");
    }
}
