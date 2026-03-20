pub mod api;
pub mod auth_middleware;
pub mod db;
pub mod embedded_frontend;
pub mod error;
pub mod provider_client;
pub mod state;
pub mod watcher;

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use tower::Layer;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_http::cors::CorsLayer;

use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

use state::{AppState, RateLimitConfig};

pub use embedded_frontend::{has_embedded_frontend, serve_embedded_frontend};

// ── OpenAPI root document ─────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    info(
        title = "OTVI API",
        version = "0.1.0",
        description = "OTVI REST API — provider management, user authentication, channel browsing and stream proxying.",
        license(name = "AGPL-3.0-only", url = "https://www.gnu.org/licenses/agpl-3.0.html"),
    ),
    paths(
        // auth
        api::user_auth::register,
        api::user_auth::login,
        api::user_auth::me,
        api::user_auth::change_password,
        api::user_auth::logout,
        // providers
        api::providers::list,
        api::providers::get_info,
        // provider auth
        api::auth::login,
        api::auth::check_session,
        api::auth::logout,
        // channels
        api::channels::list,
        api::channels::categories,
        api::channels::stream,
        // proxy
        api::proxy::proxy_stream,
        api::proxy::proxy_drm,
        // admin
        api::admin::list_users,
        api::admin::create_user,
        api::admin::delete_user,
        api::admin::set_user_providers,
        api::admin::reset_user_password,
        api::admin::get_settings,
        api::admin::update_settings,
    ),
    components(
        schemas(
            otvi_core::types::ProviderInfo,
            otvi_core::types::AuthFlowInfo,
            otvi_core::types::FieldInfo,
            otvi_core::types::LoginRequest,
            otvi_core::types::LoginResponse,
            otvi_core::types::NextStepInfo,
            otvi_core::types::Channel,
            otvi_core::types::ChannelListResponse,
            otvi_core::types::Category,
            otvi_core::types::CategoryListResponse,
            otvi_core::types::StreamInfo,
            otvi_core::types::StreamType,
            otvi_core::types::DrmInfo,
            otvi_core::types::UserRole,
            otvi_core::types::UserInfo,
            otvi_core::types::RegisterRequest,
            otvi_core::types::AppLoginRequest,
            otvi_core::types::AppLoginResponse,
            otvi_core::types::CreateUserRequest,
            otvi_core::types::UpdateUserProvidersRequest,
            otvi_core::types::ChangePasswordRequest,
            otvi_core::types::AdminResetPasswordRequest,
            otvi_core::types::ServerSettings,
        ),
    ),
    modifiers(&BearerSecurityAddon),
    tags(
        (name = "auth",      description = "OTVI user authentication (register, login, me, change-password, logout)"),
        (name = "providers", description = "Provider listing and per-provider authentication"),
        (name = "channels",  description = "Channel browsing, category listing and stream URL resolution"),
        (name = "proxy",     description = "HLS/DASH stream proxy"),
        (name = "admin",     description = "Admin-only user and server-settings management"),
    ),
)]
struct ApiDoc;

/// Adds the `bearer_token` HTTP Bearer security scheme to the OpenAPI document.
struct BearerSecurityAddon;

impl utoipa::Modify for BearerSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_token",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

/// Build the production API router with rate limiting applied.
///
/// ## Rate limiting
///
/// Two tiers are applied, both keyed by peer IP address, configured via
/// [`RateLimitConfig`] (read from environment variables by [`RateLimitConfig::from_env`]).
///
/// | Tier    | Routes | Default quota |
/// |---------|--------|---------------|
/// | Auth    | `POST /api/auth/login`, `POST /api/auth/register`, `POST /api/*/auth/login` | 10 req burst, +1 every 3 s |
/// | General | All other `/api` routes | 60 req burst, +1 every 1 s |
///
/// The server **must** be started with
/// `axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())`
/// for the peer IP to be available to the extractor.
///
/// ## Notable routes
///
/// | Path | Description |
/// |------|-------------|
/// | `GET /api/docs` | Swagger UI (redirects to `/api/docs/`) |
/// | `GET /api/docs/` | Swagger UI index |
/// | `GET /api/docs/openapi.json` | Raw OpenAPI JSON document |
pub fn build_router(state: Arc<AppState>, rate_limit: RateLimitConfig) -> axum::Router {
    if !rate_limit.enabled {
        tracing::warn!("API rate limiting is disabled; set RATE_LIMIT_ENABLED=true to enable it");
        return build_routes(
            state,
            tower::layer::util::Identity::new(),
            tower::layer::util::Identity::new(),
        );
    }

    // Auth tier: protects login / register / provider-auth against brute-force.
    let auth_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .burst_size(rate_limit.auth_burst)
            .per_second(rate_limit.auth_period_secs)
            .use_headers()
            .finish()
            .expect("invalid auth rate-limit config"),
    );

    // General tier: broad throttling for all other API routes.
    let general_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .burst_size(rate_limit.general_burst)
            .per_second(rate_limit.general_period_secs)
            .use_headers()
            .finish()
            .expect("invalid general rate-limit config"),
    );

    // Spawn background threads to evict stale entries every 60 s.
    let auth_limiter = auth_governor_conf.limiter().clone();
    spawn_governor_cleanup(Box::new(move || auth_limiter.retain_recent()));

    let general_limiter = general_governor_conf.limiter().clone();
    spawn_governor_cleanup(Box::new(move || general_limiter.retain_recent()));

    let auth_layer = GovernorLayer::new(auth_governor_conf);
    let general_layer = GovernorLayer::new(general_governor_conf);

    build_routes(state, auth_layer, general_layer)
}

