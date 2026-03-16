//! Provider-level authentication.
//!
//! ## Cache invalidation
//!
//! The channel-list and category caches are keyed by `(provider_id, session_uid)`.
//! Whenever a provider session is created (login completes) or destroyed (logout),
//! the corresponding cache entries are evicted so that the next listing always
//! reflects the updated credentials.
//!
//! Endpoints let OTVI users authenticate with a TV provider via the configured
//! multi-step flow.  Each step advances the provider's auth process (e.g.
//! send OTP → verify OTP).
//!
//! # Scope
//!
//! The provider YAML contains an `auth.scope` field:
//!
//! * `global`   – Only an **admin** can log in / log out.  A single shared
//!   provider session is used for all OTVI users of that provider.
//! * `per_user` – Every OTVI user manages their own provider credentials.
//!
//! All endpoints require a valid OTVI JWT (`Authorization: Bearer …`).

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use chrono::Utc;
use uuid::Uuid;

use otvi_core::config::{AuthFlow, AuthScope};
use otvi_core::template::{TemplateContext, extract_json_path};
use otvi_core::types::*;

use crate::api::provider_access::authorize_provider_route;
use crate::auth_middleware::ActiveClaims;
use crate::auth_middleware::Claims;
use crate::db;
use crate::error::AppError;
use crate::provider_client;
use crate::state::{AppState, ChannelCacheKey};

// ── Apply input transforms (base64, …) ────────────────────────────────────

fn apply_transforms(flow: &AuthFlow, inputs: &HashMap<String, String>) -> HashMap<String, String> {
    let mut result = inputs.clone();
    for field in &flow.inputs {
        if let Some(transform) = &field.transform
            && let Some(val) = inputs.get(&field.key)
        {
            let transformed = match transform.as_str() {
                "base64" => {
                    use base64::Engine;
                    base64::engine::general_purpose::STANDARD.encode(val.as_bytes())
                }
                _ => continue,
            };
            result.insert(format!("{}_{}", field.key, transform), transformed);
        }
    }
    result
}

// ── Provider-session user ID ───────────────────────────────────────────────

/// Returns the `user_id` key used in `provider_sessions`.
/// For `global`-scoped providers the key is `""` (shared across all users).
fn session_user_id(scope: &AuthScope, claims: &Claims) -> String {
    match scope {
        AuthScope::Global => String::new(),
        AuthScope::PerUser => claims.sub.clone(),
    }
}

// ── Login ──────────────────────────────────────────────────────────────────

