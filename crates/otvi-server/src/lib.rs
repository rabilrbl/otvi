pub mod api;
pub mod auth_middleware;
pub mod db;
pub mod error;
pub mod provider_client;
pub mod state;
pub mod watcher;

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use tower_http::cors::CorsLayer;

use state::AppState;

/// Build the API router with the given application state.
///
/// This is extracted from `main()` so that integration tests can construct
/// the full router without starting a TCP listener.
pub fn build_router(state: Arc<AppState>) -> axum::Router {
    let user_auth_routes = axum::Router::new()
        .route("/register", post(api::user_auth::register))
        .route("/login", post(api::user_auth::login))
        .route("/me", get(api::user_auth::me))
        .route("/logout", post(api::user_auth::logout))
        .route("/change-password", post(api::user_auth::change_password));

    let provider_routes = axum::Router::new()
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

    let admin_routes = axum::Router::new()
        .route("/users", get(api::admin::list_users))
        .route("/users", post(api::admin::create_user))
        .route("/users/{id}", delete(api::admin::delete_user))
        .route("/users/{id}/providers", put(api::admin::set_user_providers))
        .route("/users/{id}/password", put(api::admin::reset_user_password))
        .route("/settings", get(api::admin::get_settings))
        .route("/settings", put(api::admin::update_settings));

    let api_routes = axum::Router::new()
        .nest("/auth", user_auth_routes)
        .merge(provider_routes)
        .nest("/admin", admin_routes);

    // ── Build CORS layer from environment ────────────────────────────────
    let cors = build_cors_layer();

    axum::Router::new()
        .nest("/api", api_routes)
        // ── Health / readiness checks ────────────────────────────────────
        .route("/healthz", get(health_check))
        .route("/readyz", get(ready_check))
        // ── Provider YAML JSON Schema ────────────────────────────────────
        .route("/api/schema/provider", get(provider_schema))
        .layer(cors)
        .with_state(state)
}

// ── CORS ──────────────────────────────────────────────────────────────────

/// Build a `CorsLayer` that respects the `CORS_ORIGINS` environment variable.
///
/// | `CORS_ORIGINS` value | Behaviour                                    |
/// |----------------------|----------------------------------------------|
/// | unset / `"*"`        | Permissive (allow all) – suitable for dev    |
/// | `"http://a,https://b"` | Restricted to the listed origins           |
///
/// In production, set `CORS_ORIGINS` to the exact frontend origin, e.g.:
/// ```text
/// CORS_ORIGINS=https://tv.example.com
/// ```
fn build_cors_layer() -> CorsLayer {
    use axum::http::HeaderValue;
    use tower_http::cors::AllowOrigin;

    match std::env::var("CORS_ORIGINS") {
        Ok(origins) if origins != "*" && !origins.is_empty() => {
            let allowed: Vec<HeaderValue> = origins
                .split(',')
                .filter_map(|o| o.trim().parse::<HeaderValue>().ok())
                .collect();

            if allowed.is_empty() {
                tracing::warn!(
                    "CORS_ORIGINS set but no valid origins parsed – falling back to permissive"
                );
                CorsLayer::permissive()
            } else {
                tracing::info!(origins = %origins, "CORS restricted to configured origins");
                CorsLayer::new()
                    .allow_origin(AllowOrigin::list(allowed))
                    .allow_methods([
                        axum::http::Method::GET,
                        axum::http::Method::POST,
                        axum::http::Method::PUT,
                        axum::http::Method::DELETE,
                        axum::http::Method::OPTIONS,
                    ])
                    .allow_headers([
                        axum::http::header::AUTHORIZATION,
                        axum::http::header::CONTENT_TYPE,
                        axum::http::header::ACCEPT,
                    ])
                    .allow_credentials(false)
            }
        }
        _ => {
            tracing::warn!(
                "CORS_ORIGINS not set – using permissive CORS policy (not suitable for production)"
            );
            CorsLayer::permissive()
        }
    }
}

// ── Health checks ─────────────────────────────────────────────────────────

/// `GET /healthz` – liveness probe.
///
/// Returns `200 OK` immediately.  Orchestrators (Docker, Kubernetes) use this
/// to determine whether the process is alive.  No DB check is performed so
/// this responds even when the database is temporarily unavailable.
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

/// `GET /readyz` – readiness probe.
///
/// Returns `200 OK` when the database is reachable, `503 Service Unavailable`
/// otherwise.  Orchestrators use this to decide whether to route traffic here.
async fn ready_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::user_count(&state.db).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ready" })),
        ),
        Err(e) => {
            tracing::error!("Readiness check failed: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "status": "unavailable", "error": e.to_string() })),
            )
        }
    }
}

// ── Provider JSON Schema ──────────────────────────────────────────────────

/// `GET /api/schema/provider` – return the JSON Schema for provider YAML files.
///
/// The schema is generated at compile time from the `ProviderConfig` Rust
/// types via `schemars`.  Operators can paste this URL into VS Code's
/// `yaml.schemas` setting to get auto-complete and inline validation while
/// editing provider configuration files.
///
/// # VS Code setup
///
/// ```jsonc
/// // .vscode/settings.json
/// {
///   "yaml.schemas": {
///     "http://localhost:3000/api/schema/provider": "providers/*.yaml"
///   }
/// }
/// ```
async fn provider_schema() -> impl IntoResponse {
    // Re-derive the schema from ProviderConfig at request time.
    // This is a cheap operation (~microseconds) and avoids storing a
    // global static.
    let schema = schemars::schema_for!(otvi_core::config::ProviderConfig);
    (
        StatusCode::OK,
        [("Content-Type", "application/schema+json")],
        Json(schema),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_middleware::JwtKeys;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    #[allow(unused_imports)]
    use schemars::JsonSchema as _;
    use tower::ServiceExt;

    async fn test_db() -> (crate::db::Db, tempfile::TempDir) {
        sqlx::any::install_default_drivers();
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}", db_path.display());
        let db = crate::db::init(&url).await.expect("test db init");
        (db, dir)
    }

    fn test_keys() -> JwtKeys {
        JwtKeys::new(b"test-secret-lib")
    }

    async fn build_test_app() -> (axum::Router, tempfile::TempDir) {
        let (db, dir) = test_db().await;
        let state = Arc::new(
            crate::state::AppState::load_providers("nonexistent_dir_for_test", db, test_keys())
                .unwrap(),
        );
        (build_router(state), dir)
    }

    #[tokio::test]
    async fn health_check_returns_200() {
        let (app, _dir) = build_test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ready_check_returns_200_with_good_db() {
        let (app, _dir) = build_test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn provider_schema_returns_json_schema() {
        let (app, _dir) = build_test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/schema/provider")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.contains("json"),
            "expected JSON content-type, got: {content_type}"
        );
    }

    #[tokio::test]
    async fn build_cors_permissive_when_not_set() {
        // Verify the router builds without panicking when CORS_ORIGINS is not set.
        // SAFETY: single-threaded test environment; no other threads read this var.
        unsafe { std::env::remove_var("CORS_ORIGINS") };
        let (app, _dir) = build_test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .method("OPTIONS")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // OPTIONS on a non-preflight-registered route still returns something
        // (not a 500), which means the middleware didn't panic.
        assert_ne!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
