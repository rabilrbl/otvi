//! HTTP client for communicating with the OTVI backend API from WASM.

use gloo_net::http::{Request, RequestBuilder, Response};
use gloo_storage::{LocalStorage, Storage};
use otvi_core::types::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

// ── JWT token management ─────────────────────────────────────────────────────

const JWT_KEY: &str = "otvi_jwt";

pub fn store_token(token: &str) {
    let _ = LocalStorage::set(JWT_KEY, token);
}

pub fn get_token() -> Option<String> {
    LocalStorage::get::<String>(JWT_KEY).ok()
}

pub fn clear_token() {
    LocalStorage::delete(JWT_KEY);
}

fn bearer() -> Option<String> {
    get_token().map(|t| format!("Bearer {t}"))
}

enum Method {
    Get,
    Post,
    Put,
    Delete,
}

enum FailureText {
    Status,
    BodyOr(&'static str),
    BodyOrStatus,
    UnauthorizedOrStatus,
}

impl FailureText {
    async fn message_for(self, resp: Response) -> String {
        let status = resp.status();
        match self {
            Self::Status => format!("HTTP {status}"),
            Self::UnauthorizedOrStatus if status == 401 => "__unauthorized__".into(),
            Self::UnauthorizedOrStatus => format!("HTTP {status}"),
            Self::BodyOr(fallback) => resp.text().await.unwrap_or_else(|_| fallback.into()),
            Self::BodyOrStatus => resp
                .text()
                .await
                .unwrap_or_else(|_| format!("HTTP {status}")),
        }
    }
}

fn request(method: Method, url: &str) -> RequestBuilder {
    match method {
        Method::Get => Request::get(url),
        Method::Post => Request::post(url),
        Method::Put => Request::put(url),
        Method::Delete => Request::delete(url),
    }
}

fn authed(method: Method, url: &str) -> RequestBuilder {
    let req = request(method, url);
    match bearer() {
        Some(b) => req.header("Authorization", &b),
        None => req,
    }
}

fn authed_required(method: Method, url: &str) -> Result<RequestBuilder, String> {
    let Some(b) = bearer() else {
        return Err("Not logged in".into());
    };
    Ok(request(method, url).header("Authorization", &b))
}

fn json_body<T: Serialize>(req: RequestBuilder, body: &T) -> Result<Request, String> {
    req.json(body).map_err(|e| format!("{e:?}"))
}

async fn send(req: RequestBuilder) -> Result<Response, String> {
    req.send().await.map_err(|e| e.to_string())
}

async fn send_request(req: Request) -> Result<Response, String> {
    req.send().await.map_err(|e| e.to_string())
}

async fn read_json<T: DeserializeOwned>(resp: Response) -> Result<T, String> {
    resp.json::<T>().await.map_err(|e| e.to_string())
}

async fn send_json<T: DeserializeOwned>(
    req: RequestBuilder,
    failure: FailureText,
) -> Result<T, String> {
    let resp = send(req).await?;
    if resp.ok() {
        read_json(resp).await
    } else {
        Err(failure.message_for(resp).await)
    }
}

async fn send_request_json<T: DeserializeOwned>(
    req: Request,
    failure: FailureText,
) -> Result<T, String> {
    let resp = send_request(req).await?;
    if resp.ok() {
        read_json(resp).await
    } else {
        Err(failure.message_for(resp).await)
    }
}

async fn send_empty(req: RequestBuilder, failure: FailureText) -> Result<(), String> {
    let resp = send(req).await?;
    if resp.ok() {
        Ok(())
    } else {
        Err(failure.message_for(resp).await)
    }
}

async fn send_request_empty(req: Request, failure: FailureText) -> Result<(), String> {
    let resp = send_request(req).await?;
    if resp.ok() {
        Ok(())
    } else {
        Err(failure.message_for(resp).await)
    }
}

// ── OTVI app-level auth ──────────────────────────────────────────────────────

/// Outcome of the single boot-check request (`GET /api/auth/me`).
#[derive(Clone)]
pub enum AppBoot {
    /// Valid JWT found – ready to use the app.
    Ready(UserInfo),
    /// No JWT / expired – show the login page.
    NeedsLogin,
    /// No users in the database – show the admin-creation wizard.
    NeedsSetup,
}

#[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
#[derive(Clone, Default)]
pub struct UiTestMockState {
    pub boot: Option<AppBoot>,
    pub providers: Option<Result<Vec<ProviderInfo>, String>>,
    pub provider: Option<Result<ProviderInfo, String>>,
    pub admin_users: Option<Result<Vec<UserInfo>, String>>,
    pub settings: Option<Result<ServerSettings, String>>,
    pub provider_session_valid: Option<bool>,
    pub channels: Option<Result<ChannelListResponse, String>>,
    pub categories: Option<Result<CategoryListResponse, String>>,
    pub stream: Option<Result<StreamInfo, String>>,
}

#[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
mod ui_test_mock {
    use super::UiTestMockState;
    use std::cell::RefCell;

