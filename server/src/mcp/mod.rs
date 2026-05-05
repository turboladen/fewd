//! MCP (Model Context Protocol) server — exposes fewd's domain to AI clients.
//!
//! Mounted into the main Axum router at `/mcp` via [`router`]. The transport
//! is Streamable HTTP (single endpoint, POST for JSON-RPC, GET for SSE).
//! Access is gated by a light "family-member bearer" auth layer: the client
//! sends `Authorization: Bearer <name>`, the middleware resolves that to an
//! active [`Person`](crate::entities::person::Model) row (case-insensitive),
//! and the resolved row rides the request into tool handlers as an
//! [`AuthenticatedPerson`] extension.
//!
//! # Threat model
//!
//! Three facts every contributor extending this surface should be holding
//! explicitly. The README has the user-facing version of the same; this is
//! the contributor-facing version, kept here so it surfaces during code
//! review of any change to `/mcp`.
//!
//! 1. **`/mcp` is LAN-only by design.** The server binds `0.0.0.0` but the
//!    security model assumes only people on the operator's LAN can reach
//!    the port. Do not add features that assume internet exposure
//!    (rate-limited public APIs, shareable tokens) without first replacing
//!    this auth scheme. Issue `fewd-2y6.6` tracks per-person opaque
//!    tokens for that follow-up.
//!
//! 2. **The bearer "token" has no entropy.** Family member names appear in
//!    `list_people`, `get_family_overview`, shopping briefs, and meal
//!    plans — they are identifiers, not secrets. A network observer or
//!    anyone with access to a meal-plan summary learns every valid token
//!    on the server. Treat the bearer like a username pickbox, not a
//!    credential.
//!
//! 3. **Any authenticated family member can do anything.** There is no
//!    per-user authorization. [`AuthenticatedPerson`] is plumbed through
//!    the request extensions and made visible to tool handlers, but only
//!    `whoami` reads it today. `create_meal` does not check that the
//!    serving's person matches the caller; `create_recipe` does not
//!    record the author. Adding a "self-only" or role-based rule means
//!    enforcing it at every write site, not just one. Issue `fewd-2y6.8`
//!    tracks promoting `AuthenticatedPerson` to a typed extractor so the
//!    type system catches missed sites.

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

    let config = StreamableHttpServerConfig::default().with_allowed_hosts(merge_allowed_hosts(
        std::env::var("MCP_ALLOWED_HOSTS").ok().as_deref(),
    ));

    let streamable = StreamableHttpService::new(
        move || Ok(FewdMcp::new(handler_db.clone())),
        Arc::new(session_manager),
        config,
    );

    Router::new()
        .fallback_service(streamable)
        .layer(middleware::from_fn_with_state(db, require_family_bearer))
}

/// Build the MCP host allowlist by appending operator-supplied hostnames from
/// `MCP_ALLOWED_HOSTS` to rmcp's localhost defaults.
///
/// rmcp's `StreamableHttpService` rejects requests whose `Host` header isn't
/// on this list (DNS-rebinding defense, on by default since rmcp 1.4). The
/// crate's own default — `localhost`, `127.0.0.1`, `::1` — silently 403s any
/// LAN hostname like `dietpi.local`, so a deploy bound to `0.0.0.0` becomes
/// unreachable from other machines until the operator opts in.
///
/// Env-var format: comma-separated, e.g. `MCP_ALLOWED_HOSTS=dietpi.local,fewd.lan:3000`.
/// Per rmcp matching rules, an entry without a port matches any port; an
/// entry with a port matches only that port.
fn merge_allowed_hosts(env_value: Option<&str>) -> Vec<String> {
    let mut hosts: Vec<String> = vec!["localhost".into(), "127.0.0.1".into(), "::1".into()];

    if let Some(raw) = env_value {
        for entry in raw.split(',') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            hosts.push(trimmed.to_string());
        }
    }

    hosts
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

#[cfg(test)]
mod tests {
    use super::merge_allowed_hosts;

    fn defaults() -> Vec<String> {
        vec!["localhost".into(), "127.0.0.1".into(), "::1".into()]
    }

    #[test]
    fn unset_env_keeps_localhost_defaults() {
        assert_eq!(merge_allowed_hosts(None), defaults());
    }

    #[test]
    fn empty_env_keeps_localhost_defaults() {
        assert_eq!(merge_allowed_hosts(Some("")), defaults());
    }

    #[test]
    fn whitespace_only_env_keeps_localhost_defaults() {
        assert_eq!(merge_allowed_hosts(Some("   ,  , ")), defaults());
    }

    #[test]
    fn single_host_appends_to_defaults() {
        let mut expected = defaults();
        expected.push("dietpi.local".into());
        assert_eq!(merge_allowed_hosts(Some("dietpi.local")), expected);
    }

    #[test]
    fn comma_separated_hosts_all_appended_in_order() {
        let mut expected = defaults();
        expected.push("dietpi.local".into());
        expected.push("fewd.lan:3000".into());
        expected.push("192.168.1.42".into());
        assert_eq!(
            merge_allowed_hosts(Some("dietpi.local,fewd.lan:3000,192.168.1.42")),
            expected,
        );
    }

    #[test]
    fn entries_are_trimmed_and_blanks_skipped() {
        let mut expected = defaults();
        expected.push("dietpi.local".into());
        expected.push("fewd.lan".into());
        assert_eq!(
            merge_allowed_hosts(Some("  dietpi.local , , fewd.lan  ,  ")),
            expected,
        );
    }
}
