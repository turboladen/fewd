use fewd_lib::dto::{
    CreateMealDto, CreateMealTemplateDto, CreatePersonDto, CreateRecipeDto, IngredientAmountDto,
    IngredientDto, PersonServingDto, UpdateRecipeDto,
};
use fewd_lib::services::meal_service::MealService;
use fewd_lib::services::meal_template_service::MealTemplateService;
use fewd_lib::services::person_service::PersonService;
use fewd_lib::services::prompt_builder::PromptBuilder;
use fewd_lib::services::recipe_adapter::{PersonAdaptOptions, RecipeAdapter};
use fewd_lib::services::recipe_enhancer;
use fewd_lib::services::recipe_scaler;
use fewd_lib::services::recipe_service::RecipeService;
use fewd_lib::services::seed_data;
use fewd_lib::services::settings_service::SettingsService;
use fewd_lib::services::shopping_service::ShoppingService;
use fewd_lib::services::suggestion_service::SuggestionService;
use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection, EntityTrait, IntoActiveModel, Set};

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}

fn test_person_dto(name: &str) -> CreatePersonDto {
    CreatePersonDto {
        name: name.to_string(),
        birthdate: "2000-01-15".to_string(),
        dietary_goals: None,
        dislikes: vec!["olives".to_string()],
        favorites: vec!["pasta".to_string()],
        notes: None,
        drink_preferences: None,
        drink_dislikes: None,
    }
}

fn test_recipe_dto(name: &str) -> CreateRecipeDto {
    CreateRecipeDto {
        name: name.to_string(),
        description: Some("A test recipe".to_string()),
        source: "manual".to_string(),
        source_url: None,
        parent_recipe_id: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: 4,
        portion_size: None,
        instructions: "Mix and cook".to_string(),
        ingredients: vec![
            IngredientDto {
                name: "flour".to_string(),
                amount: IngredientAmountDto::Single { value: 2.0 },
                unit: "cups".to_string(),
                notes: None,
            },
            IngredientDto {
                name: "eggs".to_string(),
                amount: IngredientAmountDto::Single { value: 3.0 },
                unit: "whole".to_string(),
                notes: None,
            },
        ],
        nutrition_per_serving: None,
        tags: vec!["dinner".to_string(), "easy".to_string()],
        notes: None,
        icon: None,
    }
}

// --- PersonService Tests ---

#[tokio::test]
async fn person_create_and_get_all() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    assert_eq!(person.name, "Alice");
    assert!(person.is_active);
    assert_eq!(person.birthdate.to_string(), "2000-01-15");

    let all = PersonService::get_all(&db).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Alice");
}

#[tokio::test]
async fn person_get_all_filters_inactive() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Bob"))
        .await
        .unwrap();

    // Deactivate via update
    let update = fewd_lib::dto::UpdatePersonDto {
        name: None,
        birthdate: None,
        dietary_goals: None,
        dislikes: None,
        favorites: None,
        notes: None,
        is_active: Some(false),
        drink_preferences: None,
        drink_dislikes: None,
    };
    PersonService::update(&db, person.id.clone(), update)
        .await
        .unwrap();

    let all = PersonService::get_all(&db).await.unwrap();
    assert_eq!(all.len(), 0);

    // But get_by_id still finds them
    let found = PersonService::get_by_id(&db, person.id).await.unwrap();
    assert!(found.is_some());
}

#[tokio::test]
async fn person_get_all_ordered_by_name() {
    let db = setup_db().await;
    PersonService::create(&db, test_person_dto("Zara"))
        .await
        .unwrap();
    PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    PersonService::create(&db, test_person_dto("Mia"))
        .await
        .unwrap();

    let all = PersonService::get_all(&db).await.unwrap();
    assert_eq!(all[0].name, "Alice");
    assert_eq!(all[1].name, "Mia");
    assert_eq!(all[2].name, "Zara");
}

#[tokio::test]
async fn person_update_partial_fields() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let update = fewd_lib::dto::UpdatePersonDto {
        name: Some("Alice Updated".to_string()),
        birthdate: None,
        dietary_goals: Some("more protein".to_string()),
        dislikes: None,
        favorites: None,
        notes: None,
        is_active: None,
        drink_preferences: None,
        drink_dislikes: None,
    };
    let updated = PersonService::update(&db, person.id, update).await.unwrap();
    assert_eq!(updated.name, "Alice Updated");
    assert_eq!(updated.dietary_goals, Some("more protein".to_string()));
    // Birthdate unchanged
    assert_eq!(updated.birthdate.to_string(), "2000-01-15");
}

