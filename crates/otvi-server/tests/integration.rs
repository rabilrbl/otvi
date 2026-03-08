//! End-to-end integration tests for every otvi-server API endpoint.
//!
//! These tests spin up the full Axum router in-process (no TCP listener) with
//! an in-memory SQLite database and the httpbin test provider.  Requests are
//! sent directly through `tower::ServiceExt::oneshot`.
//!
//! Tests marked `#[ignore]` require a running httpbin instance.  Run them via:
//!
//! ```sh
//! ./scripts/integration-test.sh
//! ```
//!
//! which starts Docker compose, patches the URL, runs these tests, and cleans up.
//!
//! ## Coverage areas
//!
//! | Area | Tests |
//! |------|-------|
//! | User auth | register, login, me, logout, change-password |
//! | Password policy | min-length, uppercase, digit (register + change + admin) |
//! | must_change_password | blocked on active routes, unblocked after change |
//! | Providers | list, get, not-found |
//! | Provider auth | login, check, logout, invalid flow/step, not-found, global scope |
//! | Channels | list, categories, stream, provider not-found |
//! | Proxy | invalid URL, upstream fetch |
//! | Admin users | list, create, delete, set-providers, reset-password |
//! | Admin settings | get, update, signup-disable enforcement |
//! | Access control | regular user cannot access admin, restricted providers |
//! | Infrastructure | /healthz, /readyz, /api/schema/provider, /api/docs/ |
//! | Full flow | end-to-end happy path |

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use otvi_server::auth_middleware::JwtKeys;
use otvi_server::state::AppState;

// ── Test helpers ────────────────────────────────────────────────────────────

/// Build the app with a temporary SQLite database and the httpbin test provider.
/// Returns (Router, TempDir) — hold the TempDir to keep the database alive.
async fn build_test_app() -> (axum::Router, tempfile::TempDir) {
    sqlx::any::install_default_drivers();

    let dir = tempfile::tempdir().expect("create temp dir");
    let db_path = dir.path().join("test.db");
    let db_url = format!("sqlite://{}", db_path.display());
    let db = otvi_server::db::init(&db_url).await.unwrap();
    let jwt_keys = JwtKeys::new(b"integration-test-secret");

    // Load the test provider YAML, optionally patching the base URL.
    let mut yaml = include_str!("fixtures/httpbin-provider.yaml").to_string();
    if let Ok(url) = std::env::var("HTTPBIN_URL") {
        yaml = yaml.replace("https://httpbin.org", &url);
    }
    let provider: otvi_core::config::ProviderConfig =
        serde_yaml_ng::from_str(&yaml).expect("parse test provider YAML");

    let mut providers = std::collections::HashMap::new();
    providers.insert(provider.provider.id.clone(), provider);

    let state = Arc::new(AppState {
        providers_rw: std::sync::RwLock::new(providers),
        db,
        jwt_keys,
        http_client: reqwest::Client::builder()
            .build()
            .expect("build HTTP client"),
        proxy_ctx: std::sync::RwLock::new(std::collections::HashMap::new()),
        channel_cache: otvi_server::state::ChannelCache::new(std::time::Duration::from_secs(300)),
    });

    (otvi_server::build_router_without_rate_limit(state), dir)
}

/// Send a request and return (status, body as Value).
async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

