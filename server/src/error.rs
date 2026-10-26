use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use sea_orm::DbErr;
use serde::Serialize;

use crate::services::service_error::ServiceError;

#[derive(Serialize)]
struct ErrorBody {
    message: String,
}

pub enum AppError {
    Database(DbErr),
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred".to_string(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred".to_string(),
                )
            }
        };

        (status, Json(ErrorBody { message })).into_response()
    }
}

/// Maps a missing row to a 404 that carries the service's message, and
/// every other database error to a 500.
//
// An update whose row is deleted after the service loaded it affects no rows,
// and SeaORM reports that as `RecordNotUpdated`, so a concurrent delete
// answers 404 too.
impl From<DbErr> for AppError {
    fn from(err: DbErr) -> Self {
        match err {
            DbErr::RecordNotFound(msg) => AppError::NotFound(msg),
            DbErr::RecordNotUpdated => AppError::NotFound("Record not found".to_string()),
            other => AppError::Database(other),
        }
    }
}

impl From<ServiceError> for AppError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Validation(e) => AppError::BadRequest(e.to_string()),
            ServiceError::Database(e) => e.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_vanished_row_is_not_found_and_other_errors_stay_internal() {
        let missing = DbErr::RecordNotFound("Recipe not found".to_string());
        assert!(
            matches!(AppError::from(missing), AppError::NotFound(m) if m == "Recipe not found")
        );
        assert!(matches!(
            AppError::from(DbErr::RecordNotUpdated),
            AppError::NotFound(_)
        ));
        assert!(matches!(
            AppError::from(DbErr::Custom("boom".to_string())),
            AppError::Database(_)
        ));
    }
}