#[tokio::test]
async fn person_invalid_birthdate_fails() {
    let db = setup_db().await;
    let dto = CreatePersonDto {
        name: "Test".to_string(),
        birthdate: "not-a-date".to_string(),
        dietary_goals: None,
        dislikes: vec![],
        favorites: vec![],
        notes: None,
        drink_preferences: None,
        drink_dislikes: None,
    };
    let result = PersonService::create(&db, dto).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn person_delete() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    PersonService::delete(&db, person.id.clone()).await.unwrap();
    let found = PersonService::get_by_id(&db, person.id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn person_json_fields_roundtrip() {
    let db = setup_db().await;
    let dto = CreatePersonDto {
        name: "Test".to_string(),
        birthdate: "2000-01-01".to_string(),
        dietary_goals: None,
        dislikes: vec!["olives".to_string(), "mushrooms".to_string()],
        favorites: vec!["pizza".to_string()],
        notes: None,
        drink_preferences: None,
        drink_dislikes: None,
    };
    let person = PersonService::create(&db, dto).await.unwrap();

    let dislikes: Vec<String> = serde_json::from_str(&person.dislikes).unwrap();
    let favorites: Vec<String> = serde_json::from_str(&person.favorites).unwrap();
    assert_eq!(dislikes, vec!["olives", "mushrooms"]);
    assert_eq!(favorites, vec!["pizza"]);
}

// --- RecipeService Tests ---

#[tokio::test]
async fn recipe_create_and_get_all() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    assert_eq!(recipe.name, "Pasta");
    assert!(!recipe.is_favorite);
    assert_eq!(recipe.times_made, 0);
    assert_eq!(recipe.servings, 4);

    let all = RecipeService::get_all(&db).await.unwrap();
    assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn recipe_create_derives_slug_from_name() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pizza Margherita"))
        .await
        .unwrap();
    assert_eq!(recipe.slug, "pizza-margherita");
}

#[tokio::test]
async fn recipe_slug_collision_gets_suffix() {
    let db = setup_db().await;
    let first = RecipeService::create(&db, test_recipe_dto("Pizza"))
        .await
        .unwrap();
    let second = RecipeService::create(&db, test_recipe_dto("Pizza"))
        .await
        .unwrap();
    let third = RecipeService::create(&db, test_recipe_dto("Pizza"))
        .await
        .unwrap();
    assert_eq!(first.slug, "pizza");
    assert_eq!(second.slug, "pizza-2");
    assert_eq!(third.slug, "pizza-3");
}

#[tokio::test]
async fn recipe_slug_collision_stays_under_length_cap() {
    // Base name that, after slugify, is already at the 80-char cap. Two creates
    // with the same name force collision handling to truncate the base and
    // re-append the suffix so the final slug stays ≤ 80 chars.
    let db = setup_db().await;
    let long_name = "a".repeat(200);
    let first = RecipeService::create(&db, test_recipe_dto(&long_name))
        .await
        .unwrap();
    let second = RecipeService::create(&db, test_recipe_dto(&long_name))
        .await
        .unwrap();
    assert_eq!(first.slug.len(), 80);
    assert!(
        second.slug.len() <= 80,
        "second slug was {}",
        second.slug.len()
    );
    assert!(second.slug.ends_with("-2"));
    assert_ne!(first.slug, second.slug);
}

#[tokio::test]
async fn recipe_slug_survives_rename() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Untitled Recipe"))
        .await
        .unwrap();
    assert_eq!(recipe.slug, "untitled-recipe");

    let renamed = RecipeService::update(
        &db,
        recipe.id.clone(),
        UpdateRecipeDto {
            name: Some("Grandma's Bolognese".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(renamed.name, "Grandma's Bolognese");
    // Slug is pinned at creation — renames do not rewrite it.
    assert_eq!(renamed.slug, "untitled-recipe");
}

#[tokio::test]
async fn recipe_get_by_slug() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    let found = RecipeService::get_by_slug(&db, recipe.slug.clone())
        .await
        .unwrap()
        .expect("found by slug");
    assert_eq!(found.id, recipe.id);

    let missing = RecipeService::get_by_slug(&db, "does-not-exist".to_string())
        .await
        .unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn recipe_toggle_favorite() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();
    assert!(!recipe.is_favorite);

    let toggled = RecipeService::toggle_favorite(&db, recipe.id.clone())
        .await
        .unwrap();
    assert!(toggled.is_favorite);

    let toggled_back = RecipeService::toggle_favorite(&db, recipe.id)
        .await
        .unwrap();
    assert!(!toggled_back.is_favorite);
}

#[tokio::test]
async fn recipe_search() {
    let db = setup_db().await;
    RecipeService::create(&db, test_recipe_dto("Chicken Tacos"))
        .await
        .unwrap();
    RecipeService::create(&db, test_recipe_dto("Beef Stew"))
        .await
        .unwrap();
    RecipeService::create(&db, test_recipe_dto("Chicken Soup"))
        .await
        .unwrap();

    let results = RecipeService::search(&db, "Chicken".to_string())
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn recipe_update_partial() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    let update = UpdateRecipeDto {
        name: Some("Pasta Carbonara".to_string()),
        description: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: Some(6),
        portion_size: None,
        instructions: None,
        ingredients: None,
        nutrition_per_serving: None,
        tags: None,
        notes: None,
        icon: None,
        is_favorite: None,
        rating: None,
    };
    let updated = RecipeService::update(&db, recipe.id, update).await.unwrap();
    assert_eq!(updated.name, "Pasta Carbonara");
    assert_eq!(updated.servings, 6);
    // Instructions unchanged
    assert_eq!(updated.instructions, "Mix and cook");
}

#[tokio::test]
async fn recipe_delete() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();
    RecipeService::delete(&db, recipe.id.clone()).await.unwrap();
    let found = RecipeService::get_by_id(&db, recipe.id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn recipe_not_found_error() {
    let db = setup_db().await;
    let result = RecipeService::toggle_favorite(&db, "nonexistent-id".to_string()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn recipe_set_rating() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();
    assert!(recipe.rating.is_none());

    let update = UpdateRecipeDto {
        name: None,
        description: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: None,
        portion_size: None,
        instructions: None,
        ingredients: None,
        nutrition_per_serving: None,
        tags: None,
        notes: None,
        icon: None,
        is_favorite: None,
        rating: Some(4.0),
    };
    let updated = RecipeService::update(&db, recipe.id.clone(), update)
        .await
        .unwrap();
    assert_eq!(updated.rating, Some(4.0));

    // Verify persisted after re-fetch
    let fetched = RecipeService::get_by_id(&db, recipe.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.rating, Some(4.0));
}

#[tokio::test]
async fn recipe_rating_rejects_invalid_values() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    // Too low
    let update = UpdateRecipeDto {
        name: None,
        description: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: None,
        portion_size: None,
        instructions: None,
        ingredients: None,
        nutrition_per_serving: None,
        tags: None,
        notes: None,
        icon: None,
        is_favorite: None,
        rating: Some(0.0),
    };
    assert!(RecipeService::update(&db, recipe.id.clone(), update)
        .await
        .is_err());

    // Too high
    let update = UpdateRecipeDto {
        name: None,
        description: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: None,
        portion_size: None,
        instructions: None,
        ingredients: None,
        nutrition_per_serving: None,
        tags: None,
        notes: None,
        icon: None,
        is_favorite: None,
        rating: Some(5.5),
    };
    assert!(RecipeService::update(&db, recipe.id.clone(), update)
        .await
        .is_err());

    // Below minimum (0.4 rounds to 0, which is < 1)
    let update = UpdateRecipeDto {
        name: None,
        description: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: None,
        portion_size: None,
        instructions: None,
        ingredients: None,
        nutrition_per_serving: None,
        tags: None,
        notes: None,
        icon: None,
        is_favorite: None,
        rating: Some(0.4),
    };
    assert!(RecipeService::update(&db, recipe.id, update).await.is_err());
}

// --- MealService Tests ---

#[tokio::test]
async fn meal_create_and_query_date_range() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Dinner".to_string(),
        order_index: 2,
        servings: vec![PersonServingDto::Recipe {
            person_id: person.id.clone(),
            recipe_id: recipe.id.clone(),
            servings_count: 1.0,
            notes: None,
        }],
    };
    MealService::create(&db, meal_dto).await.unwrap();

    // Query that includes this date
    let meals = MealService::get_all_for_date_range(
        &db,
        "2025-06-09".to_string(),
        "2025-06-11".to_string(),
    )
    .await
    .unwrap();
    assert_eq!(meals.len(), 1);
    assert_eq!(meals[0].meal_type, "Dinner");

    // Query that misses this date
    let meals = MealService::get_all_for_date_range(
        &db,
        "2025-06-01".to_string(),
        "2025-06-09".to_string(),
    )
    .await
    .unwrap();
    assert_eq!(meals.len(), 0);
}

#[tokio::test]
async fn meal_increments_recipe_times_made() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    assert_eq!(recipe.times_made, 0);

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Dinner".to_string(),
        order_index: 2,
        servings: vec![PersonServingDto::Recipe {
            person_id: person.id,
            recipe_id: recipe.id.clone(),
            servings_count: 1.0,
            notes: None,
        }],
    };
    MealService::create(&db, meal_dto).await.unwrap();

    let updated_recipe = RecipeService::get_by_id(&db, recipe.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_recipe.times_made, 1);
    assert!(updated_recipe.last_made.is_some());
}

