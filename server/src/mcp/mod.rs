//! MCP (Model Context Protocol) server — exposes fewd's domain to AI clients.
//!
//! Mounted into the main Axum router at `/mcp` via [`router`]. The transport
//! is Streamable HTTP (single endpoint, POST for JSON-RPC, GET for SSE).
//! Access is gated by a per-person opaque-token auth layer: the client
//! sends `Authorization: Bearer <token>`, the middleware
//! constant-time-verifies the token's argon2id hash against active
//! [`Person`](crate::entities::person::Model) rows, and the resolved row
//! rides the request into tool handlers as an [`AuthenticatedPerson`]
//! extension. Tokens are issued from the web Settings UI (one per
//! person; rotation is just re-provisioning).
//!
//! # Threat model
//!
//! Two facts every contributor extending this surface should be holding
//! explicitly. The README has the user-facing version of the same; this is
//! the contributor-facing version, kept here so it surfaces during code
//! review of any change to `/mcp`.
//!
//! 1. **`/mcp` was historically LAN-only by necessity; tokens make that a
//!    defense-in-depth recommendation, not a hard requirement.** Per-person
//!    opaque tokens (256 bits, argon2id-hashed at rest, constant-time
//!    verified) can stand on their own as authentication. Still: the
//!    server binds `0.0.0.0`, has no rate-limiting, and exposes no
//!    revocation telemetry, so internet-facing exposure remains unwise
//!    without a fronting proxy. Treat LAN scoping as a sensible default,
//!    not a security boundary the auth layer requires.
//!
//! 2. **Any authenticated family member can do anything.** There is no
//!    per-user authorization today. [`AuthenticatedPerson`] is plumbed
//!    through the request extensions and made visible to tool handlers,
//!    but only `whoami` reads it. When per-user checks become necessary
//!    ("only the caller can revoke their own meals", "only adults can
//!    set drink preferences"), the canonical extraction site is
//!    `handler::authenticated_person` — go through that helper at
//!    every check, never reach into `context.extensions` directly. That
//!    convention makes any authorization decision findable via
//!    `git grep authenticated_person`. The integration test at
//!    `server/tests/mcp_auth_plumbing_test.rs` pins that the helper
//!    receives the right identity end-to-end so a middleware refactor
//!    can't silently break the read path.

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
use crate::services::mcp_token_service::McpTokenService;

use self::handler::FewdMcp;

mod handler;
mod lookups;
pub(crate) mod schemas;

/// A family member resolved from the `Authorization: Bearer <token>` header.
/// Inserted into the HTTP request extensions by the auth middleware; tool
/// handlers read it via `handler::authenticated_person` — that's the
/// canonical extraction site, and the only place that should reach into
/// the request extensions to pull this out. Going through the helper
/// keeps any future per-user authorization check findable via `git grep
/// authenticated_person` instead of scattered across tool bodies.
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

    let default_config = StreamableHttpServerConfig::default();
    let allowed_hosts = merge_allowed_hosts(
        default_config.allowed_hosts.clone(),
        std::env::var("MCP_ALLOWED_HOSTS").ok().as_deref(),
    );
    let config = default_config.with_allowed_hosts(allowed_hosts);

    let streamable = StreamableHttpService::new(
        move || Ok(FewdMcp::new(handler_db.clone())),
        Arc::new(session_manager),
        config,
    );

    Router::new()
        .fallback_service(streamable)
        .layer(middleware::from_fn_with_state(db, require_mcp_token))
}

