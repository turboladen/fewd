use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::recipe::CreateRecipeDto;
use crate::services::ai_suggestion_service::{
    AiSuggestionService, MealCharacter, SuggestionContext,
};
use crate::services::claude_client::ClaudeClient;
use crate::services::meal_service::MealService;
use crate::services::person_service::PersonService;
use crate::services::recipe_adapter::PersonAdaptOptions;
use crate::services::recipe_service::RecipeService;
use crate::services::settings_service::SettingsService;
use crate::services::suggestion_service::{MealSuggestions, SuggestionService};
use crate::AppState;

#[derive(Debug, Deserialize, Serialize)]
pub struct GetSuggestionsDto {
    pub person_ids: Vec<String>,
    pub reference_date: String,
}

#[tauri::command]
pub async fn get_meal_suggestions(
    state: State<'_, AppState>,
    data: GetSuggestionsDto,
) -> Result<MealSuggestions, String> {
    let reference_date = chrono::NaiveDate::parse_from_str(&data.reference_date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid reference_date: {}", e))?;

    SuggestionService::get_suggestions(&state.db, data.person_ids, reference_date)
        .await
        .map_err(|e| {
            eprintln!("Failed to get meal suggestions: {}", e);
            format!("Could not get suggestions: {}", e)
        })
}

#[derive(Debug, Deserialize)]
pub struct AiSuggestMealsDto {
    pub person_options: Vec<PersonAdaptOptions>,
    pub meal_type: String,
    pub character: MealCharacter,
    pub feedback: Option<String>,
    pub previous_suggestion_names: Option<Vec<String>>,
}

#[tauri::command]
pub async fn ai_suggest_meals(
    state: State<'_, AppState>,
    data: AiSuggestMealsDto,
) -> Result<Vec<CreateRecipeDto>, String> {
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

    // Fetch people
    let mut people = Vec::new();
    for opt in &data.person_options {
        let person = PersonService::get_by_id(&state.db, opt.person_id.clone())
            .await
            .map_err(|e| format!("Could not get person: {}", e))?
            .ok_or_else(|| format!("Person {} not found", opt.person_id))?;
        people.push(person);
    }

    // Fetch recent meals (last 30 days) for history context
    let today = chrono::Utc::now().date_naive();
    let start = today - chrono::Duration::days(30);
    let meals = MealService::get_all_for_date_range(
        &state.db,
        start.format("%Y-%m-%d").to_string(),
        today.format("%Y-%m-%d").to_string(),
    )
    .await
    .map_err(|e| format!("Could not get meals: {}", e))?;

    // Fetch all recipes for meal history name lookup
    let recipes = RecipeService::get_all(&state.db)
        .await
        .map_err(|e| format!("Could not get recipes: {}", e))?;

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

    eprintln!(
        "AI suggestion: calling Claude with model={}, meal_type={}, {} people",
        model,
        data.meal_type,
        data.person_options.len()
    );

    let result = AiSuggestionService::suggest_meals(&api_key, &model, &ctx)
        .await
        .map_err(|e| {
            eprintln!("AI suggestion failed: {}", e);
            format!("AI suggestion failed: {}", e)
        })?;

    eprintln!(
        "AI suggestion: got {} suggestions ({}+{} tokens)",
        result.suggestions.len(),
        result.input_tokens,
        result.output_tokens
    );

    SettingsService::increment_token_usage(&state.db, result.input_tokens, result.output_tokens)
        .await;

    Ok(result.suggestions)
}
