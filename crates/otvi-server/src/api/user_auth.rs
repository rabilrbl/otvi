//! OTVI application-level user authentication.
//!
//! These endpoints manage OTVI accounts (independent of any TV provider).
//! Provider-level authentication lives in `api/auth.rs`.
//!
//! Routes:
//!   POST  /api/auth/register — create an account (disabled by admin if signup is off)
//!   POST  /api/auth/login    — exchange username+password for a JWT
//!   GET   /api/auth/me       — return the currently authenticated user's info
//!   POST  /api/auth/logout   — no-op; clients simply discard their JWT

use std::sync::Arc;

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::Json;
use axum::extract::State;

use otvi_core::types::{AppLoginRequest, AppLoginResponse, RegisterRequest, UserInfo, UserRole};

use crate::auth_middleware::{Claims, create_token};
use crate::db;
use crate::error::AppError;
use crate::state::AppState;

/// `POST /api/auth/register`
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
    let user_id = db::create_user(&state.db, &req.username, &hash, &role)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let providers = db::get_user_providers(&state.db, &user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let token = create_token(&state.jwt_keys, &user_id, &req.username, &role);

    Ok(Json(AppLoginResponse {
        token,
        user: UserInfo {
            id: user_id,
            username: req.username,
            role,
            providers,
        },
    }))
}

/// `POST /api/auth/login`
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

    let token = create_token(&state.jwt_keys, &row.id, &row.username, &role);

    Ok(Json(AppLoginResponse {
        token,
        user: UserInfo {
            id: row.id,
            username: row.username,
            role,
            providers,
        },
    }))
}

/// `GET /api/auth/me`
pub async fn me(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<UserInfo>, AppError> {
    let providers = db::get_user_providers(&state.db, &claims.sub)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let role = claims.role();
    let id = claims.sub;
    let username = claims.username;

    Ok(Json(UserInfo {
        id,
        username,
        role,
        providers,
    }))
}

/// `POST /api/auth/logout` — JWT is stateless; the client drops its token.
/// This endpoint exists so the frontend can call a logout URL uniformly.
pub async fn logout() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "success": true }))
}

// ── Helpers ────────────────────────────────────────────────────────────────

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Password hash error: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> Result<(), AppError> {
    let parsed =
        PasswordHash::new(hash).map_err(|e| AppError::Internal(format!("Invalid hash: {e}")))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AppError::Unauthorized)
}
