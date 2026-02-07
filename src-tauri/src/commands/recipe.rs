use serde::{Deserialize, Serialize};
use tauri::State;

use crate::entities::recipe;
use crate::services::recipe_parser::RecipeParser;
use crate::services::recipe_service::RecipeService;
use crate::AppState;

// --- Nested types ---

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TimeValueDto {
    pub value: i32,
    pub unit: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortionSizeDto {
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum IngredientAmountDto {
    #[serde(rename = "single")]
    Single { value: f64 },
    #[serde(rename = "range")]
    Range { min: f64, max: f64 },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IngredientDto {
    pub name: String,
    pub amount: IngredientAmountDto,
    pub unit: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NutritionDto {
    pub calories: Option<i32>,
    pub protein_grams: Option<i32>,
    pub carbs_grams: Option<i32>,
    pub fat_grams: Option<i32>,
    pub notes: Option<String>,
}

// --- DTOs ---

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateRecipeDto {
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub parent_recipe_id: Option<String>,
    pub prep_time: Option<TimeValueDto>,
    pub cook_time: Option<TimeValueDto>,
    pub total_time: Option<TimeValueDto>,
    pub servings: i32,
    pub portion_size: Option<PortionSizeDto>,
    pub instructions: String,
    pub ingredients: Vec<IngredientDto>,
    pub nutrition_per_serving: Option<NutritionDto>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateRecipeDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub prep_time: Option<TimeValueDto>,
    pub cook_time: Option<TimeValueDto>,
    pub total_time: Option<TimeValueDto>,
    pub servings: Option<i32>,
    pub portion_size: Option<PortionSizeDto>,
    pub instructions: Option<String>,
    pub ingredients: Option<Vec<IngredientDto>>,
    pub nutrition_per_serving: Option<NutritionDto>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub is_favorite: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImportRecipeDto {
    pub markdown: String,
}

// --- Commands ---

#[tauri::command]
pub async fn get_all_recipes(state: State<'_, AppState>) -> Result<Vec<recipe::Model>, String> {
    RecipeService::get_all(&state.db).await.map_err(|e| {
        eprintln!("Failed to get all recipes: {}", e);
        format!("Could not get recipes: {}", e)
    })
}

#[tauri::command]
pub async fn get_recipe(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<recipe::Model>, String> {
    RecipeService::get_by_id(&state.db, id).await.map_err(|e| {
        eprintln!("Failed to get recipe: {}", e);
        format!("Could not get recipe: {}", e)
    })
}

#[tauri::command]
pub async fn create_recipe(
    state: State<'_, AppState>,
    data: CreateRecipeDto,
) -> Result<recipe::Model, String> {
    RecipeService::create(&state.db, data).await.map_err(|e| {
        eprintln!("Failed to create recipe: {}", e);
        format!("Could not create recipe: {}", e)
    })
}

#[tauri::command]
pub async fn update_recipe(
    state: State<'_, AppState>,
    id: String,
    data: UpdateRecipeDto,
) -> Result<recipe::Model, String> {
    RecipeService::update(&state.db, id, data)
        .await
        .map_err(|e| {
            eprintln!("Failed to update recipe: {}", e);
            format!("Could not update recipe: {}", e)
        })
}

#[tauri::command]
pub async fn delete_recipe(state: State<'_, AppState>, id: String) -> Result<(), String> {
    RecipeService::delete(&state.db, id).await.map_err(|e| {
        eprintln!("Failed to delete recipe: {}", e);
        format!("Could not delete recipe: {}", e)
    })
}

#[tauri::command]
pub async fn search_recipes(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<recipe::Model>, String> {
    RecipeService::search(&state.db, query).await.map_err(|e| {
        eprintln!("Failed to search recipes: {}", e);
        format!("Could not search recipes: {}", e)
    })
}

#[tauri::command]
pub async fn toggle_favorite_recipe(
    state: State<'_, AppState>,
    id: String,
) -> Result<recipe::Model, String> {
    RecipeService::toggle_favorite(&state.db, id)
        .await
        .map_err(|e| {
            eprintln!("Failed to toggle favorite: {}", e);
            format!("Could not toggle favorite: {}", e)
        })
}

#[tauri::command]
pub async fn import_recipe_from_markdown(
    state: State<'_, AppState>,
    data: ImportRecipeDto,
) -> Result<recipe::Model, String> {
    let dto = RecipeParser::parse_markdown(&data.markdown)
        .map_err(|e| format!("Failed to parse markdown: {}", e))?;

    RecipeService::create(&state.db, dto).await.map_err(|e| {
        eprintln!("Failed to import recipe: {}", e);
        format!("Could not import recipe: {}", e)
    })
}