#[tokio::test]
async fn meal_with_adhoc_items() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Breakfast".to_string(),
        order_index: 0,
        servings: vec![PersonServingDto::Adhoc {
            person_id: person.id,
            adhoc_items: vec![IngredientDto {
                name: "banana".to_string(),
                amount: IngredientAmountDto::Single { value: 1.0 },
                unit: "whole".to_string(),
                notes: None,
            }],
            notes: None,
        }],
    };
    let meal = MealService::create(&db, meal_dto).await.unwrap();
    assert_eq!(meal.meal_type, "Breakfast");
}

#[tokio::test]
async fn meal_delete() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Lunch".to_string(),
        order_index: 1,
        servings: vec![PersonServingDto::Adhoc {
            person_id: person.id,
            adhoc_items: vec![],
            notes: None,
        }],
    };
    let meal = MealService::create(&db, meal_dto).await.unwrap();
    MealService::delete(&db, meal.id.clone()).await.unwrap();
    let found = MealService::get_by_id(&db, meal.id).await.unwrap();
    assert!(found.is_none());
}

// --- ShoppingService Tests ---

#[tokio::test]
async fn shopping_list_empty_range() {
    let db = setup_db().await;
    let list =
        ShoppingService::get_shopping_list(&db, "2025-06-09".to_string(), "2025-06-15".to_string())
            .await
            .unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn shopping_list_aggregates_recipe_ingredients() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    // Create a meal with 1 serving (recipe has 4 servings, so scale = 0.25)
    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Dinner".to_string(),
        order_index: 2,
        servings: vec![PersonServingDto::Recipe {
            person_id: person.id,
            recipe_id: recipe.id,
            servings_count: 1.0,
            notes: None,
        }],
    };
    MealService::create(&db, meal_dto).await.unwrap();

    let list =
        ShoppingService::get_shopping_list(&db, "2025-06-09".to_string(), "2025-06-15".to_string())
            .await
            .unwrap();

    // Should have 2 ingredients (flour, eggs) scaled to 1/4
    assert_eq!(list.len(), 2);

    let eggs = list.iter().find(|i| i.ingredient_name == "eggs").unwrap();
    assert_eq!(eggs.items.len(), 1);

    let flour = list.iter().find(|i| i.ingredient_name == "flour").unwrap();
    assert_eq!(flour.items.len(), 1);
}

