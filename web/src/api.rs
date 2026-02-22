//! HTTP client for communicating with the OTVI backend API from WASM.

use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use otvi_core::types::*;
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

// ── OTVI app-level auth ──────────────────────────────────────────────────────

/// Outcome of the single boot-check request (`GET /api/auth/me`).
pub enum AppBoot {
    /// Valid JWT found – ready to use the app.
    Ready(UserInfo),
    /// No JWT / expired – show the login page.
    NeedsLogin,
    /// No users in the database – show the admin-creation wizard.
    NeedsSetup,
}

/// Called once on app startup.  Calls `GET /api/auth/me` with whatever token
/// is in localStorage.  Maps the three possible outcomes:
/// • 200 OK           → `Ready(UserInfo)`
/// • 401 Unauthorized → `NeedsLogin`  (token missing / expired, users exist)
/// • 403 Forbidden    → `NeedsSetup`  (no users in DB yet)
pub async fn boot_check() -> AppBoot {
    let req = match bearer() {
        Some(b) => Request::get("/api/auth/me").header("Authorization", &b),
        None => Request::get("/api/auth/me"),
    };
    let Ok(resp) = req.send().await else {
        return AppBoot::NeedsLogin;
    };
    match resp.status() {
        200 => resp
            .json::<UserInfo>()
            .await
            .map(AppBoot::Ready)
            .unwrap_or(AppBoot::NeedsLogin),
        403 => AppBoot::NeedsSetup,
        _ => AppBoot::NeedsLogin,
    }
}

pub async fn app_login(username: &str, password: &str) -> Result<AppLoginResponse, String> {
    let body = serde_json::to_string(&AppLoginRequest {
        username: username.to_string(),
        password: password.to_string(),
    })
    .map_err(|e| e.to_string())?;

    let resp = Request::post("/api/auth/login")
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.ok() {
        resp.json::<AppLoginResponse>().await.map_err(|e| e.to_string())
    } else {
        Err(resp.text().await.unwrap_or_else(|_| "Login failed".into()))
    }
}

pub async fn app_register(username: &str, password: &str) -> Result<AppLoginResponse, String> {
    let body = serde_json::to_string(&RegisterRequest {
        username: username.to_string(),
        password: password.to_string(),
    })
    .map_err(|e| e.to_string())?;

    let resp = Request::post("/api/auth/register")
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.ok() {
        resp.json::<AppLoginResponse>().await.map_err(|e| e.to_string())
    } else {
        Err(resp.text().await.unwrap_or_else(|_| "Registration failed".into()))
    }
}

pub async fn change_password(
    current_password: &str,
    new_password: &str,
) -> Result<AppLoginResponse, String> {
    let body = serde_json::to_string(&ChangePasswordRequest {
        current_password: current_password.to_string(),
        new_password: new_password.to_string(),
    })
    .map_err(|e| e.to_string())?;

    let resp = post_authed("/api/auth/change-password")
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.ok() {
        resp.json::<AppLoginResponse>().await.map_err(|e| e.to_string())
    } else {
        Err(resp.text().await.unwrap_or_else(|_| "Password change failed".into()))
    }
}

// ── Provider endpoints ──────────────────────────────────────────────────────

/// Helper: build a GET request with Authorization header if we have a token.
fn get_authed(url: &str) -> gloo_net::http::RequestBuilder {
    let req = Request::get(url);
    match bearer() {
        Some(b) => req.header("Authorization", &b),
        None => req,
    }
}

fn post_authed(url: &str) -> gloo_net::http::RequestBuilder {
    let req = Request::post(url);
    match bearer() {
        Some(b) => req.header("Authorization", &b),
        None => req,
    }
}

pub async fn fetch_providers() -> Result<Vec<ProviderInfo>, String> {
    let resp = get_authed("/api/providers")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        if resp.status() == 401 {
            return Err("__unauthorized__".into());
        }
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Vec<ProviderInfo>>().await.map_err(|e| e.to_string())
}

