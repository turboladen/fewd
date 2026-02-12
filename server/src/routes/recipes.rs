use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::dto::{
    AdaptRecipeDto, CreateRecipeDto, ImportRecipeDto, ImportRecipeFromUrlDto, IngredientDto,
    UpdateRecipeDto,
};
use crate::entities::recipe;
use crate::error::AppError;
use crate::services::claude_client::ClaudeClient;
use crate::services::person_service::PersonService;
use crate::services::recipe_enhancer;
use crate::services::recipe_import_service::RecipeImportService;
use crate::services::recipe_parser::RecipeParser;
use crate::services::recipe_scaler;
use crate::services::recipe_service::RecipeService;
use crate::services::settings_service::SettingsService;
use crate::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<recipe::Model>>, AppError> {
    RecipeService::get_all(&state.db).await.map(Json).map_err(AppError::from)
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Option<recipe::Model>>, AppError> {
    RecipeService::get_by_id(&state.db, id).await.map(Json).map_err(AppError::from)
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<recipe::Model>>, AppError> {
    RecipeService::search(&state.db, params.q).await.map(Json).map_err(AppError::from)
}

pub async fn create(
    State(state): State<AppState>,
    Json(data): Json<CreateRecipeDto>,
) -> Result<(StatusCode, Json<recipe::Model>), AppError> {
    RecipeService::create(&state.db, data)
        .await
        .map(|r| (StatusCode::CREATED, Json(r)))
        .map_err(AppError::from)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(data): Json<UpdateRecipeDto>,
) -> Result<Json<recipe::Model>, AppError> {
    RecipeService::update(&state.db, id, data).await.map(Json).map_err(AppError::from)
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    RecipeService::delete(&state.db, id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub async fn toggle_favorite(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<recipe::Model>, AppError> {
    RecipeService::toggle_favorite(&state.db, id).await.map(Json).map_err(AppError::from)
}

pub async fn import_markdown(
    State(state): State<AppState>,
    Json(data): Json<ImportRecipeDto>,
) -> Result<(StatusCode, Json<recipe::Model>), AppError> {
    let dto = RecipeParser::parse_markdown(&data.markdown)
        .map_err(|e| AppError::BadRequest(format!("Failed to parse markdown: {}", e)))?;

    RecipeService::create(&state.db, dto)
        .await
        .map(|r| (StatusCode::CREATED, Json(r)))
        .map_err(AppError::from)
}

#[derive(Deserialize)]
pub struct ScaleBody {
    pub new_servings: i32,
}

pub async fn preview_scale(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ScaleBody>,
) -> Result<Json<recipe_scaler::ScaleResult>, AppError> {
    if body.new_servings < 1 {
        return Err(AppError::BadRequest("Servings must be at least 1".into()));
    }

    let recipe = RecipeService::get_by_id(&state.db, id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("Recipe not found".into()))?;

    let ingredients: Vec<IngredientDto> = serde_json::from_str(&recipe.ingredients)
        .map_err(|e| AppError::Internal(format!("Could not parse ingredients: {}", e)))?;

    let ratio = body.new_servings as f64 / recipe.servings as f64;
    Ok(Json(recipe_scaler::scale_ingredients(&ingredients, ratio)))
}

pub async fn enhance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<String>, AppError> {
    let recipe = RecipeService::get_by_id(&state.db, id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("Recipe not found".into()))?;

    let ingredients: Vec<IngredientDto> = serde_json::from_str(&recipe.ingredients)
        .map_err(|e| AppError::Internal(format!("Could not parse ingredients: {}", e)))?;

    Ok(Json(recipe_enhancer::enhance_instructions(
        &ingredients,
        &recipe.instructions,
    )))
}

pub async fn adapt(
    State(state): State<AppState>,
    Json(data): Json<AdaptRecipeDto>,
) -> Result<Json<CreateRecipeDto>, AppError> {
    let api_key = get_api_key(&state).await?;
    let model = get_model(&state).await;

    let recipe = RecipeService::get_by_id(&state.db, data.recipe_id.clone())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("Recipe not found".into()))?;

    let mut people = Vec::new();
    for opt in &data.person_options {
        let person = PersonService::get_by_id(&state.db, opt.person_id.clone())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound(format!("Person {} not found", opt.person_id)))?;
        people.push(person);
    }

    let result = crate::services::recipe_adapter::RecipeAdapter::adapt_recipe(
        &api_key,
        &model,
        &recipe,
        &people,
        &data.person_options,
        &data.user_instructions,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Adaptation failed: {}", e)))?;

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    Ok(Json(result.recipe))
}

pub async fn import_url(
    State(state): State<AppState>,
    Json(data): Json<ImportRecipeFromUrlDto>,
) -> Result<(StatusCode, Json<recipe::Model>), AppError> {
    let api_key = get_api_key(&state).await?;
    let model = get_model(&state).await;

    let result = RecipeImportService::import_from_url(&api_key, &model, &data.url)
        .await
        .map_err(|e| AppError::Internal(format!("Import failed: {}", e)))?;

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    RecipeService::create(&state.db, result.recipe)
        .await
        .map(|r| (StatusCode::CREATED, Json(r)))
        .map_err(AppError::from)
}

pub async fn import_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<recipe::Model>), AppError> {
    let mut file_bytes = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        if field.name() == Some("file") {
            file_bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Failed to read file: {}", e)))?
                .to_vec();
            break;
        }
    }

    if file_bytes.is_empty() {
        return Err(AppError::BadRequest("No file provided".into()));
    }

    let api_key = get_api_key(&state).await?;
    let model = get_model(&state).await;

    let result = RecipeImportService::import_from_pdf_bytes(&api_key, &model, &file_bytes)
        .await
        .map_err(|e| AppError::Internal(format!("Import failed: {}", e)))?;

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    RecipeService::create(&state.db, result.recipe)
        .await
        .map(|r| (StatusCode::CREATED, Json(r)))
        .map_err(AppError::from)
}

/// Helper: get the Anthropic API key from settings
async fn get_api_key(state: &AppState) -> Result<String, AppError> {
    let key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::BadRequest("No API key configured. Set it in Settings.".into()))?;

    if key.is_empty() {
        return Err(AppError::BadRequest(
            "No API key configured. Set it in Settings.".into(),
        ));
    }
    Ok(key)
}

/// Helper: get the selected Claude model (or default)
async fn get_model(state: &AppState) -> String {
    SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| ClaudeClient::default_model().to_string())
}
