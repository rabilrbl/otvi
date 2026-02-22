use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

mod api;
mod auth_middleware;
mod db;
mod error;
mod provider_client;
mod state;

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

    let providers_dir =
        std::env::var("PROVIDERS_DIR").unwrap_or_else(|_| "providers".to_string());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "dist".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());

    // ── Database ────────────────────────────────────────────────────────────
    // Register all bundled drivers so AnyPool can inspect the URL scheme.
    sqlx::any::install_default_drivers();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data.db".to_string());
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
    // Application-level auth (OTVI user accounts).
    let user_auth_routes = Router::new()
        .route("/register", post(api::user_auth::register))
        .route("/login", post(api::user_auth::login))
        .route("/me", get(api::user_auth::me))
        .route("/logout", post(api::user_auth::logout))
        ;

    // Provider-specific auth (TV provider sessions).
    let provider_routes = Router::new()
        .route("/providers", get(api::providers::list))
        .route("/providers/:id", get(api::providers::get_info))
        .route("/providers/:id/auth/login", post(api::auth::login))
        .route("/providers/:id/auth/logout", post(api::auth::logout))
        .route("/providers/:id/auth/check", get(api::auth::check_session))
        .route("/providers/:id/channels", get(api::channels::list))
        .route(
            "/providers/:id/channels/categories",
            get(api::channels::categories),
        )
        .route(
            "/providers/:id/channels/:channel_id/stream",
            get(api::channels::stream),
        )
        .route("/proxy", get(api::proxy::proxy_stream));

    // Admin-only routes.
    let admin_routes = Router::new()
        .route("/users", get(api::admin::list_users))
        .route("/users", post(api::admin::create_user))
        .route("/users/:id", delete(api::admin::delete_user))
        .route("/users/:id/providers", put(api::admin::set_user_providers))
        .route("/settings", get(api::admin::get_settings))
        .route("/settings", put(api::admin::update_settings));

    let api_routes = Router::new()
        .nest("/auth", user_auth_routes)
        .merge(provider_routes)
        .nest("/admin", admin_routes);

    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(
            ServeDir::new(&static_dir)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(format!("{static_dir}/index.html"))),
        )
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(app_state));

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
