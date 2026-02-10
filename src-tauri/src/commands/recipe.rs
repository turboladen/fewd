use serde::{Deserialize, Serialize};
use tauri::State;

use crate::entities::recipe;
use crate::services::claude_client::ClaudeClient;
use crate::services::person_service::PersonService;
use crate::services::recipe_adapter::{PersonAdaptOptions, RecipeAdapter};
use crate::services::recipe_enhancer;
use crate::services::recipe_import_service::RecipeImportService;
use crate::services::recipe_parser::RecipeParser;
use crate::services::recipe_scaler;
use crate::services::recipe_service::RecipeService;
use crate::services::settings_service::SettingsService;
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
    Single {
        #[serde(deserialize_with = "deserialize_f64_or_null")]
        value: f64,
    },
    #[serde(rename = "range")]
    Range {
        #[serde(deserialize_with = "deserialize_f64_or_null")]
        min: f64,
        #[serde(deserialize_with = "deserialize_f64_or_null")]
        max: f64,
    },
}

/// Deserialize an f64 that may be null (AI sometimes returns null for "to taste" amounts)
fn deserialize_f64_or_null<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<f64>::deserialize(deserializer).map(|opt| opt.unwrap_or(0.0))
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IngredientDto {
    pub name: String,
    pub amount: IngredientAmountDto,
    #[serde(default)]
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
    pub rating: Option<f64>,
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

#[tauri::command]
pub async fn preview_scale_recipe(
    state: State<'_, AppState>,
    id: String,
    new_servings: i32,
) -> Result<recipe_scaler::ScaleResult, String> {
    if new_servings < 1 {
        return Err("Servings must be at least 1".to_string());
    }

    let recipe = RecipeService::get_by_id(&state.db, id)
        .await
        .map_err(|e| {
            eprintln!("Failed to get recipe for scaling: {}", e);
            format!("Could not get recipe: {}", e)
        })?
        .ok_or_else(|| "Recipe not found".to_string())?;

    let ingredients: Vec<IngredientDto> =
        serde_json::from_str(&recipe.ingredients).map_err(|e| {
            eprintln!("Failed to parse recipe ingredients: {}", e);
            format!("Could not parse ingredients: {}", e)
        })?;

    let ratio = new_servings as f64 / recipe.servings as f64;
    Ok(recipe_scaler::scale_ingredients(&ingredients, ratio))
}

#[tauri::command]
pub async fn enhance_recipe_instructions(
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let recipe = RecipeService::get_by_id(&state.db, id)
        .await
        .map_err(|e| {
            eprintln!("Failed to get recipe for enhancement: {}", e);
            format!("Could not get recipe: {}", e)
        })?
        .ok_or_else(|| "Recipe not found".to_string())?;

    let ingredients: Vec<IngredientDto> =
        serde_json::from_str(&recipe.ingredients).map_err(|e| {
            eprintln!("Failed to parse recipe ingredients: {}", e);
            format!("Could not parse ingredients: {}", e)
        })?;

    Ok(recipe_enhancer::enhance_instructions(
        &ingredients,
        &recipe.instructions,
    ))
}

// --- AI Adaptation ---

#[derive(Debug, Deserialize)]
pub struct AdaptRecipeDto {
    pub recipe_id: String,
    pub person_options: Vec<PersonAdaptOptions>,
    pub user_instructions: String,
}

#[tauri::command]
pub async fn adapt_recipe(
    state: State<'_, AppState>,
    data: AdaptRecipeDto,
) -> Result<CreateRecipeDto, String> {
    // Fetch API key
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(|e| format!("Failed to read API key: {}", e))?
        .ok_or_else(|| "No API key configured. Set it in Settings.".to_string())?;
    if api_key.is_empty() {
        return Err("No API key configured. Set it in Settings.".to_string());
    }

    // Fetch model (or use default)
    let model = SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .map_err(|e| format!("Failed to read model: {}", e))?
        .unwrap_or_else(|| ClaudeClient::default_model().to_string());

    // Fetch recipe
    let recipe = RecipeService::get_by_id(&state.db, data.recipe_id.clone())
        .await
        .map_err(|e| {
            eprintln!("Failed to get recipe for adaptation: {}", e);
            format!("Could not get recipe: {}", e)
        })?
        .ok_or_else(|| "Recipe not found".to_string())?;

    // Fetch people
    let mut people = Vec::new();
    for opt in &data.person_options {
        let person = PersonService::get_by_id(&state.db, opt.person_id.clone())
            .await
            .map_err(|e| {
                eprintln!("Failed to get person: {}", e);
                format!("Could not get person: {}", e)
            })?
            .ok_or_else(|| format!("Person {} not found", opt.person_id))?;
        people.push(person);
    }

    // Call the adapter
    let result = RecipeAdapter::adapt_recipe(
        &api_key,
        &model,
        &recipe,
        &people,
        &data.person_options,
        &data.user_instructions,
    )
    .await
    .map_err(|e| {
        eprintln!("Recipe adaptation failed: {}", e);
        format!("Adaptation failed: {}", e)
    })?;

    // Increment token usage
    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    Ok(result.recipe)
}

// --- AI Import ---

#[derive(Debug, Deserialize)]
pub struct ImportRecipeFromUrlDto {
    pub url: String,
}

#[tauri::command]
pub async fn import_recipe_from_url(
    state: State<'_, AppState>,
    data: ImportRecipeFromUrlDto,
) -> Result<recipe::Model, String> {
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(|e| format!("Failed to read API key: {}", e))?
        .ok_or_else(|| "No API key configured. Set it in Settings.".to_string())?;
    if api_key.is_empty() {
        return Err("No API key configured. Set it in Settings.".to_string());
    }

    let model = SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .map_err(|e| format!("Failed to read model: {}", e))?
        .unwrap_or_else(|| ClaudeClient::default_model().to_string());

    eprintln!("Importing recipe from URL: {}", data.url);

    let result = RecipeImportService::import_from_url(&api_key, &model, &data.url)
        .await
        .map_err(|e| {
            eprintln!("URL import failed: {}", e);
            format!("Import failed: {}", e)
        })?;

    eprintln!(
        "Import tokens — input: {}, output: {}",
        result.input_tokens, result.output_tokens
    );

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    RecipeService::create(&state.db, result.recipe)
        .await
        .map_err(|e| {
            eprintln!("Failed to save imported recipe: {}", e);
            format!("Could not save recipe: {}", e)
        })
}

#[derive(Debug, Deserialize)]
pub struct ImportRecipeFromFileDto {
    pub file_path: String,
}

#[tauri::command]
pub async fn import_recipe_from_file(
    state: State<'_, AppState>,
    data: ImportRecipeFromFileDto,
) -> Result<recipe::Model, String> {
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(|e| format!("Failed to read API key: {}", e))?
        .ok_or_else(|| "No API key configured. Set it in Settings.".to_string())?;
    if api_key.is_empty() {
        return Err("No API key configured. Set it in Settings.".to_string());
    }

    let model = SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .map_err(|e| format!("Failed to read model: {}", e))?
        .unwrap_or_else(|| ClaudeClient::default_model().to_string());

    eprintln!("Importing recipe from file: {}", data.file_path);

    let result = RecipeImportService::import_from_pdf(&api_key, &model, &data.file_path)
        .await
        .map_err(|e| {
            eprintln!("File import failed: {}", e);
            format!("Import failed: {}", e)
        })?;

    eprintln!(
        "Import tokens — input: {}, output: {}",
        result.input_tokens, result.output_tokens
    );

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    RecipeService::create(&state.db, result.recipe)
        .await
        .map_err(|e| {
            eprintln!("Failed to save imported recipe: {}", e);
            format!("Could not save recipe: {}", e)
        })
}
