use std::sync::Arc;

use tower_http::services::{ServeDir, ServeFile};

use otvi_server::auth_middleware;
use otvi_server::db;
use otvi_server::state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (silently ignored when absent).
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "otvi_server=info".into()),
        )
        .init();

    let providers_dir = std::env::var("PROVIDERS_DIR").unwrap_or_else(|_| "providers".to_string());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "dist".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());

    // ── Database ────────────────────────────────────────────────────────────
    // Register all bundled drivers so AnyPool can inspect the URL scheme.
    sqlx::any::install_default_drivers();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data.db".to_string());
    tracing::info!("Connecting to database: {database_url}");
    let db = db::init(&database_url).await?;

    // ── JWT secret ──────────────────────────────────────────────────────────
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!(
            "JWT_SECRET not set – using random secret (tokens valid only until restart)"
        );
        uuid::Uuid::new_v4().to_string()
    });
    let jwt_keys = auth_middleware::JwtKeys::new(jwt_secret.as_bytes());

    // ── Providers ───────────────────────────────────────────────────────────
    let app_state = state::AppState::load_providers(&providers_dir, db, jwt_keys)?;
    tracing::info!(
        "Loaded {} provider(s): {:?}",
        app_state.providers.len(),
        app_state.providers.keys().collect::<Vec<_>>()
    );

    // ── Routes ──────────────────────────────────────────────────────────────
    let state = Arc::new(app_state);
    let app = otvi_server::build_router(state).fallback_service(
        ServeDir::new(&static_dir)
            .append_index_html_on_directories(true)
            .fallback(ServeFile::new(format!("{static_dir}/index.html"))),
    );

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
