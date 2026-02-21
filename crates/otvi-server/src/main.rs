use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

mod api;
mod error;
mod provider_client;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let app_state = state::AppState::load_providers(&providers_dir)?;
    tracing::info!(
        "Loaded {} provider(s): {:?}",
        app_state.providers.len(),
        app_state.providers.keys().collect::<Vec<_>>()
    );

    let api_routes = Router::new()
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
