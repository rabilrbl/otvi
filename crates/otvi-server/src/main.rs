use std::net::SocketAddr;
use std::sync::Arc;

use tower_http::services::{ServeDir, ServeFile};

use otvi_server::auth_middleware;
use otvi_server::db;
use otvi_server::state;
use otvi_server::state::RateLimitConfig;
use otvi_server::watcher;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (silently ignored when absent).
    dotenvy::dotenv().ok();

    // ── Structured logging ──────────────────────────────────────────────────
    // Set LOG_FORMAT=json for machine-readable output (e.g. Loki, Datadog).
    // Defaults to human-readable text for local development.
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "text".to_string());

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "otvi_server=info".into());

    match log_format.to_lowercase().as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(env_filter)
                .with_current_span(false)
                .with_span_list(false)
                .init();
        }
        _ => {
            tracing_subscriber::fmt().with_env_filter(env_filter).init();
        }
    }

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
    let provider_count = app_state.providers_rw.read().map(|g| g.len()).unwrap_or(0);
    tracing::info!("Loaded {provider_count} provider(s)");

    // ── Rate limiting ───────────────────────────────────────────────────────
    let rate_limit = RateLimitConfig::from_env();

    // ── Routes ──────────────────────────────────────────────────────────────
    let state = Arc::new(app_state);

    // ── Hot-reload watcher ──────────────────────────────────────────────────
    // Watches the providers directory for YAML changes and reloads the
    // provider map in-place without restarting the server.
    watcher::spawn(state.clone(), providers_dir.clone());
    tracing::info!(dir = %providers_dir, "Provider hot-reload enabled");

    let app = otvi_server::build_router(state, rate_limit).fallback_service(
        ServeDir::new(&static_dir)
            .append_index_html_on_directories(true)
            .fallback(ServeFile::new(format!("{static_dir}/index.html"))),
    );

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
