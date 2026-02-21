use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use otvi_core::template::{extract_json_path, TemplateContext};
use otvi_core::types::*;

use crate::error::AppError;
use crate::provider_client;
use crate::state::{AppState, SessionData};

/// Apply input transforms declared in the flow's field definitions.
/// When a field has a `transform`, the transformed value is stored alongside
/// the original under the key `<original_key>_<transform>`.  For example,
/// `phone` with `transform: base64` produces both `input.phone` (original)
/// and `input.phone_base64` (encoded).
fn apply_transforms(
    flow: &otvi_core::config::AuthFlow,
    inputs: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut result = inputs.clone();
    for field in &flow.inputs {
        if let Some(transform) = &field.transform {
            if let Some(val) = inputs.get(&field.key) {
                let transformed = match transform.as_str() {
                    "base64" => {
                        use base64::Engine;
                        base64::engine::general_purpose::STANDARD.encode(val.as_bytes())
                    }
                    _ => continue,
                };
                // Store as <key>_<transform>, e.g. "phone_base64"
                result.insert(format!("{}_{}", field.key, transform), transformed);
            }
        }
    }
    result
}

/// `POST /api/providers/:id/auth/login`
///
/// Handles both single-step and multi-step authentication flows.
pub async fn login(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let flow = provider
        .auth
        .flows
        .iter()
        .find(|f| f.id == req.flow_id)
        .ok_or_else(|| AppError::NotFound("Auth flow not found".into()))?;

    let step = flow
        .steps
        .get(req.step)
        .ok_or_else(|| AppError::BadRequest("Invalid step index".into()))?;

    // ── Apply input transforms (e.g. base64) ────────────────────────────
    let transformed_inputs = apply_transforms(flow, &req.inputs);

    // ── Build template context ──────────────────────────────────────────
    let mut context = TemplateContext::new();

    // User-provided inputs (with transforms applied)
    for (k, v) in &transformed_inputs {
        context.set(format!("input.{k}"), v.clone());
    }

    // Stored session values from previous steps
    if let Some(sid) = &req.session_id {
        let sessions = state.sessions.read().unwrap();
        if let Some(session) = sessions.get(sid) {
            for (k, v) in &session.stored_values {
                context.set(format!("stored.{k}"), v.clone());
            }
            for (k, v) in &session.step_extracts {
                context.set(format!("extract.{k}"), v.clone());
                // Also available without prefix for body templates
                context.set(k.clone(), v.clone());
            }
        }
    }

    // Built-in variables
    context.set("uuid", Uuid::new_v4().to_string());
    // Generate a stable device ID (16-char hex) and store in the session
    let device_id = format!("{:016x}", rand::random::<u64>());
    context.set("device_id", &device_id);

    // ── Execute the provider request ────────────────────────────────────
    let response = provider_client::execute_request(
        &state.http_client,
        &provider.defaults.base_url,
        &provider.defaults.headers,
        &step.request,
        &context,
    )
    .await;

    match response {
        Ok(provider_resp) => {
            let json_body = provider_resp.body;
            // Extract values from the response
            let mut extracted = HashMap::new();
            if let Some(on_success) = &step.on_success {
                for (key, path) in &on_success.extract {
                    if let Some(value) = extract_json_path(&json_body, path) {
                        extracted.insert(key.clone(), value);
                    }
                }
            }

            // Always store the device_id so it persists across steps
            extracted.entry("device_id".to_string()).or_insert(device_id);

            // Create or update the server-side session
            let session_id = req
                .session_id
                .unwrap_or_else(|| Uuid::new_v4().to_string());

            {
                let mut sessions = state.sessions.write().unwrap();
                let session =
                    sessions
                        .entry(session_id.clone())
                        .or_insert_with(|| SessionData {
                            provider_id: provider_id.clone(),
                            stored_values: HashMap::new(),
                            step_extracts: HashMap::new(),
                        });
                session.stored_values.extend(extracted.clone());
                session.step_extracts = extracted;
            }
            state.save_sessions();

            // Check whether the flow needs additional user input
            let has_prompt = step
                .on_success
                .as_ref()
                .and_then(|s| s.prompt.as_ref())
                .is_some();

            if has_prompt {
                let prompt_fields = step
                    .on_success
                    .as_ref()
                    .unwrap()
                    .prompt
                    .as_ref()
                    .unwrap();

                let next_step_index = req.step + 1;
                let next_step_name = flow
                    .steps
                    .get(next_step_index)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();

                Ok(Json(LoginResponse {
                    success: false,
                    session_id: Some(session_id),
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
            } else if req.step + 1 < flow.steps.len() {
                // More steps remaining but no additional input needed
                Ok(Json(LoginResponse {
                    success: false,
                    session_id: Some(session_id),
                    next_step: None,
                    user_name: None,
                    error: None,
                }))
            } else {
                // Final step – authentication complete
                let user_name = {
                    let sessions = state.sessions.read().unwrap();
                    sessions
                        .get(&session_id)
                        .and_then(|s| s.stored_values.get("user_name").cloned())
                };

                Ok(Json(LoginResponse {
                    success: true,
                    session_id: Some(session_id),
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

/// `GET /api/providers/:id/auth/check` — validate that the session token is still valid.
pub async fn check_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let session_id = extract_session(&headers)?;
    let sessions = state.sessions.read().unwrap();
    if sessions.contains_key(&session_id) {
        Ok(Json(serde_json::json!({ "valid": true })))
    } else {
        Err(AppError::Unauthorized)
    }
}

/// `POST /api/providers/:id/auth/logout`
pub async fn logout(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let session_id = extract_session(&headers)?;

    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    // Execute the provider's logout endpoint if configured
    if let Some(logout_cfg) = &provider.auth.logout {
        let context = build_context(&state, &session_id)?;
        let _ = provider_client::execute_request_body(
            &state.http_client,
            &provider.defaults.base_url,
            &provider.defaults.headers,
            &logout_cfg.request,
            &context,
        )
        .await;
    }

    // Remove server-side session
    state.sessions.write().unwrap().remove(&session_id);
    state.save_sessions();

    Ok(Json(serde_json::json!({ "success": true })))
}

// ── Helpers ─────────────────────────────────────────────────────────────────

pub(crate) fn extract_session(headers: &HeaderMap) -> Result<String, AppError> {
    headers
        .get("X-Session-Token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(AppError::Unauthorized)
}

pub(crate) fn build_context(
    state: &AppState,
    session_id: &str,
) -> Result<TemplateContext, AppError> {
    let sessions = state.sessions.read().unwrap();
    let session = sessions.get(session_id).ok_or(AppError::Unauthorized)?;

    let mut context = TemplateContext::new();
    for (k, v) in &session.stored_values {
        context.set(format!("stored.{k}"), v.clone());
    }
    context.set("uuid", uuid::Uuid::new_v4().to_string());

    Ok(context)
}