#[tokio::test]
async fn shopping_list_includes_adhoc() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Breakfast".to_string(),
        order_index: 0,
        servings: vec![PersonServingDto::Adhoc {
            person_id: person.id,
            adhoc_items: vec![IngredientDto {
                name: "banana".to_string(),
                amount: IngredientAmountDto::Single { value: 2.0 },
                unit: "whole".to_string(),
                notes: None,
            }],
            notes: None,
        }],
    };
    MealService::create(&db, meal_dto).await.unwrap();

    let list =
        ShoppingService::get_shopping_list(&db, "2025-06-09".to_string(), "2025-06-15".to_string())
            .await
            .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].ingredient_name, "banana");
}

// --- Recipe Scaling Tests ---

#[tokio::test]
async fn recipe_scale_preview_doubles_ingredients() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    let ingredients: Vec<IngredientDto> = serde_json::from_str(&recipe.ingredients).unwrap();
    let ratio = 8.0 / recipe.servings as f64; // 4 → 8 servings
    let result = recipe_scaler::scale_ingredients(&ingredients, ratio);

    assert_eq!(result.ingredients.len(), 2);
    match &result.ingredients[0].amount {
        IngredientAmountDto::Single { value } => assert_eq!(*value, 4.0), // 2 cups * 2
        _ => panic!("expected Single"),
    }
    match &result.ingredients[1].amount {
        IngredientAmountDto::Single { value } => assert_eq!(*value, 6.0), // 3 eggs * 2
        _ => panic!("expected Single"),
    }
    // Eggs are "whole" (discrete), but 6.0 is whole → no flag
    assert!(result.flagged.is_empty());
}

#[tokio::test]
async fn recipe_scale_flags_fractional_discrete() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    let ingredients: Vec<IngredientDto> = serde_json::from_str(&recipe.ingredients).unwrap();
    let ratio = 6.0 / recipe.servings as f64; // 4 → 6 servings (1.5x)
    let result = recipe_scaler::scale_ingredients(&ingredients, ratio);

    // Flour: 2 * 1.5 = 3.0 cups → no flag (continuous unit)
    // Eggs: 3 * 1.5 = 4.5 whole → flagged
    assert_eq!(result.flagged.len(), 1);
    assert_eq!(result.flagged[0].name, "eggs");
    assert_eq!(result.flagged[0].scaled_value, 4.5);
}

#[tokio::test]
async fn recipe_create_with_parent_id() {
    let db = setup_db().await;
    let parent = RecipeService::create(&db, test_recipe_dto("Original"))
        .await
        .unwrap();

    let mut child_dto = test_recipe_dto("Original (8 servings)");
    child_dto.parent_recipe_id = Some(parent.id.clone());
    child_dto.source = "scaled".to_string();
    child_dto.servings = 8;

    let child = RecipeService::create(&db, child_dto).await.unwrap();
    assert_eq!(child.parent_recipe_id, Some(parent.id.clone()));
    assert_eq!(child.source, "scaled");
    assert_eq!(child.servings, 8);

    // Verify persists on re-fetch
    let fetched = RecipeService::get_by_id(&db, child.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.parent_recipe_id, Some(parent.id));
}

// --- Recipe Enhancement Tests ---

#[tokio::test]
async fn recipe_enhance_injects_amounts() {
    let db = setup_db().await;

    let mut dto = test_recipe_dto("Pancakes");
    dto.instructions = "Mix flour until smooth.\nAdd eggs and stir.".to_string();
    let recipe = RecipeService::create(&db, dto).await.unwrap();

    let ingredients: Vec<IngredientDto> = serde_json::from_str(&recipe.ingredients).unwrap();
    let enhanced = recipe_enhancer::enhance_instructions(&ingredients, &recipe.instructions);

    // flour (2 cups) should be injected in first line
    assert!(enhanced.contains("**2 cups flour**"));
    // eggs (3 whole) should be injected in second line
    assert!(enhanced.contains("**3 whole eggs**"));
}

#[tokio::test]
async fn recipe_enhance_skips_already_numbered() {
    let db = setup_db().await;

    let mut dto = test_recipe_dto("Pancakes");
    dto.instructions = "Add 2 cups flour to bowl.".to_string();
    let recipe = RecipeService::create(&db, dto).await.unwrap();

    let ingredients: Vec<IngredientDto> = serde_json::from_str(&recipe.ingredients).unwrap();
    let enhanced = recipe_enhancer::enhance_instructions(&ingredients, &recipe.instructions);

    // Should NOT inject because "flour" already has "2 cups" before it
    assert!(!enhanced.contains("**"));
    assert_eq!(enhanced, "Add 2 cups flour to bowl.");
}

// --- SeedData Tests ---

#[tokio::test]
async fn seed_data_populates_empty_db() {
    let db = setup_db().await;
    seed_data::seed_if_empty(&db).await.unwrap();

    let people = PersonService::get_all(&db).await.unwrap();
    assert_eq!(people.len(), 4);

    let names: Vec<&str> = people.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"Alex"));
    assert!(names.contains(&"Jordan"));
    assert!(names.contains(&"Sam"));
    assert!(names.contains(&"Pat"));
}