/// Build a JSON POST request.
fn post_json(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

/// Build an authenticated JSON POST request.
fn post_json_auth(uri: &str, body: &Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

/// Build an authenticated GET request.
fn get_auth(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Build an authenticated PUT request.
fn put_json_auth(uri: &str, body: &Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

/// Build an authenticated DELETE request.
fn delete_auth(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Register the first user (becomes admin) and return the JWT token + user ID.
async fn register_admin(app: &axum::Router) -> (String, String) {
    let (status, body) = send(
        app,
        post_json(
            "/api/auth/register",
            &json!({"username": "admin", "password": "Admin-Password-1"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register admin: {body}");
    let token = body["token"].as_str().unwrap().to_string();
    let id = body["user"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["user"]["role"], "admin");
    (token, id)
}

// ── User Authentication Tests ───────────────────────────────────────────────

#[tokio::test]
async fn first_user_becomes_admin() {
    let (app, _db_dir) = build_test_app().await;
    let (_, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "first", "password": "Password123"}),
        ),
    )
    .await;
    assert_eq!(body["user"]["role"], "admin");
}

#[tokio::test]
async fn second_user_is_regular_user() {
    let (app, _db_dir) = build_test_app().await;
    register_admin(&app).await;

    let (status, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "user2", "password": "UserPass123"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["role"], "user");
}

#[tokio::test]
async fn register_empty_username_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (status, _) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "", "password": "Password123"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_duplicate_username_rejected() {
    let (app, _db_dir) = build_test_app().await;
    register_admin(&app).await;

    let (status, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "admin", "password": "AnotherPass1"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("already taken"));
}

#[tokio::test]
async fn login_success() {
    let (app, _db_dir) = build_test_app().await;
    register_admin(&app).await;

    let (status, body) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "admin", "password": "Admin-Password-1"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
    assert_eq!(body["user"]["username"], "admin");
}

#[tokio::test]
async fn login_wrong_password() {
    let (app, _db_dir) = build_test_app().await;
    register_admin(&app).await;

    let (status, _) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "admin", "password": "WrongPassword"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_nonexistent_user() {
    let (app, _db_dir) = build_test_app().await;
    register_admin(&app).await;

    let (status, _) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "ghost", "password": "Whatever1"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_returns_user_info() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(&app, get_auth("/api/auth/me", &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], "admin");
    assert_eq!(body["role"], "admin");
}

#[tokio::test]
async fn me_without_token_returns_error() {
    let (app, _db_dir) = build_test_app().await;
    register_admin(&app).await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/me")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_no_users_returns_needs_setup() {
    let (app, _db_dir) = build_test_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/me")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&app, req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["needs_setup"], true);
}

#[tokio::test]
async fn change_password_success() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/auth/change-password",
            &json!({
                "current_password": "Admin-Password-1",
                "new_password": "NewSecurePass1"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "change password: {body}");
    assert!(body["token"].is_string());

    // Login with new password works
    let (status, _) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "admin", "password": "NewSecurePass1"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn change_password_wrong_current() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(
        &app,
        post_json_auth(
            "/api/auth/change-password",
            &json!({
                "current_password": "wrong",
                "new_password": "NewPassword1"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn change_password_too_short() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(
        &app,
        post_json_auth(
            "/api/auth/change-password",
            &json!({
                "current_password": "Admin-Password-1",
                "new_password": "short"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn logout_returns_success() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(&app, post_json_auth("/api/auth/logout", &json!({}), &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
}

// ── Password policy tests ───────────────────────────────────────────────────

#[tokio::test]
async fn register_password_no_uppercase_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (status, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "user1", "password": "password123"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(body["error"].as_str().unwrap().contains("uppercase"));
}

#[tokio::test]
async fn register_password_no_digit_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (status, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "user1", "password": "PasswordOnly"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(body["error"].as_str().unwrap().contains("digit"));
}

#[tokio::test]
async fn register_password_too_short_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (status, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "user1", "password": "Sh0rt"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(body["error"].as_str().unwrap().contains("8 characters"));
}

#[tokio::test]
async fn change_password_no_uppercase_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/auth/change-password",
            &json!({
                "current_password": "Admin-Password-1",
                "new_password": "newpassword1"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(body["error"].as_str().unwrap().contains("uppercase"));
}

#[tokio::test]
async fn change_password_no_digit_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/auth/change-password",
            &json!({
                "current_password": "Admin-Password-1",
                "new_password": "NewPasswordOnly"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(body["error"].as_str().unwrap().contains("digit"));
}

// ── must_change_password enforcement ───────────────────────────────────────

/// Admin-created users have must_change_password=true.  They should be able to
/// reach /me and /change-password but all other protected routes must return 403.
#[tokio::test]
async fn must_change_password_blocks_active_routes() {
    let (app, _db_dir) = build_test_app().await;
    let (admin_token, _) = register_admin(&app).await;

    // Admin creates a user — must_change_password is set to true.
    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "forcedchange",
                "password": "Password123",
                "role": "user"
            }),
            &admin_token,
        ),
    )
    .await;
    assert_eq!(body["must_change_password"], true);

    // Log in as the new user.
    let (_, body) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "forcedchange", "password": "Password123"}),
        ),
    )
    .await;
    let user_token = body["token"].as_str().unwrap().to_string();
    assert_eq!(body["user"]["must_change_password"], true);

    // /me must still work (uses plain Claims extractor).
    let (status, _) = send(&app, get_auth("/api/auth/me", &user_token)).await;
    assert_eq!(status, StatusCode::OK);

    // Any ActiveClaims-gated route must be blocked with 403.
    let (status, body) = send(&app, get_auth("/api/providers", &user_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "/api/providers: {body}");
}

#[tokio::test]
async fn must_change_password_unblocked_after_change() {
    let (app, _db_dir) = build_test_app().await;
    let (admin_token, _) = register_admin(&app).await;

    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "willchange",
                "password": "Password123",
                "role": "user"
            }),
            &admin_token,
        ),
    )
    .await;
    assert_eq!(body["must_change_password"], true);

    let (_, body) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "willchange", "password": "Password123"}),
        ),
    )
    .await;
    let user_token = body["token"].as_str().unwrap().to_string();

    // Change password — returns a fresh JWT with must_change_password=false.
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/auth/change-password",
            &json!({
                "current_password": "Password123",
                "new_password": "NewPassword1"
            }),
            &user_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "change pwd: {body}");
    assert_eq!(body["user"]["must_change_password"], false);
    let fresh_token = body["token"].as_str().unwrap().to_string();

    // Protected routes are now accessible with the fresh token.
    let (status, _) = send(&app, get_auth("/api/providers", &fresh_token)).await;
    assert_eq!(status, StatusCode::OK);
}

// ── Provider Tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn list_providers() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(&app, get_auth("/api/providers", &token)).await;
    assert_eq!(status, StatusCode::OK);
    let providers = body.as_array().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["id"], "test-httpbin");
    assert_eq!(providers[0]["name"], "Test Provider");
}

#[tokio::test]
async fn get_provider_details() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(&app, get_auth("/api/providers/test-httpbin", &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "test-httpbin");
    let flows = body["auth_flows"].as_array().unwrap();
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0]["id"], "simple");
    let fields = flows[0]["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 2);
}

