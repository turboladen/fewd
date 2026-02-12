pub mod ai_suggestion_service;
pub mod claude_client;
pub mod meal_service;
pub mod meal_template_service;
pub mod person_service;
pub mod prompt_builder;
pub mod recipe_adapter;
pub mod recipe_enhancer;
pub mod recipe_import_service;
pub mod recipe_parser;
pub mod recipe_scaler;
pub mod recipe_service;
pub mod seed_data;
pub mod settings_service;
pub mod shopping_service;
pub mod suggestion_service;
pub mod unit_converter;

pub fn to_json<T: serde::Serialize>(value: &T) -> Result<String, sea_orm::DbErr> {
    serde_json::to_string(value)
        .map_err(|e| sea_orm::DbErr::Custom(format!("JSON serialization error: {}", e)))
}