#[tokio::test]
async fn seed_data_skips_nonempty_db() {
    let db = setup_db().await;
    PersonService::create(&db, test_person_dto("Existing"))
        .await
        .unwrap();

    seed_data::seed_if_empty(&db).await.unwrap();

    let people = PersonService::get_all(&db).await.unwrap();
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].name, "Existing");
}

// --- MealTemplateService Tests ---

#[tokio::test]
async fn meal_template_create_and_get_all() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    let dto = CreateMealTemplateDto {
        name: "Weeknight Pasta".to_string(),
        meal_type: "Dinner".to_string(),
        servings: vec![PersonServingDto::Recipe {
            person_id: person.id.clone(),
            recipe_id: recipe.id.clone(),
            servings_count: 1.0,
            notes: None,
        }],
    };
    let template = MealTemplateService::create(&db, dto).await.unwrap();
    assert_eq!(template.name, "Weeknight Pasta");
    assert_eq!(template.meal_type, "Dinner");

    let all = MealTemplateService::get_all(&db).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Weeknight Pasta");
}

#[tokio::test]
async fn meal_template_update() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let dto = CreateMealTemplateDto {
        name: "Quick Brekkie".to_string(),
        meal_type: "Breakfast".to_string(),
        servings: vec![PersonServingDto::Adhoc {
            person_id: person.id.clone(),
            adhoc_items: vec![IngredientDto {
                name: "toast".to_string(),
                amount: IngredientAmountDto::Single { value: 2.0 },
                unit: "slices".to_string(),
                notes: None,
            }],
            notes: None,
        }],
    };
    let template = MealTemplateService::create(&db, dto).await.unwrap();

    let update = fewd_lib::dto::UpdateMealTemplateDto {
        name: Some("Sunday Brekkie".to_string()),
        meal_type: None,
        servings: None,
    };
    let updated = MealTemplateService::update(&db, template.id, update)
        .await
        .unwrap();
    assert_eq!(updated.name, "Sunday Brekkie");
    assert_eq!(updated.meal_type, "Breakfast");
}

#[tokio::test]
async fn meal_template_delete() {
    let db = setup_db().await;

    let dto = CreateMealTemplateDto {
        name: "To Delete".to_string(),
        meal_type: "Lunch".to_string(),
        servings: vec![],
    };
    let template = MealTemplateService::create(&db, dto).await.unwrap();
    MealTemplateService::delete(&db, template.id.clone())
        .await
        .unwrap();
    let found = MealTemplateService::get_by_id(&db, template.id)
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn meal_template_create_from_meal() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    // Create a meal first
    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Dinner".to_string(),
        order_index: 2,
        servings: vec![PersonServingDto::Recipe {
            person_id: person.id.clone(),
            recipe_id: recipe.id.clone(),
            servings_count: 2.0,
            notes: Some("extra cheese".to_string()),
        }],
    };
    let meal = MealService::create(&db, meal_dto).await.unwrap();

    // Create template from meal
    let template = MealTemplateService::create_from_meal(&db, meal.id, "Pasta Night".to_string())
        .await
        .unwrap();
    assert_eq!(template.name, "Pasta Night");
    assert_eq!(template.meal_type, "Dinner");

    // Verify servings were copied correctly
    let servings: Vec<PersonServingDto> = serde_json::from_str(&template.servings).unwrap();
    assert_eq!(servings.len(), 1);
    match &servings[0] {
        PersonServingDto::Recipe {
            person_id,
            recipe_id,
            servings_count,
            notes,
        } => {
            assert_eq!(person_id, &person.id);
            assert_eq!(recipe_id, &recipe.id);
            assert_eq!(*servings_count, 2.0);
            assert_eq!(notes, &Some("extra cheese".to_string()));
        }
        _ => panic!("Expected Recipe serving"),
    }
}

// --- SuggestionService Tests ---

