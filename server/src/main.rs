use std::net::SocketAddr;

use axum::http::{header, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::Router;
use fewd_lib::{db, routes, AppState};
use rust_embed::Embed;
use tower_http::cors::CorsLayer;
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

    let app = Router::new()
        .nest("/api", routes::api_routes())
        .fallback(serve_spa)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server running on http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind address");
    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}

async fn serve_spa(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    let asset = if path.is_empty() {
        Assets::get("index.html")
    } else {
        Assets::get(path).or_else(|| Assets::get("index.html"))
    };

    match asset {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .as_ref()
                .to_string();
            ([(header::CONTENT_TYPE, mime)], content.data).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
