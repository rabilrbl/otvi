use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde_json::Value;

use otvi_core::template::extract_json_path;
use otvi_core::types::*;

use crate::error::AppError;
use crate::provider_client;
use crate::state::AppState;

use super::auth::{build_context, extract_session};

/// `GET /api/providers/:id/channels`
pub async fn list(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<ChannelListResponse>, AppError> {
    let session_id = extract_session(&headers)?;
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let mut context = build_context(&state, &session_id)?;
    // Forward query params as input.* variables
    for (k, v) in &params {
        context.set(format!("input.{k}"), v.clone());
    }

    let response = provider_client::execute_request_body(
        &state.http_client,
        &provider.defaults.base_url,
        &provider.defaults.headers,
        &provider.channels.list.request,
        &context,
    )
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let channels = map_channels(&response, &provider.channels.list.response)?;

    Ok(Json(ChannelListResponse { channels }))
}

/// `GET /api/providers/:id/channels/categories`
pub async fn categories(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CategoryListResponse>, AppError> {
    let session_id = extract_session(&headers)?;
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let cat_endpoint = provider
        .channels
        .categories
        .as_ref()
        .ok_or_else(|| AppError::NotFound("Categories not configured".into()))?;

    let context = build_context(&state, &session_id)?;

    let response = provider_client::execute_request_body(
        &state.http_client,
        &provider.defaults.base_url,
        &provider.defaults.headers,
        &cat_endpoint.request,
        &context,
    )
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let categories = map_categories(&response, &cat_endpoint.response)?;

    Ok(Json(CategoryListResponse { categories }))
}

/// `GET /api/providers/:id/channels/:channel_id/stream`
pub async fn stream(
    State(state): State<Arc<AppState>>,
    Path((provider_id, channel_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<StreamInfo>, AppError> {
    let session_id = extract_session(&headers)?;
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let mut context = build_context(&state, &session_id)?;
    context.set("input.channel_id", &channel_id);

    let response = provider_client::execute_request_body(
        &state.http_client,
        &provider.defaults.base_url,
        &provider.defaults.headers,
        &provider.playback.stream.request,
        &context,
    )
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    // Extract stream URL
    let stream_url =
        extract_json_path(&response, &provider.playback.stream.response.url)
            .ok_or_else(|| AppError::Internal("Stream URL not found in response".into()))?;

    // Extract stream type: if the value doesn't start with '$.' treat it as a
    // literal (e.g. "hls"), otherwise extract from the response JSON.
    let stream_type_raw = &provider.playback.stream.response.stream_type;
    let stream_type_str = if stream_type_raw.starts_with("$.") {
        extract_json_path(&response, stream_type_raw)
            .unwrap_or_else(|| "hls".to_string())
    } else {
        stream_type_raw.clone()
    };

    let stream_type = match stream_type_str.to_lowercase().as_str() {
        "dash" | "mpd" => StreamType::Dash,
        _ => StreamType::Hls,
    };

    // Extract optional DRM configuration
    let drm = if let Some(drm_cfg) = &provider.playback.stream.response.drm {
        let system = extract_json_path(&response, &drm_cfg.system).unwrap_or_default();
        let license_url = {
            // Try as JSONPath first, fall back to template resolution
            extract_json_path(&response, &drm_cfg.license_url)
                .unwrap_or_else(|| context.resolve(&drm_cfg.license_url))
        };
        let mut drm_headers = HashMap::new();
        for (k, v) in &drm_cfg.headers {
            drm_headers.insert(k.clone(), context.resolve(v));
        }
        Some(DrmInfo {
            system,
            license_url,
            headers: drm_headers,
        })
    } else {
        None
    };

    // Proxy the stream URL through our backend to avoid CORS issues
    let proxied_url = format!(
        "/api/proxy?url={}",
        urlencoding::encode(&stream_url)
    );

    Ok(Json(StreamInfo {
        url: proxied_url,
        stream_type,
        drm,
    }))
}

// ── Response mapping helpers ────────────────────────────────────────────────

fn map_channels(
    response: &Value,
    mapping: &otvi_core::config::ResponseMapping,
) -> Result<Vec<Channel>, AppError> {
    let items = get_items_array(response, mapping)?;
    let field_map = &mapping.mapping;

    let mut channels = Vec::new();
    for item in items {
        channels.push(Channel {
            id: extract_mapped_field(item, field_map, "id")
                .unwrap_or_else(|| "unknown".into()),
            name: extract_mapped_field(item, field_map, "name")
                .unwrap_or_else(|| "Unnamed".into()),
            logo: extract_mapped_field(item, field_map, "logo"),
            category: extract_mapped_field(item, field_map, "category"),
            number: extract_mapped_field(item, field_map, "number"),
            description: extract_mapped_field(item, field_map, "description"),
        });
    }

    Ok(channels)
}

fn map_categories(
    response: &Value,
    mapping: &otvi_core::config::ResponseMapping,
) -> Result<Vec<Category>, AppError> {
    let items = get_items_array(response, mapping)?;
    let field_map = &mapping.mapping;

    let mut categories = Vec::new();
    for item in items {
        categories.push(Category {
            id: extract_mapped_field(item, field_map, "id")
                .unwrap_or_else(|| "unknown".into()),
            name: extract_mapped_field(item, field_map, "name")
                .unwrap_or_else(|| "Unknown".into()),
        });
    }

    Ok(categories)
}

/// Navigate to the array indicated by `items_path` in the response.
fn get_items_array<'a>(
    response: &'a Value,
    mapping: &otvi_core::config::ResponseMapping,
) -> Result<&'a Vec<Value>, AppError> {
    let root = if let Some(path) = &mapping.items_path {
        let path = path.strip_prefix("$.").unwrap_or(path);
        let mut current = response;
        for part in path.split('.') {
            current = current
                .get(part)
                .ok_or_else(|| AppError::Internal(format!("Path segment '{part}' not found")))?;
        }
        current
    } else {
        response
    };

    root.as_array()
        .ok_or_else(|| AppError::Internal("Expected array in response".into()))
}

/// Extract a single mapped field from an item using the mapping table.
fn extract_mapped_field(
    item: &Value,
    field_map: &HashMap<String, String>,
    canonical_name: &str,
) -> Option<String> {
    let json_path = field_map.get(canonical_name)?;
    extract_json_path(item, json_path)
}
