use axum::extract::State;
use axum::Json;

use crate::dto::{AiSuggestMealsDto, CreateRecipeDto, GetSuggestionsDto};
use crate::error::AppError;
use crate::services::ai_suggestion_service::{AiSuggestionService, SuggestionContext};
use crate::services::claude_client::ClaudeClient;
use crate::services::meal_service::MealService;
use crate::services::person_service::PersonService;
use crate::services::recipe_service::RecipeService;
use crate::services::settings_service::SettingsService;
use crate::services::suggestion_service::{MealSuggestions, SuggestionService};
use crate::AppState;

pub async fn get_suggestions(
    State(state): State<AppState>,
    Json(data): Json<GetSuggestionsDto>,
) -> Result<Json<MealSuggestions>, AppError> {
    let reference_date = chrono::NaiveDate::parse_from_str(&data.reference_date, "%Y-%m-%d")
        .map_err(|e| AppError::BadRequest(format!("Invalid reference_date: {}", e)))?;

    SuggestionService::get_suggestions(&state.db, data.person_ids, reference_date)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn ai_suggest(
    State(state): State<AppState>,
    Json(data): Json<AiSuggestMealsDto>,
) -> Result<Json<Vec<CreateRecipeDto>>, AppError> {
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

    let mut people = Vec::new();
    for opt in &data.person_options {
        let person = PersonService::get_by_id(&state.db, opt.person_id.clone())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound(format!("Person {} not found", opt.person_id)))?;
        people.push(person);
    }

    let today = chrono::Utc::now().date_naive();
    let start = today - chrono::Duration::days(30);
    let meals = MealService::get_all_for_date_range(
        &state.db,
        start.format("%Y-%m-%d").to_string(),
        today.format("%Y-%m-%d").to_string(),
    )
    .await
    .map_err(AppError::from)?;

    let recipes = RecipeService::get_all(&state.db).await.map_err(AppError::from)?;

    let ctx = SuggestionContext {
        people: &people,
        person_options: &data.person_options,
        meal_type: &data.meal_type,
        character: &data.character,
        meals: &meals,
        recipes: &recipes,
        feedback: data.feedback.as_deref(),
        previous_suggestions: data
            .previous_suggestion_names
            .as_deref()
            .map(|v| v as &[String]),
    };

    let result = AiSuggestionService::suggest_meals(&api_key, &model, &ctx)
        .await
        .map_err(|e| AppError::Internal(format!("AI suggestion failed: {}", e)))?;

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    Ok(Json(result.suggestions))
}