#[tokio::test]
async fn get_provider_not_found() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(&app, get_auth("/api/providers/nonexistent", &token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn provider_auth_login_provider_not_found() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(
        &app,
        post_json_auth(
            "/api/providers/nonexistent/auth/login",
            &json!({
                "flow_id": "simple",
                "step": 0,
                "inputs": {"username": "u", "password": "p"}
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn provider_auth_check_not_found() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(
        &app,
        get_auth("/api/providers/nonexistent/auth/check", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn provider_auth_logout_not_found() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(
        &app,
        post_json_auth("/api/providers/nonexistent/auth/logout", &json!({}), &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn provider_auth_check_before_login() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/auth/check", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], false);
}

#[tokio::test]
#[ignore = "requires httpbin (run via ./scripts/integration-test.sh)"]
async fn provider_auth_login() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/login",
            &json!({
                "flow_id": "simple",
                "step": 0,
                "inputs": {"username": "testuser", "password": "testpass"}
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "provider login: {body}");
    assert_eq!(body["success"], true);
}

#[tokio::test]
#[ignore = "requires httpbin (run via ./scripts/integration-test.sh)"]
async fn provider_auth_check_after_login() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    // Login first
    send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/login",
            &json!({
                "flow_id": "simple",
                "step": 0,
                "inputs": {"username": "testuser", "password": "testpass"}
            }),
            &token,
        ),
    )
    .await;

    // Check session
    let (status, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/auth/check", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], true);
}

#[tokio::test]
#[ignore = "requires httpbin (run via ./scripts/integration-test.sh)"]
async fn provider_auth_logout() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    // Login first
    send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/login",
            &json!({
                "flow_id": "simple",
                "step": 0,
                "inputs": {"username": "testuser", "password": "testpass"}
            }),
            &token,
        ),
    )
    .await;

    // Logout
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/logout",
            &json!({}),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "provider logout: {body}");
    assert_eq!(body["success"], true);

    // Check session cleared
    let (_, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/auth/check", &token),
    )
    .await;
    assert_eq!(body["valid"], false);
}

#[tokio::test]
async fn provider_auth_login_invalid_flow() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/login",
            &json!({
                "flow_id": "nonexistent",
                "step": 0,
                "inputs": {}
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn provider_auth_login_invalid_step() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/login",
            &json!({
                "flow_id": "simple",
                "step": 99,
                "inputs": {}
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Channel Tests ───────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires httpbin (run via ./scripts/integration-test.sh)"]
async fn channel_list() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/channels", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "channel list: {body}");
    let channels = body["channels"].as_array().unwrap();
    assert!(
        !channels.is_empty(),
        "should have channels from httpbin /json"
    );
    assert!(channels[0]["id"].is_string());
    assert!(channels[0]["name"].is_string());
}

#[tokio::test]
async fn channel_categories_static() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/channels/categories", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "categories: {body}");
    let categories = body["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 2);
    assert_eq!(categories[0]["id"], "1");
    assert_eq!(categories[0]["name"], "Entertainment");
    assert_eq!(categories[1]["id"], "2");
    assert_eq!(categories[1]["name"], "News");
}

#[tokio::test]
#[ignore = "requires httpbin (run via ./scripts/integration-test.sh)"]
async fn channel_stream_url() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/channels/ch42/stream", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "stream: {body}");
    assert!(body["url"].is_string());
    assert_eq!(body["stream_type"], "hls");
}

#[tokio::test]
async fn channel_list_provider_not_found() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, _) = send(
        &app,
        get_auth("/api/providers/nonexistent/channels", &token),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Global-scope provider access control ────────────────────────────────────

#[tokio::test]
async fn global_provider_login_requires_admin() {
    // Build a fresh app with a global-scoped variant of the test provider.
    sqlx::any::install_default_drivers();
    let dir = tempfile::tempdir().unwrap();
    let db_url = format!("sqlite://{}", dir.path().join("test.db").display());
    let db = otvi_server::db::init(&db_url).await.unwrap();

    let mut yaml = include_str!("fixtures/httpbin-provider.yaml").to_string();
    // Patch scope to global.
    yaml = yaml.replace("scope: per_user", "scope: global");
    if let Ok(url) = std::env::var("HTTPBIN_URL") {
        yaml = yaml.replace("https://httpbin.org", &url);
    }
    let provider: otvi_core::config::ProviderConfig = serde_yaml_ng::from_str(&yaml).unwrap();
    let mut providers = std::collections::HashMap::new();
    providers.insert(provider.provider.id.clone(), provider);

    let state = Arc::new(AppState {
        providers_rw: std::sync::RwLock::new(providers),
        db,
        jwt_keys: otvi_server::auth_middleware::JwtKeys::new(b"global-test-secret"),
        http_client: reqwest::Client::new(),
        proxy_ctx: std::sync::RwLock::new(std::collections::HashMap::new()),
        channel_cache: otvi_server::state::ChannelCache::new(std::time::Duration::from_secs(300)),
    });
    let app = otvi_server::build_router_without_rate_limit(state);

    // Register admin + regular user.
    let (admin_token, _) = {
        let (_, body) = send(
            &app,
            post_json(
                "/api/auth/register",
                &json!({"username": "admin", "password": "Admin-Password-1"}),
            ),
        )
        .await;
        let token = body["token"].as_str().unwrap().to_string();
        let id = body["user"]["id"].as_str().unwrap().to_string();
        (token, id)
    };
    let (_, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "regular", "password": "UserPass123"}),
        ),
    )
    .await;
    let user_token = body["token"].as_str().unwrap().to_string();

    // Regular user must be denied (403) on a global-scoped provider login.
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/login",
            &json!({
                "flow_id": "simple",
                "step": 0,
                "inputs": {"username": "u", "password": "p"}
            }),
            &user_token,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "regular user on global provider: {body}"
    );

    // Admin must be allowed (login attempt itself may fail due to network, but
    // the auth check must pass — we get either 200 or a provider error, not 403).
    let (status, _) = send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/login",
            &json!({
                "flow_id": "simple",
                "step": 0,
                "inputs": {"username": "u", "password": "p"}
            }),
            &admin_token,
        ),
    )
    .await;
    assert_ne!(status, StatusCode::FORBIDDEN);
}

