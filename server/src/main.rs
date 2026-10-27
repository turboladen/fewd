use std::net::SocketAddr;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::Router;
use fewd_lib::{db, mcp, routes, AppState};
use rust_embed::Embed;
use tower_http::trace::TraceLayer;

#[derive(Embed)]
#[folder = "../dist"]
struct Assets;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/fewd.db".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");

    tracing::info!("Initializing database at {}", db_path);
    let db = db::init(&db_path)
        .await
        .expect("Failed to initialize database");

    let state = AppState { db };

    // The server sends no CORS headers, on purpose. The SPA, and the Vite proxy
    // in dev, call `/api` same-origin, so no browser caller needs a CORS grant.
    // Without one, a cross-origin page can neither read an `/api` response nor
    // pass the preflight that a JSON or `X-Requested-With` POST forces. That is
    // half of the CSRF invariant; `routes::csrf` enforces the other half.
    // Adding a CORS layer weakens it.
    let api_and_spa = Router::new()
        .nest("/api", routes::api_routes())
        .fallback(serve_spa);

    let app = Router::new()
        .nest_service("/mcp", mcp::router(state.db.clone()))
        .merge(api_and_spa)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind address");
    let bound_port = listener
        .local_addr()
        .expect("listener has no local address")
        .port();
    tracing::info!("Server running on http://localhost:{}", bound_port);
    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}

async fn serve_spa(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try the exact path first, fall back to index.html for SPA client-side routing
    let (asset, mime_path) = if path.is_empty() {
        (Assets::get("index.html"), "index.html")
    } else {
        match Assets::get(path) {
            Some(file) => (Some(file), path),
            None => (Assets::get("index.html"), "index.html"),
        }
    };

    match asset {
        Some(content) => {
            let mime = mime_guess::from_path(mime_path)
                .first_or_octet_stream()
                .as_ref()
                .to_string();
            ([(header::CONTENT_TYPE, mime)], content.data).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
