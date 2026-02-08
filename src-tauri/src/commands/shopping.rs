use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::recipe::IngredientAmountDto;
use crate::services::shopping_service::ShoppingService;
use crate::AppState;

// --- Types ---

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum SourceType {
    #[serde(rename = "recipe")]
    Recipe,
    #[serde(rename = "adhoc")]
    Adhoc,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IngredientSourceDto {
    pub amount: IngredientAmountDto,
    pub unit: String,
    pub source_type: SourceType,
    pub source_name: Option<String>,
    pub meal_id: String,
    pub meal_date: String,
    pub meal_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AggregatedIngredientDto {
    pub ingredient_name: String,
    pub total_amount: Option<IngredientAmountDto>,
    pub total_unit: Option<String>,
    pub items: Vec<IngredientSourceDto>,
}

// --- Commands ---

#[tauri::command]
pub async fn get_shopping_list(
    state: State<'_, AppState>,
    start_date: String,
    end_date: String,
) -> Result<Vec<AggregatedIngredientDto>, String> {
    ShoppingService::get_shopping_list(&state.db, start_date, end_date)
        .await
        .map_err(|e| {
            eprintln!("Failed to get shopping list: {}", e);
            format!("Could not get shopping list: {}", e)
        })
}