    thread_local! {
        static MOCK_STATE: RefCell<Option<UiTestMockState>> = const { RefCell::new(None) };
    }

    pub fn set(state: UiTestMockState) {
        MOCK_STATE.with(|cell| *cell.borrow_mut() = Some(state));
    }

    pub fn clear() {
        MOCK_STATE.with(|cell| *cell.borrow_mut() = None);
    }

    pub fn read<T>(f: impl FnOnce(&UiTestMockState) -> Option<T>) -> Option<T> {
        MOCK_STATE.with(|cell| cell.borrow().as_ref().and_then(f))
    }
}

#[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
pub fn set_ui_test_mock_state(state: UiTestMockState) {
    ui_test_mock::set(state);
}

#[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
pub fn clear_ui_test_mock_state() {
    ui_test_mock::clear();
}

/// Called once on app startup.  Calls `GET /api/auth/me` with whatever token
/// is in localStorage.  Maps the three possible outcomes:
/// • 200 OK           → `Ready(UserInfo)`
/// • 401 Unauthorized → `NeedsLogin`  (token missing / expired, users exist)
/// • 403 Forbidden    → `NeedsSetup`  (no users in DB yet)
pub async fn boot_check() -> AppBoot {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.boot.clone()) {
        return mocked;
    }

    let Ok(resp) = send(authed(Method::Get, "/api/auth/me")).await else {
        return AppBoot::NeedsLogin;
    };
    match resp.status() {
        200 => read_json::<UserInfo>(resp)
            .await
            .map(AppBoot::Ready)
            .unwrap_or(AppBoot::NeedsLogin),
        403 => AppBoot::NeedsSetup,
        _ => AppBoot::NeedsLogin,
    }
}

pub async fn app_login(username: &str, password: &str) -> Result<AppLoginResponse, String> {
    let req = json_body(
        request(Method::Post, "/api/auth/login"),
        &AppLoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )?;

    send_request_json(req, FailureText::BodyOr("Login failed")).await
}

pub async fn app_register(username: &str, password: &str) -> Result<AppLoginResponse, String> {
    let req = json_body(
        request(Method::Post, "/api/auth/register"),
        &RegisterRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )?;

    send_request_json(req, FailureText::BodyOr("Registration failed")).await
}

pub async fn change_password(
    current_password: &str,
    new_password: &str,
) -> Result<AppLoginResponse, String> {
    let req = json_body(
        authed(Method::Post, "/api/auth/change-password"),
        &ChangePasswordRequest {
            current_password: current_password.to_string(),
            new_password: new_password.to_string(),
        },
    )?;

    send_request_json(req, FailureText::BodyOr("Password change failed")).await
}

// ── Provider endpoints ──────────────────────────────────────────────────────

pub async fn fetch_providers() -> Result<Vec<ProviderInfo>, String> {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.providers.clone()) {
        return mocked;
    }

    send_json(
        authed(Method::Get, "/api/providers"),
        FailureText::UnauthorizedOrStatus,
    )
    .await
}

pub async fn fetch_provider(id: &str) -> Result<ProviderInfo, String> {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.provider.clone()) {
        return mocked;
    }

    send_json(
        authed(Method::Get, &format!("/api/providers/{id}")),
        FailureText::Status,
    )
    .await
}

// ── Provider-level auth (TV provider sessions) ───────────────────────────────

