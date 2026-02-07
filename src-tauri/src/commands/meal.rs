use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::recipe::IngredientDto;
use crate::entities::meal;
use crate::services::meal_service::MealService;
use crate::AppState;

// --- Nested types ---

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "food_type")]
pub enum PersonServingDto {
    #[serde(rename = "recipe")]
    Recipe {
        person_id: String,
        recipe_id: String,
        servings_count: f64,
        notes: Option<String>,
    },
    #[serde(rename = "adhoc")]
    Adhoc {
        person_id: String,
        adhoc_items: Vec<IngredientDto>,
        notes: Option<String>,
    },
}

// --- DTOs ---

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateMealDto {
    pub date: String,
    pub meal_type: String,
    pub order_index: i32,
    pub servings: Vec<PersonServingDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateMealDto {
    pub date: Option<String>,
    pub meal_type: Option<String>,
    pub order_index: Option<i32>,
    pub servings: Option<Vec<PersonServingDto>>,
}

// --- Commands ---

#[tauri::command]
pub async fn get_meals_for_date_range(
    state: State<'_, AppState>,
    start_date: String,
    end_date: String,
) -> Result<Vec<meal::Model>, String> {
    MealService::get_all_for_date_range(&state.db, start_date, end_date)
        .await
        .map_err(|e| {
            eprintln!("Failed to get meals for date range: {}", e);
            format!("Could not get meals: {}", e)
        })
}

#[tauri::command]
pub async fn get_meal(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<meal::Model>, String> {
    MealService::get_by_id(&state.db, id).await.map_err(|e| {
        eprintln!("Failed to get meal: {}", e);
        format!("Could not get meal: {}", e)
    })
}

#[tauri::command]
pub async fn create_meal(
    state: State<'_, AppState>,
    data: CreateMealDto,
) -> Result<meal::Model, String> {
    MealService::create(&state.db, data).await.map_err(|e| {
        eprintln!("Failed to create meal: {}", e);
        format!("Could not create meal: {}", e)
    })
}

#[tauri::command]
pub async fn update_meal(
    state: State<'_, AppState>,
    id: String,
    data: UpdateMealDto,
) -> Result<meal::Model, String> {
    MealService::update(&state.db, id, data)
        .await
        .map_err(|e| {
            eprintln!("Failed to update meal: {}", e);
            format!("Could not update meal: {}", e)
        })
}

#[tauri::command]
pub async fn delete_meal(state: State<'_, AppState>, id: String) -> Result<(), String> {
    MealService::delete(&state.db, id).await.map_err(|e| {
        eprintln!("Failed to delete meal: {}", e);
        format!("Could not delete meal: {}", e)
    })
}
