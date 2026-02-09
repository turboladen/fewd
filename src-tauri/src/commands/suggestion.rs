use serde::{Deserialize, Serialize};
use tauri::State;

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
