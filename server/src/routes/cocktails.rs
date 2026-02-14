use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use crate::dto::{
    AiSuggestCocktailsDto, BulkBarItemsDto, CreateBarItemDto, CreateDrinkRecipeDto,
    ImportRecipeFromUrlDto, UpdateDrinkRecipeDto,
};
use crate::error::AppError;
use crate::services::bar_item_service::BarItemService;
use crate::services::claude_client::ClaudeClient;
use crate::services::cocktail_suggestion_service::{CocktailContext, CocktailSuggestionService};
use crate::services::drink_recipe_import_service::DrinkRecipeImportService;
use crate::services::drink_recipe_service::DrinkRecipeService;
use crate::services::person_service::PersonService;
use crate::services::settings_service::SettingsService;
use crate::AppState;

// ─── Bar Items ─────────────────────────────────────────────────

pub async fn list_bar_items(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::entities::bar_item::Model>>, AppError> {
    BarItemService::get_all(&state.db)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn create_bar_item(
    State(state): State<AppState>,
    Json(data): Json<CreateBarItemDto>,
) -> Result<(StatusCode, Json<crate::entities::bar_item::Model>), AppError> {
    BarItemService::create(&state.db, data)
        .await
        .map(|item| (StatusCode::CREATED, Json(item)))
        .map_err(AppError::from)
}

pub async fn bulk_create_bar_items(
    State(state): State<AppState>,
    Json(data): Json<BulkBarItemsDto>,
) -> Result<(StatusCode, Json<Vec<crate::entities::bar_item::Model>>), AppError> {
    BarItemService::bulk_create(&state.db, data)
        .await
        .map(|items| (StatusCode::CREATED, Json(items)))
        .map_err(AppError::from)
}

pub async fn delete_bar_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    BarItemService::delete(&state.db, id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub async fn delete_all_bar_items(State(state): State<AppState>) -> Result<StatusCode, AppError> {
    BarItemService::delete_all(&state.db)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

// ─── Drink Recipes ─────────────────────────────────────────────

pub async fn list_drink_recipes(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::entities::drink_recipe::Model>>, AppError> {
    DrinkRecipeService::get_all(&state.db)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn get_drink_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::entities::drink_recipe::Model>, AppError> {
    DrinkRecipeService::get_by_id(&state.db, id)
        .await
        .map_err(AppError::from)?
        .map(Json)
        .ok_or_else(|| AppError::NotFound("Drink recipe not found".into()))
}

pub async fn create_drink_recipe(
    State(state): State<AppState>,
    Json(data): Json<CreateDrinkRecipeDto>,
) -> Result<(StatusCode, Json<crate::entities::drink_recipe::Model>), AppError> {
    DrinkRecipeService::create(&state.db, data)
        .await
        .map(|recipe| (StatusCode::CREATED, Json(recipe)))
        .map_err(AppError::from)
}

pub async fn update_drink_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(data): Json<UpdateDrinkRecipeDto>,
) -> Result<Json<crate::entities::drink_recipe::Model>, AppError> {
    DrinkRecipeService::update(&state.db, id, data)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn delete_drink_recipe(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    DrinkRecipeService::delete(&state.db, id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub async fn toggle_drink_favorite(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::entities::drink_recipe::Model>, AppError> {
    DrinkRecipeService::toggle_favorite(&state.db, id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

// ─── Drink Recipe Import ──────────────────────────────────────

pub async fn import_drink_recipe_url(
    State(state): State<AppState>,
    Json(data): Json<ImportRecipeFromUrlDto>,
) -> Result<(StatusCode, Json<crate::entities::drink_recipe::Model>), AppError> {
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::BadRequest("No API key configured. Set it in Settings.".into()))?;

    if api_key.is_empty() {
        return Err(AppError::BadRequest(
            "No API key configured. Set it in Settings.".into(),
        ));
    }

    let model = SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| ClaudeClient::default_model().to_string());

    let result = DrinkRecipeImportService::import_from_url(&api_key, &model, &data.url)
        .await
        .map_err(|e| AppError::Internal(format!("Import failed: {}", e)))?;

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    let mut recipe = result.recipe;
    recipe.source_url = Some(data.url);

    DrinkRecipeService::create(&state.db, recipe)
        .await
        .map(|r| (StatusCode::CREATED, Json(r)))
        .map_err(AppError::from)
}

// ─── AI Cocktail Suggestions ───────────────────────────────────

pub async fn ai_suggest_cocktails(
    State(state): State<AppState>,
    Json(data): Json<AiSuggestCocktailsDto>,
) -> Result<Json<Vec<CreateDrinkRecipeDto>>, AppError> {
    let api_key = SettingsService::get(&state.db, "anthropic_api_key".to_string())
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::BadRequest("No API key configured. Set it in Settings.".into()))?;

    if api_key.is_empty() {
        return Err(AppError::BadRequest(
            "No API key configured. Set it in Settings.".into(),
        ));
    }

    let model = SettingsService::get(&state.db, "claude_model".to_string())
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| ClaudeClient::default_model().to_string());

    // Fetch selected people
    let mut people = Vec::new();
    for person_id in &data.person_ids {
        let person = PersonService::get_by_id(&state.db, person_id.clone())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound(format!("Person {} not found", person_id)))?;
        people.push(person);
    }

    // Fetch selected bar items
    let all_bar_items = BarItemService::get_all(&state.db)
        .await
        .map_err(AppError::from)?;
    let selected_bar_items: Vec<_> = all_bar_items
        .into_iter()
        .filter(|item| data.bar_item_ids.contains(&item.id))
        .collect();

    // Fetch existing drink recipes for context
    let drink_recipes = DrinkRecipeService::get_all(&state.db)
        .await
        .map_err(AppError::from)?;

    let ctx = CocktailContext {
        people: &people,
        bar_items: &selected_bar_items,
        mood: &data.mood,
        include_non_alcoholic: data.include_non_alcoholic,
        drink_recipes: &drink_recipes,
        feedback: data.feedback.as_deref(),
        previous_suggestions: data
            .previous_suggestion_names
            .as_deref()
            .map(|v| v as &[String]),
    };

    let result = CocktailSuggestionService::suggest_cocktails(&api_key, &model, &ctx)
        .await
        .map_err(|e| AppError::Internal(format!("Cocktail suggestion failed: {}", e)))?;

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    Ok(Json(result.suggestions))
}
