use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::meal::PersonServingDto;
use crate::entities::meal_template;
use crate::services::meal_template_service::MealTemplateService;
use crate::AppState;

// --- DTOs ---

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateMealTemplateDto {
    pub name: String,
    pub meal_type: String,
    pub servings: Vec<PersonServingDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateMealTemplateDto {
    pub name: Option<String>,
    pub meal_type: Option<String>,
    pub servings: Option<Vec<PersonServingDto>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateFromMealDto {
    pub meal_id: String,
    pub name: String,
}

// --- Commands ---

#[tauri::command]
pub async fn get_all_meal_templates(
    state: State<'_, AppState>,
) -> Result<Vec<meal_template::Model>, String> {
    MealTemplateService::get_all(&state.db).await.map_err(|e| {
        eprintln!("Failed to get meal templates: {}", e);
        format!("Could not get meal templates: {}", e)
    })
}

#[tauri::command]
pub async fn create_meal_template(
    state: State<'_, AppState>,
    data: CreateMealTemplateDto,
) -> Result<meal_template::Model, String> {
    MealTemplateService::create(&state.db, data)
        .await
        .map_err(|e| {
            eprintln!("Failed to create meal template: {}", e);
            format!("Could not create meal template: {}", e)
        })
}

#[tauri::command]
pub async fn update_meal_template(
    state: State<'_, AppState>,
    id: String,
    data: UpdateMealTemplateDto,
) -> Result<meal_template::Model, String> {
    MealTemplateService::update(&state.db, id, data)
        .await
        .map_err(|e| {
            eprintln!("Failed to update meal template: {}", e);
            format!("Could not update meal template: {}", e)
        })
}

#[tauri::command]
pub async fn delete_meal_template(state: State<'_, AppState>, id: String) -> Result<(), String> {
    MealTemplateService::delete(&state.db, id)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete meal template: {}", e);
            format!("Could not delete meal template: {}", e)
        })
}

#[tauri::command]
pub async fn create_template_from_meal(
    state: State<'_, AppState>,
    data: CreateFromMealDto,
) -> Result<meal_template::Model, String> {
    MealTemplateService::create_from_meal(&state.db, data.meal_id, data.name)
        .await
        .map_err(|e| {
            eprintln!("Failed to create template from meal: {}", e);
            format!("Could not create template from meal: {}", e)
        })
}