#[tokio::test]
async fn suggestion_recent_favorites() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    let pasta = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();
    let soup = RecipeService::create(&db, test_recipe_dto("Soup"))
        .await
        .unwrap();

    let today = chrono::Local::now().date_naive();
    let yesterday = (today - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let two_days_ago = (today - chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();

    // Pasta used twice, soup once
    for date in [&yesterday, &two_days_ago] {
        MealService::create(
            &db,
            CreateMealDto {
                date: date.clone(),
                meal_type: "Dinner".to_string(),
                order_index: 2,
                servings: vec![PersonServingDto::Recipe {
                    person_id: person.id.clone(),
                    recipe_id: pasta.id.clone(),
                    servings_count: 1.0,
                    notes: None,
                }],
            },
        )
        .await
        .unwrap();
    }
    MealService::create(
        &db,
        CreateMealDto {
            date: yesterday.clone(),
            meal_type: "Lunch".to_string(),
            order_index: 1,
            servings: vec![PersonServingDto::Recipe {
                person_id: person.id.clone(),
                recipe_id: soup.id.clone(),
                servings_count: 1.0,
                notes: None,
            }],
        },
    )
    .await
    .unwrap();

    let suggestions = SuggestionService::get_suggestions(&db, vec![person.id.clone()], today)
        .await
        .unwrap();

    assert_eq!(suggestions.recent_favorites.len(), 2);
    // Pasta should be first (used 2 times vs 1)
    assert_eq!(suggestions.recent_favorites[0].recipe_name, "Pasta");
    assert_eq!(suggestions.recent_favorites[1].recipe_name, "Soup");
}

#[tokio::test]
async fn suggestion_forgotten_hits() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Old Favorite"))
        .await
        .unwrap();

    // Manually set times_made, rating, and an old last_made via update
    let update = UpdateRecipeDto {
        name: None,
        description: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        servings: None,
        portion_size: None,
        instructions: None,
        ingredients: None,
        nutrition_per_serving: None,
        tags: None,
        notes: None,
        icon: None,
        is_favorite: None,
        rating: Some(5.0),
    };
    RecipeService::update(&db, recipe.id.clone(), update)
        .await
        .unwrap();

    // We need times_made >= 3. Create 3 meals to trigger increment_recipe_usage.
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let old_date = (chrono::Local::now().date_naive() - chrono::Duration::days(60))
        .format("%Y-%m-%d")
        .to_string();

    for i in 0..3 {
        MealService::create(
            &db,
            CreateMealDto {
                date: old_date.clone(),
                meal_type: "Dinner".to_string(),
                order_index: i,
                servings: vec![PersonServingDto::Recipe {
                    person_id: person.id.clone(),
                    recipe_id: recipe.id.clone(),
                    servings_count: 1.0,
                    notes: None,
                }],
            },
        )
        .await
        .unwrap();
    }

    // increment_recipe_usage sets last_made to Utc::now(), so we need to
    // manually push it back to 60 days ago for the forgotten-hits filter
    let old_last_made = chrono::Utc::now() - chrono::Duration::days(60);
    let fetched = RecipeService::get_by_id(&db, recipe.id.clone())
        .await
        .unwrap()
        .unwrap();
    let mut active: fewd_lib::entities::recipe::ActiveModel = fetched.into_active_model();
    active.last_made = Set(Some(old_last_made));
    fewd_lib::entities::recipe::Entity::update(active)
        .exec(&db)
        .await
        .unwrap();

    let today = chrono::Local::now().date_naive();
    let suggestions = SuggestionService::get_suggestions(&db, vec![person.id.clone()], today)
        .await
        .unwrap();

    assert!(!suggestions.forgotten_hits.is_empty());
    assert_eq!(suggestions.forgotten_hits[0].recipe_name, "Old Favorite");
}

#[tokio::test]
async fn suggestion_untried() {
    let db = setup_db().await;
    let alice = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    let bob = PersonService::create(&db, test_person_dto("Bob"))
        .await
        .unwrap();
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    // Alice has had this recipe, Bob hasn't
    let today_str = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    MealService::create(
        &db,
        CreateMealDto {
            date: today_str,
            meal_type: "Dinner".to_string(),
            order_index: 2,
            servings: vec![PersonServingDto::Recipe {
                person_id: alice.id.clone(),
                recipe_id: recipe.id.clone(),
                servings_count: 1.0,
                notes: None,
            }],
        },
    )
    .await
    .unwrap();

    let today = chrono::Local::now().date_naive();

    // For Bob, the recipe should appear as untried
    let bob_suggestions = SuggestionService::get_suggestions(&db, vec![bob.id.clone()], today)
        .await
        .unwrap();
    let untried_ids: Vec<&str> = bob_suggestions
        .untried
        .iter()
        .map(|s| s.recipe_id.as_str())
        .collect();
    assert!(untried_ids.contains(&recipe.id.as_str()));

    // For Alice, the recipe should NOT appear as untried
    let alice_suggestions = SuggestionService::get_suggestions(&db, vec![alice.id.clone()], today)
        .await
        .unwrap();
    let alice_untried_ids: Vec<&str> = alice_suggestions
        .untried
        .iter()
        .map(|s| s.recipe_id.as_str())
        .collect();
    assert!(!alice_untried_ids.contains(&recipe.id.as_str()));
}

#[tokio::test]
async fn suggestion_empty_history() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    let today = chrono::Local::now().date_naive();
    let suggestions = SuggestionService::get_suggestions(&db, vec![person.id.clone()], today)
        .await
        .unwrap();

    // No meals → no recent favorites
    assert!(suggestions.recent_favorites.is_empty());
    // Recipe should appear as untried
    assert_eq!(suggestions.untried.len(), 1);
    assert_eq!(suggestions.untried[0].recipe_id, recipe.id);
}

// --- SettingsService Tests ---

#[tokio::test]
async fn settings_set_and_get() {
    let db = setup_db().await;
    SettingsService::set(&db, "test_key".to_string(), "test_value".to_string())
        .await
        .unwrap();
    let value = SettingsService::get(&db, "test_key".to_string())
        .await
        .unwrap();
    assert_eq!(value, Some("test_value".to_string()));
}

#[tokio::test]
async fn settings_get_nonexistent_returns_none() {
    let db = setup_db().await;
    let value = SettingsService::get(&db, "missing_key".to_string())
        .await
        .unwrap();
    assert!(value.is_none());
}

#[tokio::test]
async fn settings_set_overwrites_existing() {
    let db = setup_db().await;
    SettingsService::set(&db, "key".to_string(), "old_value".to_string())
        .await
        .unwrap();
    SettingsService::set(&db, "key".to_string(), "new_value".to_string())
        .await
        .unwrap();
    let value = SettingsService::get(&db, "key".to_string()).await.unwrap();
    assert_eq!(value, Some("new_value".to_string()));
}

#[tokio::test]
async fn settings_delete() {
    let db = setup_db().await;
    SettingsService::set(&db, "key".to_string(), "value".to_string())
        .await
        .unwrap();
    SettingsService::delete(&db, "key".to_string())
        .await
        .unwrap();
    let value = SettingsService::get(&db, "key".to_string()).await.unwrap();
    assert!(value.is_none());
}

