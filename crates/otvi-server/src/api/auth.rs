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