// ── Proxy Tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn proxy_rejects_invalid_url() {
    let (app, _db_dir) = build_test_app().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/proxy?url=not-a-valid-url")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires httpbin (run via ./scripts/integration-test.sh)"]
async fn proxy_fetches_upstream() {
    let (app, _db_dir) = build_test_app().await;
    let httpbin = std::env::var("HTTPBIN_URL").unwrap_or("https://httpbin.org".into());
    let url = format!("{httpbin}/get");
    let encoded = urlencoding::encode(&url);

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/proxy?url={encoded}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Infrastructure endpoints ─────────────────────────────────────────────────

#[tokio::test]
async fn healthz_returns_ok() {
    let (app, _db_dir) = build_test_app().await;
    let req = Request::builder()
        .method("GET")
        .uri("/healthz")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn readyz_returns_ok_with_healthy_db() {
    let (app, _db_dir) = build_test_app().await;
    let req = Request::builder()
        .method("GET")
        .uri("/readyz")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn provider_schema_returns_json_schema() {
    let (app, _db_dir) = build_test_app().await;
    let req = Request::builder()
        .method("GET")
        .uri("/api/schema/provider")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("json"), "expected JSON content-type, got: {ct}");
}

#[tokio::test]
async fn openapi_docs_endpoint_reachable() {
    let (app, _db_dir) = build_test_app().await;
    // The swagger UI redirects from /api/docs to /api/docs/ — just check it
    // does not return a server error.
    let req = Request::builder()
        .method("GET")
        .uri("/api/docs/openapi.json")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Admin Tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_list_users() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(&app, get_auth("/api/admin/users", &token)).await;
    assert_eq!(status, StatusCode::OK);
    let users = body.as_array().unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0]["username"], "admin");
    assert_eq!(users[0]["role"], "admin");
}