pub async fn fetch_provider(id: &str) -> Result<ProviderInfo, String> {
    let resp = get_authed(&format!("/api/providers/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<ProviderInfo>().await.map_err(|e| e.to_string())
}

// ── Provider-level auth (TV provider sessions) ───────────────────────────────

/// Check whether the current user already has an authenticated provider session.
pub async fn check_provider_session(provider_id: &str) -> bool {
    let Some(b) = bearer() else { return false };
    let Ok(resp) = Request::get(&format!("/api/providers/{provider_id}/auth/check"))
        .header("Authorization", &b)
        .send()
        .await
    else {
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
    let body = serde_json::to_string(req).map_err(|e| e.to_string())?;
    let resp = post_authed(&format!("/api/providers/{provider_id}/auth/login"))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(resp.text().await.unwrap_or_else(|_| format!("HTTP {}", resp.status())));
    }
    resp.json::<LoginResponse>().await.map_err(|e| e.to_string())
}

pub async fn provider_logout(provider_id: &str) -> Result<(), String> {
    let Some(b) = bearer() else { return Err("Not logged in".into()) };
    Request::post(&format!("/api/providers/{provider_id}/auth/logout"))
        .header("Authorization", &b)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Admin helpers ────────────────────────────────────────────────────────────

fn put_authed(url: &str) -> gloo_net::http::RequestBuilder {
    let req = gloo_net::http::Request::put(url);
    match bearer() {
        Some(b) => req.header("Authorization", &b),
        None => req,
    }
}

fn delete_authed(url: &str) -> gloo_net::http::RequestBuilder {
    let req = gloo_net::http::Request::delete(url);
    match bearer() {
        Some(b) => req.header("Authorization", &b),
        None => req,
    }
}

// ── Admin: users ─────────────────────────────────────────────────────────────

pub async fn admin_list_users() -> Result<Vec<UserInfo>, String> {
    let resp = get_authed("/api/admin/users")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Vec<UserInfo>>().await.map_err(|e| e.to_string())
}

pub async fn admin_create_user(req: CreateUserRequest) -> Result<UserInfo, String> {
    let body = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    let resp = post_authed("/api/admin/users")
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        resp.json::<UserInfo>().await.map_err(|e| e.to_string())
    } else {
        Err(resp.text().await.unwrap_or_else(|_| format!("HTTP {}", resp.status())))
    }
}

pub async fn admin_delete_user(user_id: &str) -> Result<(), String> {
    let resp = delete_authed(&format!("/api/admin/users/{user_id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(resp.text().await.unwrap_or_else(|_| format!("HTTP {}", resp.status())))
    }
}

pub async fn admin_set_user_providers(user_id: &str, providers: Vec<String>) -> Result<(), String> {
    let body = serde_json::to_string(&UpdateUserProvidersRequest { providers })
        .map_err(|e| e.to_string())?;
    let resp = put_authed(&format!("/api/admin/users/{user_id}/providers"))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(resp.text().await.unwrap_or_else(|_| format!("HTTP {}", resp.status())))
    }
}

pub async fn admin_reset_password(user_id: &str, new_password: &str) -> Result<(), String> {
    let body = serde_json::to_string(&AdminResetPasswordRequest {
        new_password: new_password.to_string(),
    })
    .map_err(|e| e.to_string())?;
    let resp = put_authed(&format!("/api/admin/users/{user_id}/password"))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(resp.text().await.unwrap_or_else(|_| format!("HTTP {}", resp.status())))
    }
}

// ── Admin: settings ───────────────────────────────────────────────────────────

pub async fn admin_get_settings() -> Result<ServerSettings, String> {
    let resp = get_authed("/api/admin/settings")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<ServerSettings>().await.map_err(|e| e.to_string())
}

pub async fn admin_update_settings(settings: ServerSettings) -> Result<(), String> {
    let body = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    let resp = put_authed("/api/admin/settings")
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(resp.text().await.unwrap_or_else(|_| format!("HTTP {}", resp.status())))
    }
}

// ── Channel endpoints ───────────────────────────────────────────────────────

pub async fn fetch_channels(
    provider_id: &str,
    params: &HashMap<String, String>,
) -> Result<ChannelListResponse, String> {
    let Some(b) = bearer() else { return Err("Not logged in".into()) };
    let mut url = format!("/api/providers/{provider_id}/channels");
    if !params.is_empty() {
        let qs: Vec<String> = params.iter().map(|(k, v)| format!("{k}={v}")).collect();
        url = format!("{url}?{}", qs.join("&"));
    }
    let resp = Request::get(&url)
        .header("Authorization", &b)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<ChannelListResponse>().await.map_err(|e| e.to_string())
}

pub async fn fetch_categories(provider_id: &str) -> Result<CategoryListResponse, String> {
    let Some(b) = bearer() else { return Err("Not logged in".into()) };
    let resp = Request::get(&format!("/api/providers/{provider_id}/channels/categories"))
        .header("Authorization", &b)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CategoryListResponse>().await.map_err(|e| e.to_string())
}

// ── Playback endpoints ──────────────────────────────────────────────────────

pub async fn fetch_stream(provider_id: &str, channel_id: &str) -> Result<StreamInfo, String> {
    let Some(b) = bearer() else { return Err("Not logged in".into()) };
    let resp = Request::get(&format!("/api/providers/{provider_id}/channels/{channel_id}/stream"))
        .header("Authorization", &b)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<StreamInfo>().await.map_err(|e| e.to_string())
}