// --- PromptBuilder Tests ---

#[tokio::test]
async fn prompt_builder_person_context() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let context = PromptBuilder::build_person_context(&[person]);

    assert!(context.contains("Alice"));
    assert!(context.contains("olives"));
    assert!(context.contains("pasta"));
}

#[tokio::test]
async fn prompt_builder_recipe_context() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Chicken Tacos"))
        .await
        .unwrap();

    let context = PromptBuilder::build_recipe_context(&recipe);

    assert!(context.contains("Chicken Tacos"));
    assert!(context.contains("flour"));
    assert!(context.contains("eggs"));
    assert!(context.contains("4")); // servings
}

#[tokio::test]
async fn prompt_builder_meal_history_context() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    let recipe = RecipeService::create(&db, test_recipe_dto("Pasta"))
        .await
        .unwrap();

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Dinner".to_string(),
        order_index: 2,
        servings: vec![PersonServingDto::Recipe {
            person_id: person.id.clone(),
            recipe_id: recipe.id.clone(),
            servings_count: 1.0,
            notes: None,
        }],
    };
    let meal = MealService::create(&db, meal_dto).await.unwrap();

    let context = PromptBuilder::build_meal_history_context(&[meal], &[recipe]);

    assert!(context.contains("2025-06-10"));
    assert!(context.contains("Dinner"));
    assert!(context.contains("Pasta"));
}

// --- Recipe Adapter Tests ---

#[test]
fn adapter_build_system_prompt_contains_schema() {
    let prompt = RecipeAdapter::build_system_prompt();
    assert!(prompt.contains("Return ONLY valid JSON"));
    assert!(prompt.contains("\"source\": \"ai_adapted\""));
    assert!(prompt.contains("\"ingredients\""));
    assert!(prompt.contains("\"instructions\""));
    assert!(prompt.contains("\"tags\""));
}

#[tokio::test]
async fn adapter_build_user_message_includes_context() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Steve"))
        .await
        .unwrap();
    let recipe = RecipeService::create(&db, test_recipe_dto("Tacos"))
        .await
        .unwrap();

    let options = vec![PersonAdaptOptions {
        person_id: person.id.clone(),
        include_dietary_goals: true,
        include_dislikes: true,
        include_favorites: true,
    }];

    let message = RecipeAdapter::build_user_message(&recipe, &[person], &options, "Make it spicy");

    assert!(message.contains("Tacos"));
    assert!(message.contains("Steve"));
    assert!(message.contains("Make it spicy"));
}

#[tokio::test]
async fn adapter_filtered_people_excludes_dislikes() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();

    let options = vec![PersonAdaptOptions {
        person_id: person.id.clone(),
        include_dietary_goals: true,
        include_dislikes: false,
        include_favorites: true,
    }];

    let filtered = RecipeAdapter::build_filtered_people(&[person], &options);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].dislikes, "[]");
}

#[test]
fn adapter_parse_response_valid_json() {
    let json = r#"{
        "name": "Keto Tacos",
        "description": "Low-carb tacos",
        "source": "something_wrong",
        "parent_recipe_id": null,
        "servings": 4,
        "instructions": "Step 1: Cook meat",
        "ingredients": [{"name": "ground beef", "amount": {"type": "single", "value": 1.0}, "unit": "lb", "notes": null}],
        "tags": ["keto"],
        "notes": "Adapted for keto",
        "icon": null,
        "nutrition_per_serving": null,
        "prep_time": null,
        "cook_time": null,
        "total_time": null,
        "portion_size": null
    }"#;

    let result = RecipeAdapter::parse_response(json, "original-123").unwrap();
    assert_eq!(result.name, "Keto Tacos");
    assert_eq!(result.source, "ai_adapted");
    assert_eq!(result.parent_recipe_id, Some("original-123".to_string()));
}

#[test]
fn adapter_parse_response_strips_markdown_fences() {
    let json = "```json\n{\"name\": \"Test\", \"source\": \"x\", \"servings\": 2, \"instructions\": \"Do it\", \"ingredients\": [], \"tags\": []}\n```";

    let result = RecipeAdapter::parse_response(json, "parent-1").unwrap();
    assert_eq!(result.name, "Test");
    assert_eq!(result.source, "ai_adapted");
}

#[test]
fn adapter_parse_response_invalid_json() {
    let result = RecipeAdapter::parse_response("not json at all", "id-1");
    assert!(result.is_err());
}

#[tokio::test]
async fn settings_increment_token_usage() {
    let db = setup_db().await;

    // Set initial values
    SettingsService::set(&db, "token_usage_input".to_string(), "100".to_string())
        .await
        .unwrap();
    SettingsService::set(&db, "token_usage_output".to_string(), "50".to_string())
        .await
        .unwrap();
    SettingsService::set(&db, "token_usage_requests".to_string(), "3".to_string())
        .await
        .unwrap();

    // Increment
    SettingsService::increment_token_usage(&db, 200, 75).await;

    let input = SettingsService::get(&db, "token_usage_input".to_string())
        .await
        .unwrap()
        .unwrap();
    let output = SettingsService::get(&db, "token_usage_output".to_string())
        .await
        .unwrap()
        .unwrap();
    let requests = SettingsService::get(&db, "token_usage_requests".to_string())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(input, "300");
    assert_eq!(output, "125");
    assert_eq!(requests, "4");
}

