// Write locking, concurrent writes and favorite toggles against a real
// file-backed, multi-connection pool. Every call must succeed, and the
// counters, flags and settings must match what the calls did, with no lost
// update.

mod common;

use common::{drink_dto, recipe_dto, TempDbPath};
use fewd_lib::db;
use fewd_lib::dto::{CreateMealDto, MealType, PersonServingDto, UpdateMealDto};
use fewd_lib::services::drink_recipe_service::DrinkRecipeService;
use fewd_lib::services::meal_service::MealService;
use fewd_lib::services::recipe_service::RecipeService;
use fewd_lib::services::settings_service::SettingsService;
use fewd_lib::services::{begin_write, seed_data};
use sea_orm::{sqlx, DatabaseConnection};
use std::time::Duration;

async fn create_recipe(db: &DatabaseConnection, name: &str) -> String {
    RecipeService::create(db, recipe_dto(name))
        .await
        .expect("create recipe")
        .id
}

fn servings_for(recipe_id: &str) -> Vec<PersonServingDto> {
    vec![PersonServingDto::Recipe {
        person_id: "alice".to_string(),
        recipe_id: recipe_id.to_string(),
        servings_count: 1.0,
        notes: None,
    }]
}

fn dinner_of(recipe_id: &str) -> CreateMealDto {
    CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: MealType::Dinner,
        order_index: 2,
        servings: servings_for(recipe_id),
    }
}

