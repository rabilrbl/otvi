//! Admin-only endpoints for user and server management.
//!
//! Password policy is enforced via the shared [`validate_password`] function
//! from `api::user_auth`, ensuring consistency across all password-setting
//! paths (self-registration, change-password, admin create, admin reset).
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

use axum::Json;
use axum::extract::{Path, State};

use otvi_core::types::{
    AdminResetPasswordRequest, CreateUserRequest, ServerSettings, UpdateUserProvidersRequest,
    UserInfo, UserRole,
};

use crate::api::user_auth::{hash_password, validate_password};
use crate::auth_middleware::AdminClaims;
use crate::db;
use crate::error::AppError;
use crate::state::AppState;

// ── User management ────────────────────────────────────────────────────────

/// `GET /api/admin/users`
#[utoipa::path(
    get,
    path = "/api/admin/users",
    tag = "admin",
    security(("bearer_token" = [])),
    responses(
        (status = 200, description = "List of all OTVI users", body = Vec<UserInfo>),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    _: AdminClaims,
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
            must_change_password: row.must_change_password,
        });
    }

    Ok(Json(users))
}

/// `POST /api/admin/users`
#[utoipa::path(
    post,
    path = "/api/admin/users",
    tag = "admin",
    security(("bearer_token" = [])),
    request_body = CreateUserRequest,
    responses(
        (status = 200, description = "User created; `must_change_password` is always `true`", body = UserInfo),
        (status = 400, description = "Invalid input, weak password, or username already taken"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn create_user(
    State(state): State<Arc<AppState>>,
    _: AdminClaims,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserInfo>, AppError> {
    if req.username.trim().is_empty() || req.password.is_empty() {
        return Err(AppError::BadRequest(
            "Username and password are required".into(),
        ));
    }
    // Enforce shared password policy (min 8 chars, uppercase, digit).
    validate_password(&req.password)?;

    if db::get_user_by_username(&state.db, &req.username)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::BadRequest("Username already taken".into()));
    }

    let hash = hash_password(&req.password)?;
    let user_id = db::create_user(&state.db, &req.username, &hash, &req.role, true)
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
        must_change_password: true,
    }))
}

/// `DELETE /api/admin/users/:id`
#[utoipa::path(
    delete,
    path = "/api/admin/users/{id}",
    tag = "admin",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "User ID to delete"),
    ),
    responses(
        (status = 200, description = "User deleted"),
        (status = 400, description = "Cannot delete your own account"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    admin: AdminClaims,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Prevent an admin from deleting themselves.
    if admin.sub == user_id {
        return Err(AppError::BadRequest(
            "Cannot delete your own account".into(),
        ));
    }

    db::delete_user(&state.db, &user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ── User management (continued) ────────────────────────────────────────────

/// `PUT /api/admin/users/:id/providers`
///
/// Replace the provider allow-list for a user.
/// Send an empty `providers` array to grant access to all providers.
#[utoipa::path(
    put,
    path = "/api/admin/users/{id}/providers",
    tag = "admin",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    request_body = UpdateUserProvidersRequest,
    responses(
        (status = 200, description = "Provider allow-list updated"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn set_user_providers(
    State(state): State<Arc<AppState>>,
    _: AdminClaims,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserProvidersRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::set_user_providers(&state.db, &user_id, &req.providers)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// `PUT /api/admin/users/:id/password`
///
/// Admin resets a user's password and forces a password-change on next login.
#[utoipa::path(
    put,
    path = "/api/admin/users/{id}/password",
    tag = "admin",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    request_body = AdminResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset; `must_change_password` re-armed"),
        (status = 400, description = "Empty or policy-violating password"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn reset_user_password(
    State(state): State<Arc<AppState>>,
    _: AdminClaims,
    Path(user_id): Path<String>,
    Json(req): Json<AdminResetPasswordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if req.new_password.is_empty() {
        return Err(AppError::BadRequest("Password must not be empty".into()));
    }
    // Enforce the same password policy used everywhere else.
    validate_password(&req.new_password)?;

    let hash = hash_password(&req.new_password)?;
    db::update_password(&state.db, &user_id, &hash)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    // Re-arm the must_change_password flag so the user is forced to change on
    // next login (update_password clears it, so we set it again here).
    db::set_must_change_password(&state.db, &user_id, true)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ── Server settings ────────────────────────────────────────────────────────

/// `GET /api/admin/settings`
#[utoipa::path(
    get,
    path = "/api/admin/settings",
    tag = "admin",
    security(("bearer_token" = [])),
    responses(
        (status = 200, description = "Current server settings", body = ServerSettings),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    _: AdminClaims,
) -> Result<Json<ServerSettings>, AppError> {
    let signup_disabled = db::is_signup_disabled(&state.db)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(ServerSettings { signup_disabled }))
}

/// `PUT /api/admin/settings`
#[utoipa::path(
    put,
    path = "/api/admin/settings",
    tag = "admin",
    security(("bearer_token" = [])),
    request_body = ServerSettings,
    responses(
        (status = 200, description = "Settings updated"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    _: AdminClaims,
    Json(req): Json<ServerSettings>,
) -> Result<Json<serde_json::Value>, AppError> {
    db::set_signup_disabled(&state.db, req.signup_disabled)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}
