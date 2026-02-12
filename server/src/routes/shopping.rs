use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::dto::AggregatedIngredientDto;
use crate::error::AppError;
use crate::services::shopping_service::ShoppingService;
use crate::AppState;

#[derive(Deserialize)]
pub struct DateRangeQuery {
    pub start_date: String,
    pub end_date: String,
}

pub async fn get_shopping_list(
    State(state): State<AppState>,
    Query(params): Query<DateRangeQuery>,
) -> Result<Json<Vec<AggregatedIngredientDto>>, AppError> {
    ShoppingService::get_shopping_list(&state.db, params.start_date, params.end_date)
        .await
        .map(Json)
        .map_err(AppError::from)
}