#[tokio::test]
async fn admin_create_user() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "newuser",
                "password": "UserPass123",
                "role": "user",
                "providers": []
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create user: {body}");
    assert_eq!(body["username"], "newuser");
    assert_eq!(body["role"], "user");
    assert_eq!(body["must_change_password"], true);
}

#[tokio::test]
async fn admin_create_user_weak_password_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    // No uppercase
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "weakuser",
                "password": "password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no uppercase: {body}");
    assert!(body["error"].as_str().unwrap().contains("uppercase"));

    // No digit
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "weakuser",
                "password": "PasswordOnly",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no digit: {body}");
    assert!(body["error"].as_str().unwrap().contains("digit"));

    // Too short
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "weakuser",
                "password": "Sh0rt",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "too short: {body}");
    assert!(body["error"].as_str().unwrap().contains("8 characters"));
}

#[tokio::test]
async fn admin_create_user_empty_username_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "",
                "password": "Password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}

#[tokio::test]
async fn admin_create_user_duplicate() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "dupuser",
                "password": "Password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;

    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "dupuser",
                "password": "Password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("already taken"));
}

#[tokio::test]
async fn admin_delete_user() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "todelete",
                "password": "Password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    let user_id = body["id"].as_str().unwrap();

    let (status, body) = send(
        &app,
        delete_auth(&format!("/api/admin/users/{user_id}"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);

    let (_, body) = send(&app, get_auth("/api/admin/users", &token)).await;
    let users = body.as_array().unwrap();
    assert_eq!(users.len(), 1);
}

#[tokio::test]
async fn admin_cannot_delete_self() {
    let (app, _db_dir) = build_test_app().await;
    let (token, admin_id) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        delete_auth(&format!("/api/admin/users/{admin_id}"), &token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("own account"));
}

#[tokio::test]
async fn admin_set_user_providers() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "limited",
                "password": "Password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    let user_id = body["id"].as_str().unwrap();

    let (status, body) = send(
        &app,
        put_json_auth(
            &format!("/api/admin/users/{user_id}/providers"),
            &json!({"providers": ["test-httpbin"]}),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn admin_reset_user_password() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "resetme",
                "password": "Password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    let user_id = body["id"].as_str().unwrap();

    let (status, body) = send(
        &app,
        put_json_auth(
            &format!("/api/admin/users/{user_id}/password"),
            &json!({"new_password": "NewPassword1"}),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);

    let (status, body) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "resetme", "password": "NewPassword1"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login after reset: {body}");
    assert_eq!(body["user"]["must_change_password"], true);
}

#[tokio::test]
async fn admin_reset_password_weak_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "weakreset",
                "password": "Password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    let user_id = body["id"].as_str().unwrap();

    // No uppercase
    let (status, body) = send(
        &app,
        put_json_auth(
            &format!("/api/admin/users/{user_id}/password"),
            &json!({"new_password": "newpassword1"}),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no uppercase: {body}");
    assert!(body["error"].as_str().unwrap().contains("uppercase"));

    // No digit
    let (status, body) = send(
        &app,
        put_json_auth(
            &format!("/api/admin/users/{user_id}/password"),
            &json!({"new_password": "PasswordOnly"}),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no digit: {body}");
    assert!(body["error"].as_str().unwrap().contains("digit"));
}

#[tokio::test]
async fn admin_reset_password_empty_rejected() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "emptypass",
                "password": "Password123",
                "role": "user"
            }),
            &token,
        ),
    )
    .await;
    let user_id = body["id"].as_str().unwrap();

    let (status, _) = send(
        &app,
        put_json_auth(
            &format!("/api/admin/users/{user_id}/password"),
            &json!({"new_password": ""}),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn admin_get_settings() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(&app, get_auth("/api/admin/settings", &token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["signup_disabled"], false);
}

#[tokio::test]
async fn admin_update_settings_disable_signup() {
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        put_json_auth(
            "/api/admin/settings",
            &json!({"signup_disabled": true}),
            &token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);

    let (_, body) = send(&app, get_auth("/api/admin/settings", &token)).await;
    assert_eq!(body["signup_disabled"], true);

    let (status, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "newguy", "password": "Password123"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("disabled"));
}

#[tokio::test]
async fn regular_user_cannot_access_admin() {
    let (app, _db_dir) = build_test_app().await;
    register_admin(&app).await;

    let (_, body) = send(
        &app,
        post_json(
            "/api/auth/register",
            &json!({"username": "regular", "password": "UserPass123"}),
        ),
    )
    .await;
    let user_token = body["token"].as_str().unwrap();

    let (status, _) = send(&app, get_auth("/api/admin/users", user_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(&app, get_auth("/api/admin/settings", user_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_delete_nonexistent_user_succeeds_silently() {
    // DELETE is idempotent — deleting a user ID that does not exist should not
    // return an error (the DB silently deletes 0 rows).
    let (app, _db_dir) = build_test_app().await;
    let (token, _) = register_admin(&app).await;

    let (status, body) = send(
        &app,
        delete_auth("/api/admin/users/nonexistent-uuid", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn admin_set_providers_empty_restores_full_access() {
    let (app, _db_dir) = build_test_app().await;
    let (admin_token, _) = register_admin(&app).await;

    // Create a user restricted to a fake provider.
    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "restoreme",
                "password": "Password123",
                "role": "user",
                "providers": ["nonexistent"]
            }),
            &admin_token,
        ),
    )
    .await;
    let user_id = body["id"].as_str().unwrap().to_string();

    // Login + change password (must_change_password=true for admin-created users).
    let (_, body) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "restoreme", "password": "Password123"}),
        ),
    )
    .await;
    let user_token = body["token"].as_str().unwrap().to_string();
    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/auth/change-password",
            &json!({
                "current_password": "Password123",
                "new_password": "NewPassword1"
            }),
            &user_token,
        ),
    )
    .await;
    let user_token = body["token"].as_str().unwrap().to_string();

    // Restricted: test-httpbin should NOT appear.
    let (_, body) = send(&app, get_auth("/api/providers", &user_token)).await;
    assert!(body.as_array().unwrap().is_empty(), "should be empty");

    // Admin clears the restriction (empty providers = all).
    let (status, _) = send(
        &app,
        put_json_auth(
            &format!("/api/admin/users/{user_id}/providers"),
            &json!({"providers": []}),
            &admin_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Now test-httpbin should appear.
    let (_, body) = send(&app, get_auth("/api/providers", &user_token)).await;
    assert_eq!(
        body.as_array().unwrap().len(),
        1,
        "should see all providers"
    );
}

// ── Provider Access Control Tests ───────────────────────────────────────────

#[tokio::test]
async fn user_with_restricted_providers() {
    let (app, _db_dir) = build_test_app().await;
    let (admin_token, _) = register_admin(&app).await;

    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "restricted",
                "password": "Password123",
                "role": "user",
                "providers": ["nonexistent-provider"]
            }),
            &admin_token,
        ),
    )
    .await;
    assert!(body["id"].is_string());

    let (_, body) = send(
        &app,
        post_json(
            "/api/auth/login",
            &json!({"username": "restricted", "password": "Password123"}),
        ),
    )
    .await;
    let user_token = body["token"].as_str().unwrap();

    // Admin-created accounts have must_change_password=true; change it first.
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/auth/change-password",
            &json!({
                "current_password": "Password123",
                "new_password": "NewPassword1"
            }),
            user_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "change password: {body}");
    let user_token = body["token"].as_str().unwrap();

    let (status, body) = send(&app, get_auth("/api/providers", user_token)).await;
    assert_eq!(status, StatusCode::OK);
    let providers = body.as_array().unwrap();
    assert!(providers.is_empty());
}

