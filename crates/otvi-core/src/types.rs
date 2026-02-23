//! Shared request / response types used by both the backend REST API and the
//! frontend WASM client.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Provider info (read-only, returned to frontend) ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub logo: Option<String>,
    pub auth_flows: Vec<AuthFlowInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFlowInfo {
    pub id: String,
    pub name: String,
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub key: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
}

// ── Auth request / response ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub flow_id: String,
    pub step: usize,
    pub inputs: HashMap<String, String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub next_step: Option<NextStepInfo>,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Returned when a multi-step auth flow requires additional user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStepInfo {
    pub step_index: usize,
    pub step_name: String,
    pub fields: Vec<FieldInfo>,
}

// ── Channels ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelListResponse {
    pub channels: Vec<Channel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryListResponse {
    pub categories: Vec<Category>,
}

// ── Playback ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub url: String,
    pub stream_type: StreamType,
    #[serde(default)]
    pub drm: Option<DrmInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    Hls,
    Dash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrmInfo {
    pub system: String,
    pub license_url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

// ── OTVI user account (application-level auth, independent of providers) ────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    User,
}

/// Information about the currently authenticated OTVI user.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub role: UserRole,
    /// Provider IDs this user has access to.
    pub providers: Vec<String>,
    /// When `true` the user must change their password before proceeding.
    #[serde(default)]
    pub must_change_password: bool,
}

// ── OTVI app-level register / login / logout ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLoginResponse {
    pub token: String,
    pub user: UserInfo,
}

// ── Admin: user management ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: UserRole,
    /// Provider IDs to grant access to (empty = all providers).
    #[serde(default)]
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserProvidersRequest {
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminResetPasswordRequest {
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    /// When `true`, the public `/api/auth/register` endpoint is disabled;
    /// only admins can create new accounts via `/api/admin/users`.
    pub signup_disabled: bool,
}