/// Check whether the current user already has an authenticated provider session.
pub async fn check_provider_session(provider_id: &str) -> bool {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.provider_session_valid) {
        return mocked;
    }

    let Ok(req) = authed_required(
        Method::Get,
        &format!("/api/providers/{provider_id}/auth/check"),
    ) else {
        return false;
    };
    let Ok(resp) = send(req).await else {
        return false;
    };
    if !resp.ok() {
        return false;
    }
    #[derive(serde::Deserialize)]
    struct CheckResp {
        valid: bool,
    }
    resp.json::<CheckResp>()
        .await
        .map(|r| r.valid)
        .unwrap_or(false)
}

pub async fn login(provider_id: &str, req: &LoginRequest) -> Result<LoginResponse, String> {
    let url = format!("/api/providers/{provider_id}/auth/login");
    let req = json_body(authed(Method::Post, &url), req)?;
    send_request_json(req, FailureText::BodyOrStatus).await
}

pub async fn provider_logout(provider_id: &str) -> Result<(), String> {
    let req = authed_required(
        Method::Post,
        &format!("/api/providers/{provider_id}/auth/logout"),
    )?;
    send(req).await?;
    Ok(())
}

// ── Admin: users ─────────────────────────────────────────────────────────────

pub async fn admin_list_users() -> Result<Vec<UserInfo>, String> {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.admin_users.clone()) {
        return mocked;
    }

    send_json(authed(Method::Get, "/api/admin/users"), FailureText::Status).await
}

pub async fn admin_create_user(req: CreateUserRequest) -> Result<UserInfo, String> {
    let req = json_body(authed(Method::Post, "/api/admin/users"), &req)?;
    send_request_json(req, FailureText::BodyOrStatus).await
}

pub async fn admin_delete_user(user_id: &str) -> Result<(), String> {
    send_empty(
        authed(Method::Delete, &format!("/api/admin/users/{user_id}")),
        FailureText::BodyOrStatus,
    )
    .await
}

pub async fn admin_set_user_providers(user_id: &str, providers: Vec<String>) -> Result<(), String> {
    let req = json_body(
        authed(
            Method::Put,
            &format!("/api/admin/users/{user_id}/providers"),
        ),
        &UpdateUserProvidersRequest { providers },
    )?;
    send_request_empty(req, FailureText::BodyOrStatus).await
}

pub async fn admin_reset_password(user_id: &str, new_password: &str) -> Result<(), String> {
    let req = json_body(
        authed(Method::Put, &format!("/api/admin/users/{user_id}/password")),
        &AdminResetPasswordRequest {
            new_password: new_password.to_string(),
        },
    )?;
    send_request_empty(req, FailureText::BodyOrStatus).await
}

// ── Admin: settings ───────────────────────────────────────────────────────────

pub async fn admin_get_settings() -> Result<ServerSettings, String> {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.settings.clone()) {
        return mocked;
    }

    send_json(
        authed(Method::Get, "/api/admin/settings"),
        FailureText::Status,
    )
    .await
}

pub async fn admin_update_settings(settings: ServerSettings) -> Result<(), String> {
    let req = json_body(authed(Method::Put, "/api/admin/settings"), &settings)?;
    send_request_empty(req, FailureText::BodyOrStatus).await
}

// ── Channel endpoints ───────────────────────────────────────────────────────

pub async fn fetch_channels(
    provider_id: &str,
    params: &HashMap<String, String>,
) -> Result<ChannelListResponse, String> {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.channels.clone()) {
        return mocked;
    }

    let req = authed_required(
        Method::Get,
        &format!("/api/providers/{provider_id}/channels"),
    )?
    .query(params.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    send_json(req, FailureText::Status).await
}

pub async fn fetch_categories(provider_id: &str) -> Result<CategoryListResponse, String> {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.categories.clone()) {
        return mocked;
    }

    let req = authed_required(
        Method::Get,
        &format!("/api/providers/{provider_id}/channels/categories"),
    )?;
    send_json(req, FailureText::Status).await
}

// ── Playback endpoints ──────────────────────────────────────────────────────

pub async fn fetch_stream(provider_id: &str, channel_id: &str) -> Result<StreamInfo, String> {
    #[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
    if let Some(mocked) = ui_test_mock::read(|state| state.stream.clone()) {
        return mocked;
    }

    let req = authed_required(
        Method::Get,
        &format!("/api/providers/{provider_id}/channels/{channel_id}/stream"),
    )?;
    send_json(req, FailureText::Status).await
}
