//! MCP (Model Context Protocol) server — exposes fewd's domain to AI clients.
//!
//! Mounted into the main Axum router at `/mcp` via [`router`]. The transport
//! is Streamable HTTP (single endpoint, POST for JSON-RPC, GET for SSE).
//! Access is gated by a light "family-member bearer" auth layer: the client
//! sends `Authorization: Bearer <name>`, the middleware resolves that to an
//! active [`Person`](crate::entities::person::Model) row (case-insensitive),
//! and the resolved row rides the request into tool handlers as an
//! [`AuthenticatedPerson`] extension.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use axum_extra::headers::authorization::Bearer;
use axum_extra::headers::Authorization;
use axum_extra::typed_header::TypedHeader;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::tower::{
    StreamableHttpServerConfig, StreamableHttpService,
};
use sea_orm::DatabaseConnection;
use serde_json::json;

use crate::entities::person;
use crate::services::person_service::PersonService;

use self::handler::FewdMcp;

mod handler;
mod lookups;
mod schemas;

/// A family member resolved from the `Authorization: Bearer <name>` header.
/// Inserted into the HTTP request extensions by the auth middleware; tool
/// handlers read it via the rmcp `RequestContext::extensions`.
#[derive(Clone, Debug)]
pub struct AuthenticatedPerson(pub person::Model);

/// Build the Axum router for the MCP endpoint.
pub fn router(db: DatabaseConnection) -> Router {
    let handler_db = db.clone();

    // Extend rmcp's idle-session reaper from 5 minutes to 7 days.
    //
    // The default 5-minute timeout reaps session workers whenever a Claude
    // Desktop chat sits idle — even overnight — so the next tool call
    // lands on a stale session-id, the server correctly returns 404 per
    // the MCP spec, and `mcp-remote` hangs for ~4 minutes before
    // surfacing the failure to the user.
    //
    // We don't want to disable the reaper outright because it still
    // catches phantom sessions when a client crashes without sending
    // DELETE — those would otherwise accumulate in memory until the
    // server restarts. Seven days is long enough that normal
    // walk-away-and-come-back usage never hits it, short enough that
    // crashed-client sessions don't pile up indefinitely.
    let mut session_manager = LocalSessionManager::default();
    session_manager.session_config.keep_alive = Some(Duration::from_secs(60 * 60 * 24 * 7));

    let streamable = StreamableHttpService::new(
        move || Ok(FewdMcp::new(handler_db.clone())),
        Arc::new(session_manager),
        StreamableHttpServerConfig::default(),
    );

    Router::new()
        .fallback_service(streamable)
        .layer(middleware::from_fn_with_state(db, require_family_bearer))
}

/// Resolve `Authorization: Bearer <name>` to an active `Person`.
///
/// Header parsing (scheme case, whitespace handling, malformed headers) is
/// delegated to `axum_extra`'s `TypedHeader<Authorization<Bearer>>`
/// extractor, which follows RFC 7235. Application-level concerns — empty
/// tokens, unknown family members, DB errors — are handled below.
async fn require_family_bearer(
    State(db): State<DatabaseConnection>,
    bearer: Option<TypedHeader<Authorization<Bearer>>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(TypedHeader(auth)) = bearer else {
        return unauthorized("missing Authorization: Bearer <family-member-name>");
    };

    let name = auth.token().trim();
    if name.is_empty() {
        return unauthorized("missing Authorization: Bearer <family-member-name>");
    }

    match PersonService::find_active_by_name(&db, name).await {
        Ok(Some(person)) => {
            req.extensions_mut().insert(AuthenticatedPerson(person));
            next.run(req).await
        }
        Ok(None) => unauthorized("unknown family member"),
        Err(err) => {
            tracing::error!(?err, "MCP auth: person lookup failed");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "auth lookup failed")
        }
    }
}

fn unauthorized(message: &str) -> Response {
    error_response(StatusCode::UNAUTHORIZED, message)
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        json!({ "error": message }).to_string(),
    )
        .into_response()
}
