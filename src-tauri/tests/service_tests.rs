use fewd_lib::commands::meal::{CreateMealDto, PersonServingDto};
use fewd_lib::commands::person::CreatePersonDto;
use fewd_lib::commands::recipe::{
    CreateRecipeDto, IngredientAmountDto, IngredientDto, UpdateRecipeDto,
};
use fewd_lib::services::meal_service::MealService;
use fewd_lib::services::person_service::PersonService;
use fewd_lib::services::recipe_enhancer;
use fewd_lib::services::recipe_scaler;
use fewd_lib::services::recipe_service::RecipeService;
use fewd_lib::services::seed_data;
use fewd_lib::services::shopping_service::ShoppingService;
use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection};

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
    }
}

fn test_recipe_dto(name: &str) -> CreateRecipeDto {
    CreateRecipeDto {
        name: name.to_string(),
        description: Some("A test recipe".to_string()),
        source: "manual".to_string(),
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
    let update = fewd_lib::commands::person::UpdatePersonDto {
        name: None,
        birthdate: None,
        dietary_goals: None,
        dislikes: None,
        favorites: None,
        notes: None,
        is_active: Some(false),
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

    let update = fewd_lib::commands::person::UpdatePersonDto {
        name: Some("Alice Updated".to_string()),
        birthdate: None,
        dietary_goals: Some("more protein".to_string()),
        dislikes: None,
        favorites: None,
        notes: None,
        is_active: None,
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

    // Not a whole number
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
        rating: Some(3.5),
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
    assert!(names.contains(&"Steve"));
    assert!(names.contains(&"Amanda"));
    assert!(names.contains(&"Vivienne"));
    assert!(names.contains(&"Cleo"));
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
