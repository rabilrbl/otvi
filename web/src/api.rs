//! HTTP client for communicating with the OTVI backend API from WASM.

use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use otvi_core::types::*;
use std::collections::HashMap;

const SESSION_PREFIX: &str = "otvi_session_";

// ── Session management (localStorage) ───────────────────────────────────────

pub fn store_session(provider_id: &str, session_id: &str) {
    let key = format!("{SESSION_PREFIX}{provider_id}");
    let _ = LocalStorage::set(&key, session_id);
}

pub fn get_session(provider_id: &str) -> Option<String> {
    let key = format!("{SESSION_PREFIX}{provider_id}");
    LocalStorage::get::<String>(&key).ok()
}

pub fn clear_session(provider_id: &str) {
    let key = format!("{SESSION_PREFIX}{provider_id}");
    LocalStorage::delete(&key);
}

// ── Provider endpoints ──────────────────────────────────────────────────────

pub async fn fetch_providers() -> Result<Vec<ProviderInfo>, String> {
    let resp = Request::get("/api/providers")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    resp.json::<Vec<ProviderInfo>>()
        .await
        .map_err(|e| e.to_string())
}

pub async fn fetch_provider(id: &str) -> Result<ProviderInfo, String> {
    let resp = Request::get(&format!("/api/providers/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    resp.json::<ProviderInfo>()
        .await
        .map_err(|e| e.to_string())
}

// ── Auth endpoints ──────────────────────────────────────────────────────────

/// Check if the stored session for a provider is still valid on the server.
/// Returns `true` if valid, `false` if invalid/expired (also clears local storage).
pub async fn check_session(provider_id: &str) -> bool {
    let session = match get_session(provider_id) {
        Some(s) => s,
        None => return false,
    };

    let resp = Request::get(&format!("/api/providers/{provider_id}/auth/check"))
        .header("X-Session-Token", &session)
        .send()
        .await;

    match resp {
        Ok(r) if r.ok() => true,
        _ => {
            // Session is invalid on the server — clean up local storage
            clear_session(provider_id);
            false
        }
    }
}

pub async fn login(provider_id: &str, req: &LoginRequest) -> Result<LoginResponse, String> {
    let body = serde_json::to_string(req).map_err(|e| e.to_string())?;

    let resp = Request::post(&format!("/api/providers/{provider_id}/auth/login"))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("{e:?}"))?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    resp.json::<LoginResponse>()
        .await
        .map_err(|e| e.to_string())
}

pub async fn logout(provider_id: &str) -> Result<(), String> {
    let session = get_session(provider_id).ok_or("Not logged in")?;

    let _ = Request::post(&format!("/api/providers/{provider_id}/auth/logout"))
        .header("X-Session-Token", &session)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    clear_session(provider_id);
    Ok(())
}

// ── Channel endpoints ───────────────────────────────────────────────────────

pub async fn fetch_channels(
    provider_id: &str,
    params: &HashMap<String, String>,
) -> Result<ChannelListResponse, String> {
    let session = get_session(provider_id).ok_or("Not logged in")?;

    let mut url = format!("/api/providers/{provider_id}/channels");
    if !params.is_empty() {
        let qs: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        url = format!("{url}?{}", qs.join("&"));
    }

    let resp = Request::get(&url)
        .header("X-Session-Token", &session)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    resp.json::<ChannelListResponse>()
        .await
        .map_err(|e| e.to_string())
}

pub async fn fetch_categories(provider_id: &str) -> Result<CategoryListResponse, String> {
    let session = get_session(provider_id).ok_or("Not logged in")?;

    let resp = Request::get(&format!(
        "/api/providers/{provider_id}/channels/categories"
    ))
    .header("X-Session-Token", &session)
    .send()
    .await
    .map_err(|e| e.to_string())?;

    resp.json::<CategoryListResponse>()
        .await
        .map_err(|e| e.to_string())
}

// ── Playback endpoints ─────────────────────────────────────────────────────

pub async fn fetch_stream(
    provider_id: &str,
    channel_id: &str,
) -> Result<StreamInfo, String> {
    let session = get_session(provider_id).ok_or("Not logged in")?;

    let resp = Request::get(&format!(
        "/api/providers/{provider_id}/channels/{channel_id}/stream"
    ))
    .header("X-Session-Token", &session)
    .send()
    .await
    .map_err(|e| e.to_string())?;

    resp.json::<StreamInfo>()
        .await
        .map_err(|e| e.to_string())
}
