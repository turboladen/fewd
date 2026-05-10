use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::dto::{CreatePersonDto, UpdatePersonDto};
use crate::entities::person;
use crate::error::AppError;
use crate::services::mcp_token_service::{McpTokenService, TokenError};
use crate::services::person_service::PersonService;
use crate::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<person::Model>>, AppError> {
    PersonService::get_all(&state.db)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Option<person::Model>>, AppError> {
    PersonService::get_by_id(&state.db, id)
        .await
        .map(Json)
        .map_err(AppError::from)
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
    PersonService::update(&state.db, id, data)
        .await
        .map(Json)
        .map_err(AppError::from)
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

/// Body returned from a successful `provision_mcp_token`. The plaintext
/// is shown in the UI exactly once and then discarded — it is the only
/// thing the operator needs to put into their `mcp-remote` config. The
/// fingerprint is also persisted on the row so the UI can identify
/// which token is active without re-revealing the secret.
#[derive(Serialize)]
pub struct ProvisionMcpTokenResponse {
    pub token: String,
    pub fingerprint: String,
}

/// Empty request body for [`provision_mcp_token`]. The body has no
/// fields; its only purpose is to require `Content-Type:
/// application/json` on the request, which bumps it out of CORS-simple
/// territory and forces a preflight. Without this, a malicious page
/// could issue a cross-site `<form method="POST">` to this route and
/// silently rotate a victim's MCP token (DoS — the attacker can't
/// read the response, but the victim's existing client config stops
/// working). DELETE is non-simple by method and already preflighted,
/// so the revoke endpoint doesn't need this guard.
#[derive(Deserialize)]
pub struct ProvisionMcpTokenRequest {}

/// `POST /api/people/:id/mcp-token` — issue (or rotate) an MCP bearer
/// token for the named person. Replaces any prior token. The plaintext
/// is returned exactly once.
pub async fn provision_mcp_token(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(_): Json<ProvisionMcpTokenRequest>,
) -> Result<Json<ProvisionMcpTokenResponse>, AppError> {
    match McpTokenService::provision(&state.db, &id).await {
        Ok(issued) => Ok(Json(ProvisionMcpTokenResponse {
            token: issued.plaintext,
            fingerprint: issued.fingerprint,
        })),
        Err(TokenError::NotFound) => Err(AppError::NotFound(format!("person '{id}' not found"))),
        Err(TokenError::Inactive) => Err(AppError::BadRequest(format!(
            "person '{id}' is inactive — reactivate them via the Family tab before provisioning a token"
        ))),
        Err(TokenError::Database(err)) => Err(AppError::Database(err)),
        Err(TokenError::Hashing(err)) => Err(AppError::Internal(format!(
            "argon2 hashing failed during token provision: {err}"
        ))),
    }
}

/// `DELETE /api/people/:id/mcp-token` — null out the hash + fingerprint
/// columns so any previously-issued plaintext stops authenticating.
pub async fn revoke_mcp_token(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    match McpTokenService::revoke(&state.db, &id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(TokenError::NotFound) => Err(AppError::NotFound(format!("person '{id}' not found"))),
        Err(TokenError::Database(err)) => Err(AppError::Database(err)),
        // `revoke` deliberately allows operating on inactive rows
        // (idempotent no-op) and doesn't call argon2, so neither arm
        // fires today. Keeping them explicit (rather than `_ => …`)
        // so a future change that triggers either path gets routed
        // correctly instead of being silently swallowed.
        Err(TokenError::Inactive) => Err(AppError::Internal(
            "unexpected Inactive error from revoke".to_string(),
        )),
        Err(TokenError::Hashing(err)) => Err(AppError::Internal(format!(
            "unexpected hashing error from revoke: {err}"
        ))),
    }
}
