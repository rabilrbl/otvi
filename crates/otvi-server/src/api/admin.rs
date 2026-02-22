//! Admin-only endpoints for user and server management.
//!
//! All routes under `/api/admin/…` require a valid JWT with `role == "admin"`.
//!
//! Routes:
//!   GET    /api/admin/users              — list all OTVI users
//!   POST   /api/admin/users              — create a new user
//!   DELETE /api/admin/users/:id          — delete a user
//!   PUT    /api/admin/users/:id/providers — set a user's provider allow-list
//!   GET    /api/admin/settings           — get server settings
//!   PUT    /api/admin/settings           — update server settings

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;

use otvi_core::types::*;

use crate::api::user_auth::hash_password;
use crate::auth_middleware::AdminClaims;
use crate::db;
use crate::error::AppError;
use crate::state::AppState;

// ── User management ────────────────────────────────────────────────────────

/// `GET /api/admin/users`
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    AdminClaims(_): AdminClaims,
) -> Result<Json<Vec<UserInfo>>, AppError> {
    let rows = db::list_users(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut users = Vec::new();
    for row in rows {
        let providers = db::get_user_providers(&state.db, &row.id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let role = match row.role.as_str() {
            "admin" => UserRole::Admin,
            _ => UserRole::User,
        };
        users.push(UserInfo {
            id: row.id,
            username: row.username,
            role,
            providers,
        });
    }

    Ok(Json(users))
}

/// `POST /api/admin/users`
pub async fn create_user(
    State(state): State<Arc<AppState>>,
    AdminClaims(_): AdminClaims,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserInfo>, AppError> {
    if req.username.trim().is_empty() || req.password.is_empty() {
        return Err(AppError::BadRequest(
            "Username and password are required".into(),
        ));
    }

    if db::get_user_by_username(&state.db, &req.username)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::BadRequest("Username already taken".into()));
    }

    let hash = hash_password(&req.password)?;
    let user_id = db::create_user(&state.db, &req.username, &hash, &req.role)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Restrict to specified providers (empty = all).
    db::set_user_providers(&state.db, &user_id, &req.providers)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(UserInfo {
        id: user_id,
        username: req.username,
        role: req.role,
        providers: req.providers,
    }))
}

/// `DELETE /api/admin/users/:id`
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    AdminClaims(admin): AdminClaims,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Prevent an admin from deleting themselves.
    if admin.sub == user_id {
        return Err(AppError::BadRequest("Cannot delete your own account".into()));
    }

    db::delete_user(&state.db, &user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// `PUT /api/admin/users/:id/providers`
///
/// Replace the provider allow-list for a user.
/// Send an empty `providers` array to grant access to all providers.
pub async fn set_user_providers(
    State(state): State<Arc<AppState>>,
    AdminClaims(_): AdminClaims,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserProvidersRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::set_user_providers(&state.db, &user_id, &req.providers)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ── Server settings ────────────────────────────────────────────────────────

/// `GET /api/admin/settings`
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    AdminClaims(_): AdminClaims,
) -> Result<Json<ServerSettings>, AppError> {
    let signup_disabled = db::is_signup_disabled(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ServerSettings { signup_disabled }))
}

/// `PUT /api/admin/settings`
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    AdminClaims(_): AdminClaims,
    Json(req): Json<ServerSettings>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::set_signup_disabled(&state.db, req.signup_disabled)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}