/// Build the MCP host allowlist by appending operator-supplied hostnames from
/// `MCP_ALLOWED_HOSTS` to whatever rmcp ships as its localhost defaults.
///
/// rmcp's `StreamableHttpService` rejects requests whose `Host` header isn't
/// on this list (DNS-rebinding defense, on by default since rmcp 1.4). Its
/// own defaults — currently `localhost`, `127.0.0.1`, `::1`, but read live
/// from `StreamableHttpServerConfig::default().allowed_hosts` so a future
/// upgrade can't silently drift this layer out of sync — silently 403 any
/// LAN hostname like `dietpi.local`, so a deploy bound to `0.0.0.0` becomes
/// unreachable from other machines until the operator opts in.
///
/// Env-var format: comma-separated, e.g. `MCP_ALLOWED_HOSTS=dietpi.local,fewd.lan:3000`.
/// Per rmcp matching rules, an entry without a port matches any port; an
/// entry with a port matches only that port.
fn merge_allowed_hosts(defaults: Vec<String>, env_value: Option<&str>) -> Vec<String> {
    let mut hosts = defaults;

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

/// Resolve `Authorization: Bearer <opaque-token>` to an active `Person`.
///
/// Header parsing (scheme case, whitespace handling, malformed headers) is
/// delegated to `axum_extra`'s `TypedHeader<Authorization<Bearer>>`
/// extractor, which follows RFC 7235. Token verification (constant-time
/// argon2id) lives in [`McpTokenService::verify`]. The middleware never
/// logs the presented token — even on lookup failure — to avoid leaking
/// it through `journalctl` or shipped log files.
async fn require_mcp_token(
    State(db): State<DatabaseConnection>,
    bearer: Option<TypedHeader<Authorization<Bearer>>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(TypedHeader(auth)) = bearer else {
        return unauthorized("missing Authorization: Bearer <mcp-token>");
    };

    let token = auth.token().trim();
    if token.is_empty() {
        return unauthorized("missing Authorization: Bearer <mcp-token>");
    }

    match McpTokenService::verify(&db, token).await {
        Ok(Some(person)) => {
            req.extensions_mut().insert(AuthenticatedPerson(person));
            next.run(req).await
        }
        Ok(None) => unauthorized("invalid or revoked token"),
        Err(err) => {
            tracing::error!(?err, "MCP auth: token lookup failed");
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
    use rmcp::transport::streamable_http_server::tower::StreamableHttpServerConfig;

    /// Stand-in for whatever rmcp's defaults happen to be; the helper is
    /// agnostic to their content so tests pin a stable list. The separate
    /// `merge_preserves_rmcp_defaults_verbatim` test guards the wiring at
    /// the call site against future drift in rmcp itself.
    fn fake_defaults() -> Vec<String> {
        vec!["alpha".into(), "beta".into()]
    }

    #[test]
    fn unset_env_keeps_defaults_unchanged() {
        assert_eq!(merge_allowed_hosts(fake_defaults(), None), fake_defaults());
    }

    #[test]
    fn empty_env_keeps_defaults_unchanged() {
        assert_eq!(
            merge_allowed_hosts(fake_defaults(), Some("")),
            fake_defaults()
        );
    }

    #[test]
    fn whitespace_only_env_keeps_defaults_unchanged() {
        assert_eq!(
            merge_allowed_hosts(fake_defaults(), Some("   ,  , ")),
            fake_defaults(),
        );
    }

    #[test]
    fn single_host_appends_to_defaults() {
        let mut expected = fake_defaults();
        expected.push("dietpi.local".into());
        assert_eq!(
            merge_allowed_hosts(fake_defaults(), Some("dietpi.local")),
            expected,
        );
    }

    #[test]
    fn comma_separated_hosts_all_appended_in_order() {
        let mut expected = fake_defaults();
        expected.push("dietpi.local".into());
        expected.push("fewd.lan:3000".into());
        expected.push("192.168.1.42".into());
        assert_eq!(
            merge_allowed_hosts(
                fake_defaults(),
                Some("dietpi.local,fewd.lan:3000,192.168.1.42"),
            ),
            expected,
        );
    }

    #[test]
    fn entries_are_trimmed_and_blanks_skipped() {
        let mut expected = fake_defaults();
        expected.push("dietpi.local".into());
        expected.push("fewd.lan".into());
        assert_eq!(
            merge_allowed_hosts(fake_defaults(), Some("  dietpi.local , , fewd.lan  ,  ")),
            expected,
        );
    }

    /// Confirms the call site reads rmcp's defaults instead of hardcoding
    /// them — the original review concern for this helper. If rmcp ever
    /// changes (or empties) its default allowlist, this test surfaces it
    /// instead of letting our wiring drift silently.
    #[test]
    fn merge_preserves_rmcp_defaults_verbatim() {
        let rmcp_defaults = StreamableHttpServerConfig::default().allowed_hosts;
        assert_eq!(
            merge_allowed_hosts(rmcp_defaults.clone(), None),
            rmcp_defaults,
        );
    }

    // ─── Auth middleware (fewd-2y6.6) ───────────────────────────────
    //
    // The end-to-end smoke test plan exercises these cells manually
    // against a release binary. Pinning them as integration tests
    // here gives us a regression guard against:
    //   - flipping the auth scheme back to name-bearer
    //   - dropping the empty-token / missing-header path
    //   - accidentally accepting a revoked token
    // Each test mounts only the auth middleware on a stub handler so
    // we don't need rmcp's StreamableHttpService spun up.

    use crate::dto::CreatePersonDto;
    use crate::services::mcp_token_service::McpTokenService;
    use crate::services::person_service::PersonService;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware;
    use axum::routing::any;
    use axum::Router;
    use migration::MigratorTrait;
    use sea_orm::Database;
    use tower::ServiceExt;

    /// Builds a Router whose only route returns 200 OK if the request
    /// reached it. Wraps that route with the auth middleware so we can
    /// observe whether the middleware short-circuits or lets the call
    /// through.
    async fn build_test_router() -> (Router, sea_orm::DatabaseConnection) {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite connects");
        migration::Migrator::up(&db, None)
            .await
            .expect("migrations run on empty DB");
        let app = Router::new()
            .route("/", any(|| async { StatusCode::OK }))
            .layer(middleware::from_fn_with_state(
                db.clone(),
                super::require_mcp_token,
            ))
            .with_state(db.clone());
        (app, db)
    }

    async fn seed_alice(db: &sea_orm::DatabaseConnection) -> String {
        let alice = PersonService::create(
            db,
            CreatePersonDto {
                name: "Alice".into(),
                birthdate: "1990-01-01".into(),
                dietary_goals: None,
                dislikes: vec![],
                favorites: vec![],
                notes: None,
                drink_preferences: None,
                drink_dislikes: None,
            },
        )
        .await
        .expect("create alice");
        alice.id
    }

    async fn call(app: &Router, header: Option<&str>) -> StatusCode {
        let mut req = Request::builder().uri("/").method("GET");
        if let Some(value) = header {
            req = req.header("authorization", value);
        }
        app.clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .expect("router responds")
            .status()
    }

    #[tokio::test]
    async fn auth_rejects_request_with_no_authorization_header() {
        let (app, _db) = build_test_router().await;
        assert_eq!(call(&app, None).await, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_rejects_empty_bearer_token() {
        let (app, _db) = build_test_router().await;
        assert_eq!(
            call(&app, Some("Bearer ")).await,
            StatusCode::UNAUTHORIZED,
            "whitespace-only bearer must not pass"
        );
    }

    #[tokio::test]
    async fn auth_rejects_garbage_token() {
        let (app, db) = build_test_router().await;
        // Seed Alice but provision NO token — confirms a name-shaped
        // bearer no longer authenticates (regression guard against a
        // revert to name-bearer).
        let _id = seed_alice(&db).await;
        assert_eq!(
            call(&app, Some("Bearer Alice")).await,
            StatusCode::UNAUTHORIZED,
            "name-shaped bearer must not pass under token auth"
        );
    }

    #[tokio::test]
    async fn auth_accepts_valid_provisioned_token() {
        let (app, db) = build_test_router().await;
        let id = seed_alice(&db).await;
        let issued = McpTokenService::provision(&db, &id).await.unwrap();
        assert_eq!(
            call(&app, Some(&format!("Bearer {}", issued.plaintext))).await,
            StatusCode::OK,
            "valid token must reach the handler"
        );
    }

    #[tokio::test]
    async fn auth_rejects_revoked_token() {
        let (app, db) = build_test_router().await;
        let id = seed_alice(&db).await;
        let issued = McpTokenService::provision(&db, &id).await.unwrap();
        // Confirm the token works first.
        assert_eq!(
            call(&app, Some(&format!("Bearer {}", issued.plaintext))).await,
            StatusCode::OK
        );
        // Revoke; same plaintext now must 401.
        McpTokenService::revoke(&db, &id).await.unwrap();
        assert_eq!(
            call(&app, Some(&format!("Bearer {}", issued.plaintext))).await,
            StatusCode::UNAUTHORIZED,
            "revoked token must not authenticate"
        );
    }

    #[tokio::test]
    async fn auth_rejects_token_after_rotation() {
        let (app, db) = build_test_router().await;
        let id = seed_alice(&db).await;
        let first = McpTokenService::provision(&db, &id).await.unwrap();
        let _second = McpTokenService::provision(&db, &id).await.unwrap();
        // The original plaintext is gone after rotation.
        assert_eq!(
            call(&app, Some(&format!("Bearer {}", first.plaintext))).await,
            StatusCode::UNAUTHORIZED,
            "rotated-out token must stop authenticating"
        );
    }
}