// ===== AI Suggestion Service Tests =====

#[test]
fn ai_suggestion_meal_character_deserialize() {
    use fewd_lib::services::ai_suggestion_service::MealCharacter;

    let balanced: MealCharacter = serde_json::from_str(r#"{"type":"balanced"}"#).unwrap();
    assert!(matches!(balanced, MealCharacter::Balanced));

    let indulgent: MealCharacter = serde_json::from_str(r#"{"type":"indulgent"}"#).unwrap();
    assert!(matches!(indulgent, MealCharacter::Indulgent));

    let quick: MealCharacter = serde_json::from_str(r#"{"type":"quick"}"#).unwrap();
    assert!(matches!(quick, MealCharacter::Quick));

    let custom: MealCharacter =
        serde_json::from_str(r#"{"type":"custom","text":"high protein"}"#).unwrap();
    assert!(matches!(custom, MealCharacter::Custom { text } if text == "high protein"));
}

#[test]
fn ai_suggestion_build_system_prompt_contains_schema() {
    use fewd_lib::services::ai_suggestion_service::AiSuggestionService;

    let prompt = AiSuggestionService::build_system_prompt();
    assert!(prompt.contains("ai_suggested"));
    assert!(prompt.contains("ingredients"));
    assert!(prompt.contains("3-5"));
    assert!(prompt.contains("Return ONLY a valid JSON array"));
}

#[test]
fn ai_suggestion_build_user_message_includes_context() {
    use fewd_lib::services::ai_suggestion_service::{
        AiSuggestionService, MealCharacter, SuggestionContext,
    };

    let ctx = SuggestionContext {
        people: &[],
        person_options: &[],
        meal_type: "Dinner",
        character: &MealCharacter::Indulgent,
        meals: &[],
        recipes: &[],
        feedback: None,
        previous_suggestions: None,
    };
    let message = AiSuggestionService::build_user_message(&ctx);

    assert!(message.contains("Dinner"));
    assert!(message.contains("Indulgent"));
}

// --- Shopping List Serving Mismatch Tests ---

#[tokio::test]
async fn shopping_list_recipe_source_has_serving_info() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Tacos"))
        .await
        .unwrap();
    let person = PersonService::create(&db, test_person_dto("Bob"))
        .await
        .unwrap();

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Dinner".to_string(),
        order_index: 2,
        servings: vec![PersonServingDto::Recipe {
            person_id: person.id,
            recipe_id: recipe.id,
            servings_count: 1.0,
            notes: None,
        }],
    };
    MealService::create(&db, meal_dto).await.unwrap();

    let list =
        ShoppingService::get_shopping_list(&db, "2025-06-09".to_string(), "2025-06-15".to_string())
            .await
            .unwrap();

    assert!(!list.is_empty());
    for agg in &list {
        for source in &agg.items {
            assert_eq!(source.recipe_servings, Some(4));
            assert_eq!(source.person_servings, Some(1.0));
        }
    }
}

#[tokio::test]
async fn shopping_list_adhoc_source_has_no_serving_info() {
    let db = setup_db().await;
    let person = PersonService::create(&db, test_person_dto("Carol"))
        .await
        .unwrap();

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Snack".to_string(),
        order_index: 3,
        servings: vec![PersonServingDto::Adhoc {
            person_id: person.id,
            adhoc_items: vec![IngredientDto {
                name: "apple".to_string(),
                amount: IngredientAmountDto::Single { value: 1.0 },
                unit: "whole".to_string(),
                notes: None,
            }],
            notes: None,
        }],
    };
    MealService::create(&db, meal_dto).await.unwrap();

    let list =
        ShoppingService::get_shopping_list(&db, "2025-06-09".to_string(), "2025-06-15".to_string())
            .await
            .unwrap();

    assert_eq!(list.len(), 1);
    assert!(list[0].items[0].recipe_servings.is_none());
    assert!(list[0].items[0].person_servings.is_none());
}

#[tokio::test]
async fn shopping_list_multiple_people_same_recipe() {
    let db = setup_db().await;
    let recipe = RecipeService::create(&db, test_recipe_dto("Curry"))
        .await
        .unwrap();
    let alice = PersonService::create(&db, test_person_dto("Alice"))
        .await
        .unwrap();
    let bob = PersonService::create(&db, test_person_dto("Bob"))
        .await
        .unwrap();

    let meal_dto = CreateMealDto {
        date: "2025-06-10".to_string(),
        meal_type: "Dinner".to_string(),
        order_index: 2,
        servings: vec![
            PersonServingDto::Recipe {
                person_id: alice.id,
                recipe_id: recipe.id.clone(),
                servings_count: 1.0,
                notes: None,
            },
            PersonServingDto::Recipe {
                person_id: bob.id,
                recipe_id: recipe.id,
                servings_count: 1.5,
                notes: None,
            },
        ],
    };
    MealService::create(&db, meal_dto).await.unwrap();

    let list =
        ShoppingService::get_shopping_list(&db, "2025-06-09".to_string(), "2025-06-15".to_string())
            .await
            .unwrap();

    // Sources from same meal+recipe are merged into one line
    for agg in &list {
        assert_eq!(agg.items.len(), 1);
        assert_eq!(agg.items[0].person_servings, Some(2.5)); // 1.0 + 1.5
        assert_eq!(agg.items[0].recipe_servings, Some(4));
    }
}
