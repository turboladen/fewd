use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::dto::{CreateMealDto, UpdateMealDto};
use crate::entities::meal;
use crate::error::AppError;
use crate::services::meal_service::MealService;
use crate::AppState;

#[derive(Deserialize)]
pub struct DateRangeQuery {
    pub start_date: String,
    pub end_date: String,
}

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<DateRangeQuery>,
) -> Result<Json<Vec<meal::Model>>, AppError> {
    MealService::get_all_for_date_range(&state.db, params.start_date, params.end_date)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Option<meal::Model>>, AppError> {
    MealService::get_by_id(&state.db, id).await.map(Json).map_err(AppError::from)
}

pub async fn create(
    State(state): State<AppState>,
    Json(data): Json<CreateMealDto>,
) -> Result<(StatusCode, Json<meal::Model>), AppError> {
    MealService::create(&state.db, data)
        .await
        .map(|m| (StatusCode::CREATED, Json(m)))
        .map_err(AppError::from)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(data): Json<UpdateMealDto>,
) -> Result<Json<meal::Model>, AppError> {
    MealService::update(&state.db, id, data).await.map(Json).map_err(AppError::from)
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    MealService::delete(&state.db, id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}