/// Build the router without rate limiting, for use in integration tests.
///
/// Integration tests drive the router via `tower::ServiceExt::oneshot` with no
/// real TCP connection, so no peer `SocketAddr` is available for
/// `PeerIpKeyExtractor`. Passing `Identity` for both layer slots skips the
/// governor middleware entirely.
pub fn build_router_for_tests(state: Arc<AppState>) -> axum::Router {
    build_routes(
        state,
        tower::layer::util::Identity::new(),
        tower::layer::util::Identity::new(),
    )
}

fn build_routes<AuthL, GeneralL>(
    state: Arc<AppState>,
    auth_layer: AuthL,
    general_layer: GeneralL,
) -> axum::Router
where
    AuthL: Layer<axum::routing::Route> + Clone + Send + Sync + 'static,
    AuthL::Service: Clone
        + Send
        + Sync
        + 'static
        + tower::Service<
            axum::extract::Request,
            Response = axum::response::Response,
            Error = std::convert::Infallible,
        >,
    <AuthL::Service as tower::Service<axum::extract::Request>>::Future: Send + 'static,
    GeneralL: Layer<axum::routing::Route> + Clone + Send + Sync + 'static,
    GeneralL::Service: Clone
        + Send
        + Sync
        + 'static
        + tower::Service<
            axum::extract::Request,
            Response = axum::response::Response,
            Error = std::convert::Infallible,
        >,
    <GeneralL::Service as tower::Service<axum::extract::Request>>::Future: Send + 'static,
{
    let user_auth_routes = axum::Router::new()
        .route("/register", post(api::user_auth::register))
        .route("/login", post(api::user_auth::login))
        .route("/me", get(api::user_auth::me))
        .route("/logout", post(api::user_auth::logout))
        .route("/change-password", post(api::user_auth::change_password));

    let provider_auth_routes = axum::Router::new()
        .route("/providers/{id}/auth/login", post(api::auth::login))
        .route("/providers/{id}/auth/logout", post(api::auth::logout))
        .route("/providers/{id}/auth/check", get(api::auth::check_session))
        .layer(auth_layer.clone());

    let provider_routes = axum::Router::new()
        .route("/providers", get(api::providers::list))
        .route("/providers/{id}", get(api::providers::get_info))
        .route("/providers/{id}/channels", get(api::channels::list))
        .route(
            "/providers/{id}/channels/categories",
            get(api::channels::categories),
        )
        .route(
            "/providers/{id}/channels/{channel_id}/stream",
            get(api::channels::stream),
        )
        .route("/proxy", get(api::proxy::proxy_stream))
        .route("/proxy/drm/{token}", post(api::proxy::proxy_drm));

    let admin_routes = axum::Router::new()
        .route("/users", get(api::admin::list_users))
        .route("/users", post(api::admin::create_user))
        .route("/users/{id}", delete(api::admin::delete_user))
        .route("/users/{id}/providers", put(api::admin::set_user_providers))
        .route("/users/{id}/password", put(api::admin::reset_user_password))
        .route("/settings", get(api::admin::get_settings))
        .route("/settings", put(api::admin::update_settings));

    let auth_limited_routes = axum::Router::new()
        .nest("/auth", user_auth_routes)
        .layer(auth_layer);

    let api_routes = axum::Router::new()
        .merge(auth_limited_routes)
        .merge(provider_routes)
        .merge(provider_auth_routes)
        .nest("/admin", admin_routes)
        .layer(general_layer);

    let cors = build_cors_layer();

    let stateful = axum::Router::new()
        .nest("/api", api_routes)
        .route("/healthz", get(health_check))
        .route("/readyz", get(ready_check))
        .route("/api/schema/provider", get(provider_schema))
        .layer(cors)
        .with_state(state);

    stateful.merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", ApiDoc::openapi()))
}

// ── Rate-limit helpers ────────────────────────────────────────────────────

/// Spawn a background thread that calls `retain_recent()` on the given
/// governor limiter every 60 seconds, evicting entries that have fully
/// replenished their quota and will never be read again.
///
/// This prevents the in-memory dashmap inside governor from growing without
/// bound on servers with many distinct client IPs.
///
/// Accepts a `Box<dyn Fn() + Send>` so we never need to name any internal
/// `governor` types directly (avoiding a direct `governor` dependency).
fn spawn_governor_cleanup(cleanup: Box<dyn Fn() + Send + 'static>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(60));
            cleanup();
            tracing::debug!("Rate-limit store pruned");
        }
    });
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
        (build_router_for_tests(state), dir)
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