// ── Full End-to-End Flow ────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires httpbin (run via ./scripts/integration-test.sh)"]
async fn full_e2e_flow() {
    let (app, _db_dir) = build_test_app().await;

    // 1. Register admin
    let (admin_token, admin_id) = register_admin(&app).await;

    // 2. Get settings
    let (status, body) = send(&app, get_auth("/api/admin/settings", &admin_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["signup_disabled"], false);

    // 3. List providers
    let (status, body) = send(&app, get_auth("/api/providers", &admin_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);

    // 4. Get provider details
    let (status, body) = send(&app, get_auth("/api/providers/test-httpbin", &admin_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "test-httpbin");

    // 5. Check provider auth (not logged in)
    let (_, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/auth/check", &admin_token),
    )
    .await;
    assert_eq!(body["valid"], false);

    // 6. Provider login
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/login",
            &json!({
                "flow_id": "simple",
                "step": 0,
                "inputs": {"username": "alice", "password": "secret"}
            }),
            &admin_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "provider login: {body}");
    assert_eq!(body["success"], true);

    // 7. Check provider auth (logged in)
    let (_, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/auth/check", &admin_token),
    )
    .await;
    assert_eq!(body["valid"], true);

    // 8. List channels
    let (status, body) = send(
        &app,
        get_auth("/api/providers/test-httpbin/channels", &admin_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "channels: {body}");
    assert!(!body["channels"].as_array().unwrap().is_empty());

    // 9. List categories (static)
    let (status, body) = send(
        &app,
        get_auth(
            "/api/providers/test-httpbin/channels/categories",
            &admin_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["categories"].as_array().unwrap().len(), 2);

    // 10. Get stream URL
    let (status, body) = send(
        &app,
        get_auth(
            "/api/providers/test-httpbin/channels/ch1/stream",
            &admin_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "stream: {body}");
    assert!(body["url"].is_string());
    assert_eq!(body["stream_type"], "hls");

    // 11. Admin creates a regular user
    let (_, body) = send(
        &app,
        post_json_auth(
            "/api/admin/users",
            &json!({
                "username": "viewer",
                "password": "ViewerPass1",
                "role": "user"
            }),
            &admin_token,
        ),
    )
    .await;
    let viewer_id = body["id"].as_str().unwrap().to_string();

    // 12. List users
    let (_, body) = send(&app, get_auth("/api/admin/users", &admin_token)).await;
    assert_eq!(body.as_array().unwrap().len(), 2);

    // 13. Provider logout
    let (status, body) = send(
        &app,
        post_json_auth(
            "/api/providers/test-httpbin/auth/logout",
            &json!({}),
            &admin_token,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "provider logout: {body}");
    assert_eq!(body["success"], true);

    // 14. Delete user
    let (status, _) = send(
        &app,
        delete_auth(&format!("/api/admin/users/{viewer_id}"), &admin_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // 15. Cannot delete self
    let (status, _) = send(
        &app,
        delete_auth(&format!("/api/admin/users/{admin_id}"), &admin_token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 16. Me endpoint
    let (status, body) = send(&app, get_auth("/api/auth/me", &admin_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], "admin");
}
