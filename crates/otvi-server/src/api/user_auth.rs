//! OTVI application-level user authentication.
//!
//! These endpoints manage OTVI accounts (independent of any TV provider).
//! Provider-level authentication lives in `api/auth.rs`.
//!
//! Routes:
//!   POST  /api/auth/register        — create an account (disabled by admin if signup is off)
//!   POST  /api/auth/login           — exchange username+password for a JWT
//!   GET   /api/auth/me              — return the currently authenticated user's info
//!   POST  /api/auth/logout          — no-op; clients simply discard their JWT
//!   POST  /api/auth/change-password — change password; clears `must_change_password`
//!
//! ## Password policy
//!
//! All passwords (registration, change, admin reset) are validated through the
//! shared [`validate_password`] function which enforces:
//! - Minimum 8 characters
//! - At least one uppercase letter
//! - At least one digit
//!
//! ## must_change_password enforcement
//!
//! When a user has `must_change_password = true` the server **rejects all API
//! calls** (returning `403 Forbidden`) except for `POST /api/auth/change-password`
//! and `GET /api/auth/me`.  This is enforced centrally by the [`ActiveClaims`]
//! extractor in `auth_middleware` — handlers that must remain reachable while
//! the flag is set use the plain [`Claims`] extractor instead.
//!
//! The `must_change_password` flag is embedded directly in the JWT at issuance
//! time so the middleware guard requires **no database round-trip** per request.
//!
//! [`ActiveClaims`]: crate::auth_middleware::ActiveClaims
//! [`Claims`]: crate::auth_middleware::Claims

use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use otvi_core::types::{
    AppLoginRequest, AppLoginResponse, ChangePasswordRequest, RegisterRequest, UserInfo,
};

use crate::account;
use crate::auth_middleware::Claims;
use crate::error::AppError;
use crate::state::AppState;

// ── Handlers ──────────────────────────────────────────────────────────────

/// `POST /api/auth/register`
#[utoipa::path(
    post,
    path = "/api/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "Registration successful", body = AppLoginResponse),
        (status = 400, description = "Invalid input or username already taken"),
    ),
)]
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AppLoginResponse>, AppError> {
    account::register(&state, req).await.map(Json)
}

/// `POST /api/auth/login`
#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "auth",
    request_body = AppLoginRequest,
    responses(
        (status = 200, description = "Login successful, returns JWT", body = AppLoginResponse),
        (status = 401, description = "Invalid credentials"),
    ),
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AppLoginRequest>,
) -> Result<Json<AppLoginResponse>, AppError> {
    account::login(&state, req).await.map(Json)
}

/// `GET /api/auth/me`
///
/// Returns current user info.  A single DB query fetches the full user row
/// (which includes `must_change_password`) along with the provider list,
/// avoiding the previous two-query pattern.
#[utoipa::path(
    get,
    path = "/api/auth/me",
    tag = "auth",
    security(("bearer_token" = [])),
    responses(
        (status = 200, description = "Current authenticated user info", body = UserInfo),
        (status = 401, description = "Missing or invalid token"),
    ),
)]
pub async fn me(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<otvi_core::types::UserInfo>, AppError> {
    account::current_user(&state, &claims).await.map(Json)
}

/// `POST /api/auth/change-password`
///
/// Authenticated users change their own password.  On success the
/// `must_change_password` flag is cleared, the DB is updated, and a fresh
/// JWT (with `must_change_password = false` embedded) is returned so the
/// client's next request is immediately unblocked without a re-login.
///
/// This endpoint is intentionally **exempt** from the `must_change_password`
/// guard (uses plain [`Claims`] instead of `ActiveClaims`) — it must remain
/// reachable when the flag is set so the user can clear it.
#[utoipa::path(
    post,
    path = "/api/auth/change-password",
    tag = "auth",
    security(("bearer_token" = [])),
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed, returns fresh JWT", body = AppLoginResponse),
        (status = 400, description = "Password does not meet policy requirements"),
        (status = 401, description = "Missing/invalid token or wrong current password"),
    ),
)]
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<AppLoginResponse>, AppError> {
    account::change_password(&state, &claims, req)
        .await
        .map(Json)
}

/// `POST /api/auth/logout` — JWT is stateless; the client drops its token.
/// This endpoint exists so the frontend can call a logout URL uniformly.
#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "auth",
    security(("bearer_token" = [])),
    responses(
        (status = 200, description = "Always succeeds; client discards its token"),
    ),
)]
pub async fn logout() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "success": true }))
}

// ── Helpers ───────────────────────────────────────────────────────────────
