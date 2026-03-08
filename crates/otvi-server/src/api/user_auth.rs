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

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::Json;
use axum::extract::State;

use otvi_core::types::{
    AppLoginRequest, AppLoginResponse, ChangePasswordRequest, RegisterRequest, UserInfo, UserRole,
};

// ── Password policy ────────────────────────────────────────────────────────

/// Shared password-strength validator used by registration, change-password,
/// and admin reset.
///
/// # Rules
/// - At least 8 characters.
/// - At least one uppercase ASCII letter.
/// - At least one ASCII digit.
///
/// Returns `Ok(())` on success or an `AppError::BadRequest` with a descriptive
/// message on failure.
pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".into(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(AppError::BadRequest(
            "Password must contain at least one uppercase letter".into(),
        ));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "Password must contain at least one digit".into(),
        ));
    }
    Ok(())
}

use crate::auth_middleware::{Claims, create_token};
use crate::db;
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
    // Reject if the admin has disabled public signup.
    if db::is_signup_disabled(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
    {
        return Err(AppError::BadRequest(
            "Public registration is disabled. Contact your administrator.".into(),
        ));
    }

    if req.username.trim().is_empty() || req.password.is_empty() {
        return Err(AppError::BadRequest(
            "Username and password are required".into(),
        ));
    }
    // Enforce shared password policy on self-registration.
    validate_password(&req.password)?;

    // Check for duplicate username.
    if db::get_user_by_username(&state.db, &req.username)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::BadRequest("Username already taken".into()));
    }

    // First ever user automatically becomes admin.
    let count = db::user_count(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let role = if count == 0 {
        UserRole::Admin
    } else {
        UserRole::User
    };

    let hash = hash_password(&req.password)?;
    // Self-registered users never have must_change_password set.
    let must_change_password = false;
    let user_id = db::create_user(&state.db, &req.username, &hash, &role, must_change_password)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let providers = db::get_user_providers(&state.db, &user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let token = create_token(
        &state.jwt_keys,
        &user_id,
        &req.username,
        &role,
        must_change_password,
    );

    Ok(Json(AppLoginResponse {
        token,
        user: UserInfo {
            id: user_id,
            username: req.username,
            role,
            providers,
            must_change_password,
        },
    }))
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
    let row = db::get_user_by_username(&state.db, &req.username)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    verify_password(&req.password, &row.password_hash)?;

    let role = match row.role.as_str() {
        "admin" => UserRole::Admin,
        _ => UserRole::User,
    };

    let providers = db::get_user_providers(&state.db, &row.id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Embed must_change_password in the JWT so middleware guards need no DB
    // query per request.
    let token = create_token(
        &state.jwt_keys,
        &row.id,
        &row.username,
        &role,
        row.must_change_password,
    );

    Ok(Json(AppLoginResponse {
        token,
        user: UserInfo {
            id: row.id,
            username: row.username,
            role,
            providers,
            must_change_password: row.must_change_password,
        },
    }))
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
) -> Result<Json<UserInfo>, AppError> {
    // Single query covers both must_change_password and basic user fields.
    // We still need a separate providers query since they live in a different
    // table, but we save one DB round-trip vs. the old pattern.
    let row = db::get_user_by_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    let providers = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let role = match row.role.as_str() {
        "admin" => UserRole::Admin,
        _ => UserRole::User,
    };

    Ok(Json(UserInfo {
        id: row.id,
        username: row.username,
        role,
        providers,
        must_change_password: row.must_change_password,
    }))
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
    // Validate new password against the shared policy.
    validate_password(&req.new_password)?;

    let row = db::get_user_by_id(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or(AppError::Unauthorized)?;

    verify_password(&req.current_password, &row.password_hash)?;

    let new_hash = hash_password(&req.new_password)?;
    // update_password also clears must_change_password = 0 in the DB.
    db::update_password(&state.db, &claims.sub, &new_hash)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let role = claims.role();
    let providers = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Issue a fresh token with must_change_password = false so the client is
    // immediately unblocked without needing to re-login.
    let token = create_token(
        &state.jwt_keys,
        &claims.sub,
        &claims.username,
        &role,
        false, // flag cleared
    );

    Ok(Json(AppLoginResponse {
        token,
        user: UserInfo {
            id: claims.sub,
            username: claims.username,
            role,
            providers,
            must_change_password: false,
        },
    }))
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

/// Hash `password` with Argon2id.
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Password hash error: {e}")))
}

/// Verify `password` against an Argon2 `hash`.  Returns `AppError::Unauthorized`
/// when the password does not match.
pub fn verify_password(password: &str, hash: &str) -> Result<(), AppError> {
    let parsed =
        PasswordHash::new(hash).map_err(|e| AppError::Internal(format!("Invalid hash: {e}")))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AppError::Unauthorized)
}
