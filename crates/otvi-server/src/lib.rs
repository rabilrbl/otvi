pub mod api;
pub mod auth_middleware;
pub mod db;
pub mod error;
pub mod provider_client;
pub mod state;

use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post, put};
use tower_http::cors::CorsLayer;

/// Build the API router with the given application state.
///
/// This is extracted from `main()` so that integration tests can construct
/// the full router without starting a TCP listener.
pub fn build_router(state: Arc<state::AppState>) -> Router {
    let user_auth_routes = Router::new()
        .route("/register", post(api::user_auth::register))
        .route("/login", post(api::user_auth::login))
        .route("/me", get(api::user_auth::me))
        .route("/logout", post(api::user_auth::logout))
        .route("/change-password", post(api::user_auth::change_password));

    let provider_routes = Router::new()
        .route("/providers", get(api::providers::list))
        .route("/providers/{id}", get(api::providers::get_info))
        .route("/providers/{id}/auth/login", post(api::auth::login))
        .route("/providers/{id}/auth/logout", post(api::auth::logout))
        .route("/providers/{id}/auth/check", get(api::auth::check_session))
        .route("/providers/{id}/channels", get(api::channels::list))
        .route(
            "/providers/{id}/channels/categories",
            get(api::channels::categories),
        )
        .route(
            "/providers/{id}/channels/{channel_id}/stream",
            get(api::channels::stream),
        )
        .route("/proxy", get(api::proxy::proxy_stream));

    let admin_routes = Router::new()
        .route("/users", get(api::admin::list_users))
        .route("/users", post(api::admin::create_user))
        .route("/users/{id}", delete(api::admin::delete_user))
        .route("/users/{id}/providers", put(api::admin::set_user_providers))
        .route("/users/{id}/password", put(api::admin::reset_user_password))
        .route("/settings", get(api::admin::get_settings))
        .route("/settings", put(api::admin::update_settings));

    let api_routes = Router::new()
        .nest("/auth", user_auth_routes)
        .merge(provider_routes)
        .nest("/admin", admin_routes);

    Router::new()
        .nest("/api", api_routes)
        .layer(CorsLayer::permissive())
        .with_state(state)
}