/// `POST /api/providers/:id/auth/login`
///
/// Advance the provider auth flow by one step.
/// For `global`-scoped providers, only admins may call this endpoint.
#[utoipa::path(
    post,
    path = "/api/providers/{id}/auth/login",
    tag = "providers",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "Provider ID"),
    ),
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Step result; `success: true` when the final step completes", body = LoginResponse),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Password change required, or non-admin accessing a global-scoped provider"),
        (status = 404, description = "Provider or auth flow not found"),
    ),
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    claims: ActiveClaims,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let scope = authorize_provider_route(&state, &claims, &provider_id, true).await?;

    // Extract everything we need from the provider config while holding the
    // read lock for the shortest possible time, then drop the guard.
    let provider_data = state
        .with_provider(&provider_id, |p| {
            let flow = p.auth.flows.iter().find(|f| f.id == req.flow_id).cloned();
            let base_url = p.defaults.base_url.clone();
            let default_headers = p.defaults.headers.clone();
            (flow, base_url, default_headers)
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (maybe_flow, base_url, default_headers) = provider_data;

    let flow = maybe_flow.ok_or_else(|| AppError::NotFound("Auth flow not found".into()))?;

    let step = flow
        .steps
        .get(req.step)
        .cloned()
        .ok_or_else(|| AppError::BadRequest("Invalid step index".into()))?;

    let total_steps = flow.steps.len();
    let transformed_inputs = apply_transforms(&flow, &req.inputs);

    // ── Build template context ────────────────────────────────────────────
    let uid = session_user_id(&scope, &claims);
    let stored = db::get_provider_session_values(&state.db, &uid, &provider_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut context = TemplateContext::new();
    for (k, v) in &transformed_inputs {
        context.set(format!("input.{k}"), v.clone());
    }
    for (k, v) in &stored {
        context.set(format!("stored.{k}"), v.clone());
        context.set(k.clone(), v.clone());
    }
    context.set("uuid", Uuid::new_v4().to_string());
    let now = Utc::now();
    context.set("utcnow", now.format("%Y%m%dT%H%M%S").to_string());
    context.set("utcdate", now.format("%Y%m%d").to_string());
    let device_id = stored
        .get("device_id")
        .cloned()
        .unwrap_or_else(|| format!("{:016x}", rand::random::<u64>()));
    context.set("device_id", &device_id);

    // ── Execute provider request ──────────────────────────────────────────
    let response = provider_client::execute_request(
        &state.http_client,
        &base_url,
        &default_headers,
        &step.request,
        &context,
    )
    .await;

    match response {
        Ok(provider_resp) => {
            if let Some(expected_status) = step.success_status
                && provider_resp.status != expected_status
            {
                return Ok(Json(LoginResponse {
                    success: false,
                    session_id: None,
                    next_step: None,
                    user_name: None,
                    error: Some(format!(
                        "Provider returned unexpected status {} (expected {})",
                        provider_resp.status, expected_status
                    )),
                }));
            }

            let json_body = provider_resp.body;

            let mut extracted: HashMap<String, String> = HashMap::new();
            if let Some(on_success) = &step.on_success {
                for (key, path) in &on_success.extract {
                    if let Some(value) = extract_json_path(&json_body, path) {
                        extracted.insert(key.clone(), value);
                    }
                }
            }
            extracted
                .entry("device_id".to_string())
                .or_insert_with(|| device_id.clone());
            for (cookie_name, cookie_value) in provider_resp.cookies {
                // Persist cookies under "__cookie_.{name}" so they appear as
                // "stored.__cookie_.{name}" in the template context, matching
                // the prefix expected by provider_client.
                extracted.insert(format!("__cookie_.{cookie_name}"), cookie_value);
            }

            let mut new_stored = stored.clone();
            new_stored.extend(extracted);

            db::upsert_provider_session(&state.db, &uid, &provider_id, &new_stored)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            // Invalidate the channel/category cache for this provider + uid so
            // the next listing fetches fresh data with the new credentials.
            let cache_key = ChannelCacheKey::from_auth_scope(&provider_id, &scope, &uid);
            state.channel_cache.invalidate(&cache_key).await;

            let has_prompt = step
                .on_success
                .as_ref()
                .and_then(|s| s.prompt.as_ref())
                .is_some();

            if has_prompt {
                let prompt_fields = step.on_success.as_ref().unwrap().prompt.as_ref().unwrap();

                let next_step_index = req.step + 1;
                let next_step_name = flow
                    .steps
                    .get(next_step_index)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();

                Ok(Json(LoginResponse {
                    success: false,
                    session_id: None,
                    next_step: Some(NextStepInfo {
                        step_index: next_step_index,
                        step_name: next_step_name,
                        fields: prompt_fields
                            .iter()
                            .map(|f| FieldInfo {
                                key: f.key.clone(),
                                label: f.label.clone(),
                                field_type: f.field_type.clone(),
                                required: f.required,
                            })
                            .collect(),
                    }),
                    user_name: None,
                    error: None,
                }))
            } else if req.step + 1 < total_steps {
                Ok(Json(LoginResponse {
                    success: false,
                    session_id: None,
                    next_step: None,
                    user_name: None,
                    error: None,
                }))
            } else {
                let user_name = new_stored.get("user_name").cloned();
                Ok(Json(LoginResponse {
                    success: true,
                    session_id: None,
                    next_step: None,
                    user_name,
                    error: None,
                }))
            }
        }
        Err(e) => Ok(Json(LoginResponse {
            success: false,
            session_id: None,
            next_step: None,
            user_name: None,
            error: Some(e.to_string()),
        })),
    }
}

// ── Check session ──────────────────────────────────────────────────────────

/// `GET /api/providers/:id/auth/check`
#[utoipa::path(
    get,
    path = "/api/providers/{id}/auth/check",
    tag = "providers",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "Provider ID"),
    ),
    responses(
        (status = 200, description = "`{ \"valid\": true }` when an active session exists, `false` otherwise"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Password change required"),
        (status = 404, description = "Provider not found"),
    ),
)]
pub async fn check_session(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    claims: ActiveClaims,
) -> Result<Json<serde_json::Value>, AppError> {
    let scope = authorize_provider_route(&state, &claims, &provider_id, false).await?;

    let uid = session_user_id(&scope, &claims);
    let stored = db::get_provider_session_values(&state.db, &uid, &provider_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let valid = !stored.is_empty();
    Ok(Json(serde_json::json!({ "valid": valid })))
}

// ── Logout ─────────────────────────────────────────────────────────────────

/// `POST /api/providers/:id/auth/logout`
///
/// For `global`-scoped providers, only admins may call this endpoint.
#[utoipa::path(
    post,
    path = "/api/providers/{id}/auth/logout",
    tag = "providers",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "Provider ID"),
    ),
    responses(
        (status = 200, description = "Provider session cleared"),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Password change required, or non-admin accessing a global-scoped provider"),
        (status = 404, description = "Provider not found"),
    ),
)]
pub async fn logout(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    claims: ActiveClaims,
) -> Result<Json<serde_json::Value>, AppError> {
    let scope = authorize_provider_route(&state, &claims, &provider_id, true).await?;

    let provider_data = state
        .with_provider(&provider_id, |p| {
            (
                p.auth.logout.clone(),
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (logout_cfg, base_url, default_headers) = provider_data;

    let uid = session_user_id(&scope, &claims);

    if let Some(logout_req) = logout_cfg {
        let context = build_provider_context(&state, &uid, &provider_id).await?;
        let _ = provider_client::execute_request_body(
            &state.http_client,
            &base_url,
            &default_headers,
            &logout_req.request,
            &context,
        )
        .await;
    }

    db::delete_provider_session(&state.db, &uid, &provider_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Evict any cached channel/category data for this provider + uid.
    // After logout the upstream credentials are gone, so cached data from the
    // authenticated session must not be served to future (unauthenticated) calls.
    let cache_key = ChannelCacheKey::from_auth_scope(&provider_id, &scope, &uid);
    state.channel_cache.invalidate(&cache_key).await;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Build a [`TemplateContext`] populated with the stored provider-session
/// values for `(user_id, provider_id)`.
pub async fn build_provider_context(
    state: &AppState,
    user_id: &str,
    provider_id: &str,
) -> Result<TemplateContext, AppError> {
    let stored = db::get_provider_session_values(&state.db, user_id, provider_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut context = TemplateContext::new();
    for (k, v) in &stored {
        context.set(format!("stored.{k}"), v.clone());
    }
    context.set("uuid", uuid::Uuid::new_v4().to_string());
    let now = chrono::Utc::now();
    context.set("utcnow", now.format("%Y%m%dT%H%M%S").to_string());
    context.set("utcdate", now.format("%Y%m%d").to_string());
    Ok(context)
}

// ── Token Refresh ─────────────────────────────────────────────────────────

/// Execute the provider's refresh flow: send the refresh HTTP request,
/// extract new token values, merge into existing stored values, and persist.
///
/// Returns `Ok(())` on success.  Returns `Err` if the provider has no
/// refresh config, or if the refresh request fails, or if no values are
/// extracted.
pub async fn execute_refresh(
    state: &AppState,
    provider_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    // Load refresh config + defaults from provider YAML.
    let provider_data = state
        .with_provider(provider_id, |p| {
            (
                p.auth.refresh.clone(),
                p.auth.scope.clone(),
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (refresh_cfg, auth_scope, base_url, default_headers) = provider_data;
    let refresh =
        refresh_cfg.ok_or_else(|| AppError::Internal("Provider has no refresh config".into()))?;

    // Build template context from current stored values.
    let context = build_provider_context(state, user_id, provider_id).await?;

    // Execute the refresh request.
    let response = provider_client::execute_request(
        &state.http_client,
        &base_url,
        &default_headers,
        &refresh.request,
        &context,
    )
    .await
    .map_err(|e| {
        tracing::warn!(
            provider = %provider_id,
            user = %user_id,
            "Token refresh request failed: {e}"
        );
        AppError::Internal(format!("Token refresh failed: {e}"))
    })?;

    // Extract new values from refresh response.
    let mut extracted: HashMap<String, String> = HashMap::new();
    for (key, path) in &refresh.on_success.extract {
        if let Some(value) = extract_json_path(&response.body, path) {
            extracted.insert(key.clone(), value);
        }
    }

    if extracted.is_empty() {
        tracing::warn!(
            provider = %provider_id,
            user = %user_id,
            "Token refresh succeeded but extracted no values"
        );
        return Err(AppError::Internal(
            "Token refresh extracted no values".into(),
        ));
    }

    // Merge extracted values into existing stored session (only overwrite
    // keys present in the extraction map).
    let mut stored = db::get_provider_session_values(&state.db, user_id, provider_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    stored.extend(extracted);

    // Persist cookies from the refresh response.
    for (cookie_name, cookie_value) in &response.cookies {
        stored.insert(format!("__cookie_.{cookie_name}"), cookie_value.clone());
    }

    db::upsert_provider_session(&state.db, user_id, provider_id, &stored)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        provider = %provider_id,
        user = %user_id,
        "Token refresh succeeded — stored values updated"
    );

    // Invalidate channel/category cache for this session.
    let cache_key = ChannelCacheKey::from_auth_scope(provider_id, &auth_scope, user_id);
    state.channel_cache.invalidate(&cache_key).await;

    // Best-effort ProxyContext cleanup: moka's future::Cache does not
    // support selective invalidation by value, so stale ProxyContext entries
    // will expire naturally via TTL.  A new stream request after refresh
    // will create a fresh ProxyContext with updated tokens.
    tracing::debug!(
        provider = %provider_id,
        user = %user_id,
        "ProxyContext entries will expire via TTL; selective invalidation not supported"
    );

    Ok(())
}

/// Execute an upstream provider call and automatically refresh tokens on
/// failure.
///
/// `make_call` is an async closure that receives a [`TemplateContext`] and
/// returns a [`ProviderResponse`] (including non-2xx status codes).
///
/// If the first call returns a status code listed in the provider's
/// `refresh.on_status_codes` and a `RefreshConfig` is present, this
/// function:
///
/// 1. Acquires the per-session refresh lock (preventing concurrent
///    refreshes).
/// 2. Calls [`execute_refresh`] to update stored tokens.
/// 3. Rebuilds the template context with fresh tokens.
/// 4. Retries `make_call` exactly once.
///
/// The retry result is returned as-is — a second refresh-triggering status
/// does **not** trigger another refresh (preventing infinite loops).
///
/// Concurrent callers that block on the refresh lock will re-read fresh
/// tokens from the DB and retry without re-executing the refresh.
pub async fn with_refresh_retry<F, Fut>(
    state: &AppState,
    provider_id: &str,
    user_id: &str,
    make_call: F,
) -> Result<provider_client::ProviderResponse, AppError>
where
    F: Fn(TemplateContext) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<provider_client::ProviderResponse>>,
{
    // Load refresh config (if any) to know which status codes trigger refresh.
    let refresh_cfg = state
        .with_provider(provider_id, |p| p.auth.refresh.clone())
        .flatten();

    let context = build_provider_context(state, user_id, provider_id).await?;

    // First attempt.
    let response = make_call(context)
        .await
        .map_err(|e| AppError::Internal(format!("Upstream provider call failed: {e}")))?;

    // Check whether refresh should be triggered.
    let should_refresh = match &refresh_cfg {
        Some(cfg) => cfg.on_status_codes.contains(&response.status),
        None => false,
    };

    if !should_refresh {
        return Ok(response);
    }

    tracing::info!(
        provider = %provider_id,
        user = %user_id,
        status = response.status,
        "Upstream returned refresh-triggering status — attempting token refresh"
    );

    // Acquire per-session refresh lock.
    let lock = state.refresh_lock(provider_id, user_id);
    let _guard = lock.lock().await;

    // Perform refresh.
    if let Err(e) = execute_refresh(state, provider_id, user_id).await {
        tracing::warn!(
            provider = %provider_id,
            user = %user_id,
            error = ?e,
            "Token refresh failed, returning original upstream error"
        );
        return Ok(response);
    }

    // Rebuild context with fresh tokens and retry once.
    let fresh_context = build_provider_context(state, user_id, provider_id).await?;
    let retry_response = make_call(fresh_context).await.map_err(|e| {
        AppError::Internal(format!("Upstream provider call failed after refresh: {e}"))
    })?;

    Ok(retry_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_middleware::JwtKeys;
    use crate::db;
    use crate::state::{AppState, ChannelCache};

    use moka::future::Cache;
    use otvi_core::config::ProviderConfig;
    use std::collections::HashMap;
    use std::sync::{Mutex, RwLock};
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_keys() -> JwtKeys {
        JwtKeys::new(b"test-secret")
    }

    async fn test_db() -> (db::Db, tempfile::TempDir) {
        sqlx::any::install_default_drivers();
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}", db_path.display());
        let db = db::init(&url).await.expect("test db init");
        (db, dir)
    }

    /// Create a minimal `ProviderConfig` YAML string with a refresh block
    /// whose request points to `server_url + path`.
    fn provider_yaml_with_refresh(server_url: &str, extract: HashMap<String, String>) -> String {
        let extract_yaml: String = extract
            .iter()
            .map(|(k, v)| format!("        {k}: \"{v}\""))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"
provider:
  name: TestRefresh
  id: test_refresh
auth:
  flows:
    - id: email
      name: Email Login
      inputs:
        - key: email
          label: Email
      steps:
        - name: login
          request:
            method: POST
            path: /api/login
  refresh:
    request:
      method: POST
      path: "{server_url}/api/refresh"
    on_success:
      extract:
{extract_yaml}
channels:
  list:
    request:
      method: GET
      path: /api/channels
    response:
      items_path: "$.channels"
playback:
  stream:
    request:
      method: GET
      path: /api/play/{{{{input.id}}}}
    response:
      url: "$.url"
      type: "hls"
"#
        )
    }

    fn parse_provider(yaml: &str) -> ProviderConfig {
        serde_yaml_ng::from_str(yaml).expect("parse test provider YAML")
    }

    fn build_test_state(db: db::Db, providers: HashMap<String, ProviderConfig>) -> AppState {
        AppState {
            providers_rw: RwLock::new(providers),
            db,
            jwt_keys: test_keys(),
            http_client: reqwest::Client::new(),
            proxy_ctx: Cache::builder()
                .time_to_live(Duration::from_secs(60))
                .build(),
            channel_cache: ChannelCache::new(Duration::from_secs(60)),
            refresh_locks: Mutex::new(HashMap::new()),
        }
    }

    // ── execute_refresh tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn execute_refresh_updates_stored_values() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/refresh"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"authToken": "new-access-token"})),
            )
            .mount(&server)
            .await;

        let (test_db, _dir) = test_db().await;

        let mut extract = HashMap::new();
        extract.insert("access_token".into(), "$.authToken".into());
        let yaml = provider_yaml_with_refresh(&server.uri(), extract);
        let cfg = parse_provider(&yaml);

        let mut providers = HashMap::new();
        providers.insert("test_refresh".into(), cfg);
        let state = build_test_state(test_db, providers);

        // Seed initial stored values.
        let mut initial = HashMap::new();
        initial.insert("access_token".into(), "old-access-token".into());
        initial.insert("refresh_token".into(), "my-refresh-token".into());
        initial.insert("device_id".into(), "dev-123".into());
        db::upsert_provider_session(&state.db, "", "test_refresh", &initial)
            .await
            .unwrap();

        // Execute refresh.
        execute_refresh(&state, "test_refresh", "").await.unwrap();

        // Verify stored values were updated.
        let stored = db::get_provider_session_values(&state.db, "", "test_refresh")
            .await
            .unwrap();
        assert_eq!(stored.get("access_token").unwrap(), "new-access-token");
        // Non-extracted keys should be preserved.
        assert_eq!(stored.get("refresh_token").unwrap(), "my-refresh-token");
        assert_eq!(stored.get("device_id").unwrap(), "dev-123");
    }

    #[tokio::test]
    async fn execute_refresh_fails_on_upstream_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/refresh"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(serde_json::json!({"error": "server error"})),
            )
            .mount(&server)
            .await;

        let (test_db, _dir) = test_db().await;

        let mut extract = HashMap::new();
        extract.insert("access_token".into(), "$.authToken".into());
        let yaml = provider_yaml_with_refresh(&server.uri(), extract);
        let cfg = parse_provider(&yaml);

        let mut providers = HashMap::new();
        providers.insert("test_refresh".into(), cfg);
        let state = build_test_state(test_db, providers);

        // Seed initial stored values.
        let mut initial = HashMap::new();
        initial.insert("access_token".into(), "old-access-token".into());
        db::upsert_provider_session(&state.db, "", "test_refresh", &initial)
            .await
            .unwrap();

        // Execute refresh — should fail.
        let result = execute_refresh(&state, "test_refresh", "").await;
        assert!(result.is_err());

        // Stored values should not be modified.
        let stored = db::get_provider_session_values(&state.db, "", "test_refresh")
            .await
            .unwrap();
        assert_eq!(stored.get("access_token").unwrap(), "old-access-token");
    }

    #[tokio::test]
    async fn execute_refresh_fails_when_no_values_extracted() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/refresh"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"unrelated": "data"})),
            )
            .mount(&server)
            .await;

        let (test_db, _dir) = test_db().await;

        let mut extract = HashMap::new();
        extract.insert("access_token".into(), "$.authToken".into());
        let yaml = provider_yaml_with_refresh(&server.uri(), extract);
        let cfg = parse_provider(&yaml);

        let mut providers = HashMap::new();
        providers.insert("test_refresh".into(), cfg);
        let state = build_test_state(test_db, providers);

        let mut initial = HashMap::new();
        initial.insert("access_token".into(), "old-access-token".into());
        db::upsert_provider_session(&state.db, "", "test_refresh", &initial)
            .await
            .unwrap();

        // Execute refresh — should fail because extraction returns nothing.
        let result = execute_refresh(&state, "test_refresh", "").await;
        assert!(result.is_err());

        // Stored values should not be modified.
        let stored = db::get_provider_session_values(&state.db, "", "test_refresh")
            .await
            .unwrap();
        assert_eq!(stored.get("access_token").unwrap(), "old-access-token");
    }

    #[tokio::test]
    async fn execute_refresh_fails_for_provider_without_refresh_config() {
        let (test_db, _dir) = test_db().await;

        // Provider without refresh block.
        let yaml = r#"
provider:
  name: NoRefreshTV
  id: no_refresh
auth:
  flows:
    - id: email
      name: Email Login
      inputs:
        - key: email
          label: Email
      steps:
        - name: login
          request:
            method: POST
            path: /api/login
channels:
  list:
    request:
      method: GET
      path: /api/channels
    response:
      items_path: "$.channels"
playback:
  stream:
    request:
      method: GET
      path: /api/play/{{input.id}}
    response:
      url: "$.url"
      type: "hls"
"#;
        let cfg: ProviderConfig = serde_yaml_ng::from_str(yaml).unwrap();
        let mut providers = HashMap::new();
        providers.insert("no_refresh".into(), cfg);
        let state = build_test_state(test_db, providers);

        let result = execute_refresh(&state, "no_refresh", "").await;
        assert!(result.is_err());
    }

    // ── with_refresh_retry tests ──────────────────────────────────────────

    #[tokio::test]
    async fn with_refresh_retry_passes_through_on_success() {
        let (test_db, _dir) = test_db().await;

        let mut extract = HashMap::new();
        extract.insert("access_token".into(), "$.authToken".into());
        let yaml = provider_yaml_with_refresh("http://localhost:9999", extract);
        let cfg = parse_provider(&yaml);

        let mut providers = HashMap::new();
        providers.insert("test_refresh".into(), cfg);
        let state = build_test_state(test_db, providers);

        let result = with_refresh_retry(&state, "test_refresh", "", |_ctx| async {
            Ok(provider_client::ProviderResponse {
                status: 200,
                body: serde_json::json!({"ok": true}),
                cookies: HashMap::new(),
            })
        })
        .await
        .unwrap();

        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn with_refresh_retry_retries_after_refresh() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/refresh"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"authToken": "fresh-token"})),
            )
            .mount(&server)
            .await;

        let (test_db, _dir) = test_db().await;

        let mut extract = HashMap::new();
        extract.insert("access_token".into(), "$.authToken".into());
        let yaml = provider_yaml_with_refresh(&server.uri(), extract);
        let cfg = parse_provider(&yaml);

        let mut providers = HashMap::new();
        providers.insert("test_refresh".into(), cfg);
        let state = build_test_state(test_db, providers);

        // Seed stored values.
        let mut initial = HashMap::new();
        initial.insert("access_token".into(), "expired-token".into());
        db::upsert_provider_session(&state.db, "", "test_refresh", &initial)
            .await
            .unwrap();

        // Track call count to simulate 401 on first call, 200 on retry.
        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = with_refresh_retry(&state, "test_refresh", "", |_ctx| {
            let cc = call_count_clone.clone();
            async move {
                let n = cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n == 0 {
                    // First call: simulate 401.
                    Ok(provider_client::ProviderResponse {
                        status: 401,
                        body: serde_json::json!({"error": "unauthorized"}),
                        cookies: HashMap::new(),
                    })
                } else {
                    // Retry: simulate success.
                    Ok(provider_client::ProviderResponse {
                        status: 200,
                        body: serde_json::json!({"ok": true}),
                        cookies: HashMap::new(),
                    })
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(result.status, 200);
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 2);

        // Verify tokens were refreshed.
        let stored = db::get_provider_session_values(&state.db, "", "test_refresh")
            .await
            .unwrap();
        assert_eq!(stored.get("access_token").unwrap(), "fresh-token");
    }

    #[tokio::test]
    async fn with_refresh_retry_no_refresh_config_returns_error_as_is() {
        let (test_db, _dir) = test_db().await;

        // Provider without refresh config.
        let yaml = r#"
provider:
  name: NoRefreshTV
  id: no_refresh
auth:
  flows:
    - id: email
      name: Email Login
      inputs:
        - key: email
          label: Email
      steps:
        - name: login
          request:
            method: POST
            path: /api/login
channels:
  list:
    request:
      method: GET
      path: /api/channels
    response:
      items_path: "$.channels"
playback:
  stream:
    request:
      method: GET
      path: /api/play/{{input.id}}
    response:
      url: "$.url"
      type: "hls"
"#;
        let cfg: ProviderConfig = serde_yaml_ng::from_str(yaml).unwrap();
        let mut providers = HashMap::new();
        providers.insert("no_refresh".into(), cfg);
        let state = build_test_state(test_db, providers);

        // Returns 401 but no refresh config → returns as-is.
        let result = with_refresh_retry(&state, "no_refresh", "", |_ctx| async {
            Ok(provider_client::ProviderResponse {
                status: 401,
                body: serde_json::json!({"error": "unauthorized"}),
                cookies: HashMap::new(),
            })
        })
        .await
        .unwrap();

        assert_eq!(result.status, 401);
    }
}
