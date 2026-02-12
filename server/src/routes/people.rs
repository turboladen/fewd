use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use crate::dto::{CreatePersonDto, UpdatePersonDto};
use crate::entities::person;
use crate::error::AppError;
use crate::services::person_service::PersonService;
use crate::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<person::Model>>, AppError> {
    PersonService::get_all(&state.db).await.map(Json).map_err(AppError::from)
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Option<person::Model>>, AppError> {
    PersonService::get_by_id(&state.db, id).await.map(Json).map_err(AppError::from)
}

pub async fn create(
    State(state): State<AppState>,
    Json(data): Json<CreatePersonDto>,
) -> Result<(StatusCode, Json<person::Model>), AppError> {
    PersonService::create(&state.db, data)
        .await
        .map(|p| (StatusCode::CREATED, Json(p)))
        .map_err(AppError::from)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(data): Json<UpdatePersonDto>,
) -> Result<Json<person::Model>, AppError> {
    PersonService::update(&state.db, id, data).await.map(Json).map_err(AppError::from)
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    PersonService::delete(&state.db, id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}
