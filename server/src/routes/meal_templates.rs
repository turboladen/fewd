use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use crate::dto::{CreateFromMealDto, CreateMealTemplateDto, UpdateMealTemplateDto};
use crate::entities::meal_template;
use crate::error::AppError;
use crate::services::meal_template_service::MealTemplateService;
use crate::AppState;

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<meal_template::Model>>, AppError> {
    MealTemplateService::get_all(&state.db).await.map(Json).map_err(AppError::from)
}

pub async fn create(
    State(state): State<AppState>,
    Json(data): Json<CreateMealTemplateDto>,
) -> Result<(StatusCode, Json<meal_template::Model>), AppError> {
    MealTemplateService::create(&state.db, data)
        .await
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(AppError::from)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(data): Json<UpdateMealTemplateDto>,
) -> Result<Json<meal_template::Model>, AppError> {
    MealTemplateService::update(&state.db, id, data)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    MealTemplateService::delete(&state.db, id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub async fn create_from_meal(
    State(state): State<AppState>,
    Json(data): Json<CreateFromMealDto>,
) -> Result<(StatusCode, Json<meal_template::Model>), AppError> {
    MealTemplateService::create_from_meal(&state.db, data.meal_id, data.name)
        .await
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(AppError::from)
}