async fn times_planned(db: &DatabaseConnection, recipe_id: &str) -> i32 {
    RecipeService::get_by_id(db, recipe_id.to_string())
        .await
        .expect("read recipe")
        .expect("recipe exists")
        .times_planned
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_meal_creates_count_every_plan() {
    let temp = TempDbPath::new("meal_concurrency_creates");
    let db = db::init(&temp.0).await.expect("db::init failed");
    let recipe_id = create_recipe(&db, "Pasta").await;

    const N: usize = 50;
    let handles: Vec<_> = (0..N)
        .map(|_| {
            let db = db.clone();
            let recipe_id = recipe_id.clone();
            tokio::spawn(async move { MealService::create(&db, dinner_of(&recipe_id)).await })
        })
        .collect();
    for h in handles {
        h.await
            .expect("create task panicked")
            .expect("every concurrent create must succeed");
    }

    assert_eq!(times_planned(&db, &recipe_id).await, N as i32);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_meal_updates_keep_counters_in_step() {
    let temp = TempDbPath::new("meal_concurrency_updates");
    let db = db::init(&temp.0).await.expect("db::init failed");
    let a = create_recipe(&db, "Pasta").await;
    let b = create_recipe(&db, "Soup").await;
    let meal = MealService::create(&db, dinner_of(&a))
        .await
        .expect("create meal");

    let handles: Vec<_> = (0..40)
        .map(|i| {
            let db = db.clone();
            let meal_id = meal.id.clone();
            let target = if i % 2 == 0 { b.clone() } else { a.clone() };
            tokio::spawn(async move {
                MealService::update(
                    &db,
                    meal_id,
                    UpdateMealDto {
                        date: None,
                        meal_type: None,
                        order_index: None,
                        servings: Some(servings_for(&target)),
                    },
                )
                .await
            })
        })
        .collect();
    for h in handles {
        h.await
            .expect("update task panicked")
            .expect("every concurrent update must succeed");
    }

    // The meal plans exactly one recipe, so that recipe counts 1 and the
    // other counts 0, whichever update landed last.
    let stored = MealService::get_by_id(&db, meal.id)
        .await
        .expect("read meal")
        .expect("meal exists");
    let final_servings: Vec<PersonServingDto> =
        serde_json::from_str(&stored.servings).expect("servings JSON");
    let (planned, other) = match &final_servings[0] {
        PersonServingDto::Recipe { recipe_id, .. } if *recipe_id == a => (&a, &b),
        PersonServingDto::Recipe { .. } => (&b, &a),
        other => panic!("expected a recipe serving, got {other:?}"),
    };
    assert_eq!(times_planned(&db, planned).await, 1);
    assert_eq!(times_planned(&db, other).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_recipe_favorite_toggles_all_count() {
    let temp = TempDbPath::new("recipe_toggle_concurrency");
    let db = db::init(&temp.0).await.expect("db::init failed");
    let recipe_id = create_recipe(&db, "Pasta").await;

    // An odd number of toggles from false must end at true.
    let handles: Vec<_> = (0..41)
        .map(|_| {
            let db = db.clone();
            let recipe_id = recipe_id.clone();
            tokio::spawn(async move { RecipeService::toggle_favorite(&db, recipe_id).await })
        })
        .collect();
    for h in handles {
        h.await
            .expect("toggle task panicked")
            .expect("every concurrent toggle must succeed");
    }

    let stored = RecipeService::get_by_id(&db, recipe_id)
        .await
        .expect("read recipe")
        .expect("recipe exists");
    assert!(stored.is_favorite);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_drink_favorite_toggles_all_count() {
    let temp = TempDbPath::new("drink_toggle_concurrency");
    let db = db::init(&temp.0).await.expect("db::init failed");
    let drink = DrinkRecipeService::create(&db, drink_dto("Daiquiri"))
        .await
        .expect("create drink");

    let handles: Vec<_> = (0..41)
        .map(|_| {
            let db = db.clone();
            let id = drink.id.clone();
            tokio::spawn(async move { DrinkRecipeService::toggle_favorite(&db, id).await })
        })
        .collect();
    for h in handles {
        h.await
            .expect("toggle task panicked")
            .expect("every concurrent toggle must succeed");
    }

    let stored = DrinkRecipeService::get_by_id(&db, drink.id)
        .await
        .expect("read drink")
        .expect("drink exists");
    assert!(stored.is_favorite);
}

// Takes a pooled connection that never waits on a lock, so a test can ask for
// the write lock and see at once whether another connection holds it.
async fn impatient_connection(db: &DatabaseConnection) -> sqlx::pool::PoolConnection<sqlx::Sqlite> {
    let mut conn = db.get_sqlite_connection_pool().acquire().await.unwrap();
    sqlx::query("PRAGMA busy_timeout = 0")
        .execute(&mut *conn)
        .await
        .unwrap();
    conn
}

#[tokio::test]
async fn begin_write_holds_the_write_lock_until_the_transaction_ends() {
    let temp = TempDbPath::new("begin_write_lock");
    let db = db::init(&temp.0).await.expect("db::init failed");

    let txn = begin_write(&db).await.unwrap();
    let mut other = impatient_connection(&db).await;
    let err = sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *other)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("locked"), "{err}");

    txn.rollback().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *other)
        .await
        .unwrap();
    sqlx::query("ROLLBACK").execute(&mut *other).await.unwrap();
}

#[tokio::test]
async fn seed_on_a_seeded_database_does_not_wait_for_the_write_lock() {
    let temp = TempDbPath::new("seed_fast_path");
    // init seeds the empty database.
    let db = db::init(&temp.0).await.expect("db::init failed");
    let mut holder = impatient_connection(&db).await;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *holder)
        .await
        .unwrap();

    // Taking the write lock would wait out the 5s busy_timeout.
    tokio::time::timeout(Duration::from_secs(2), seed_data::seed_if_empty(&db))
        .await
        .expect("seed must not wait for the write lock")
        .expect("seed must succeed");

    sqlx::query("ROLLBACK").execute(&mut *holder).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_sets_of_an_absent_setting_all_succeed() {
    let temp = TempDbPath::new("settings_set_concurrency");
    let db = db::init(&temp.0).await.expect("db::init failed");

    let values: Vec<String> = (0..20).map(|i| format!("value-{i}")).collect();
    let handles: Vec<_> = values
        .iter()
        .cloned()
        .map(|value| {
            let db = db.clone();
            tokio::spawn(async move {
                SettingsService::set(&db, "concurrent_key".to_string(), value).await
            })
        })
        .collect();
    for h in handles {
        h.await
            .expect("set task panicked")
            .expect("every concurrent set must succeed");
    }

    let stored = SettingsService::get(&db, "concurrent_key".to_string())
        .await
        .expect("read setting")
        .expect("setting exists");
    assert!(values.contains(&stored), "{stored}");
}
