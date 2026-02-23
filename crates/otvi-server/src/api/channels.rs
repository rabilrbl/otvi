use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde_json::Value;

use otvi_core::config::AuthScope;
use otvi_core::template::extract_json_path;
use otvi_core::types::*;

use tracing::error;

use crate::auth_middleware::Claims;
use crate::error::AppError;
use crate::provider_client;
use crate::state::AppState;

use super::auth::build_provider_context;

/// Resolve the provider-session user ID based on the provider's auth scope.
fn session_uid(scope: &AuthScope, claims: &Claims) -> String {
    match scope {
        AuthScope::Global => String::new(),
        AuthScope::PerUser => claims.sub.clone(),
    }
}

/// `GET /api/providers/:id/channels`
pub async fn list(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    claims: Claims,
) -> Result<Json<ChannelListResponse>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let uid = session_uid(&provider.auth.scope, &claims);
    let mut context = build_provider_context(&state, &uid, &provider_id).await?;
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

    let mut channels = map_channels(&response, &provider.channels.list.response)?;

    // If a category filter was requested, apply it locally (the upstream API
    // may not support server-side filtering by category).
    if let Some(cat) = params.get("category")
        && !cat.is_empty()
    {
        channels.retain(|ch| ch.category.as_deref() == Some(cat.as_str()));
    }

    Ok(Json(ChannelListResponse { channels }))
}

/// `GET /api/providers/:id/channels/categories`
pub async fn categories(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    claims: Claims,
) -> Result<Json<CategoryListResponse>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    // If the provider defines static categories inline, return them directly
    // without making a network request.
    if !provider.channels.static_categories.is_empty() {
        let categories = provider
            .channels
            .static_categories
            .iter()
            .map(|c| Category {
                id: c.id.clone(),
                name: c.name.clone(),
            })
            .collect();
        return Ok(Json(CategoryListResponse { categories }));
    }

    let cat_endpoint = provider
        .channels
        .categories
        .as_ref()
        .ok_or_else(|| AppError::NotFound("Categories not configured".into()))?;

    let uid = session_uid(&provider.auth.scope, &claims);
    let context = build_provider_context(&state, &uid, &provider_id).await?;

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
    claims: Claims,
) -> Result<Json<StreamInfo>, AppError> {
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let uid = session_uid(&provider.auth.scope, &claims);
    let mut context = build_provider_context(&state, &uid, &provider_id).await?;
    context.set("input.channel_id", &channel_id);

    let response = provider_client::execute_request_body(
        &state.http_client,
        &provider.defaults.base_url,
        &provider.defaults.headers,
        &provider.playback.stream.request,
        &context,
    )
    .await
    .map_err(|e| {
        error!(channel_id = %channel_id, provider = %provider_id, "Playback API error: {e}");
        AppError::Internal(e.to_string())
    })?;

    // Extract stream URL
    let stream_url = extract_json_path(&response, &provider.playback.stream.response.url)
        .ok_or_else(|| {
            error!(channel_id = %channel_id, provider = %provider_id, url_path = %provider.playback.stream.response.url, response = %response, "Stream URL not found in response");
            AppError::Internal("Stream URL not found in response".into())
        })?;

    // Extract stream type: if the value doesn't start with '$.' treat it as a
    // literal (e.g. "hls"), otherwise extract from the response JSON.
    let stream_type_raw = &provider.playback.stream.response.stream_type;
    let stream_type_str = if stream_type_raw.starts_with("$.") {
        extract_json_path(&response, stream_type_raw).unwrap_or_else(|| "hls".to_string())
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

    // Proxy the stream URL through our backend to avoid CORS issues.
    // Build a ProxyContext with resolved headers and URL-param→cookie mappings,
    // store it server-side under an opaque UUID, and embed only the token.
    let proxied_url = {
        let resolved_headers: HashMap<String, String> = provider
            .playback
            .stream
            .proxy_headers
            .iter()
            .map(|(k, v)| (k.clone(), context.resolve(v)))
            .collect();
        // url_param_cookies are param/cookie names — no template resolution needed.
        let url_param_cookies = provider.playback.stream.proxy_url_cookies.clone();
        // proxy_cookies have template vars that need resolving (same as proxy_headers).
        let static_cookies: HashMap<String, String> = provider
            .playback
            .stream
            .proxy_cookies
            .iter()
            .map(|(k, v)| (k.clone(), context.resolve(v)))
            .collect();

        let needs_ctx = !resolved_headers.is_empty()
            || !url_param_cookies.is_empty()
            || !static_cookies.is_empty()
            || provider.playback.stream.append_manifest_query_to_key_uris;
        let base = format!("/api/proxy?url={}", urlencoding::encode(&stream_url));
        if needs_ctx {
            let ctx = crate::state::ProxyContext {
                headers: resolved_headers,
                url_param_cookies,
                resolved_cookies: Default::default(),
                static_cookies,
                manifest_query: None,
                append_manifest_query_to_key_uris: provider
                    .playback
                    .stream
                    .append_manifest_query_to_key_uris,
                key_exclude_resolved_cookies: provider.playback.stream.key_exclude_resolved_cookies,
                key_uri_patterns: provider.playback.stream.key_uri_patterns.clone(),
            };
            let token = uuid::Uuid::new_v4().to_string();
            state.proxy_ctx.write().unwrap().insert(token.clone(), ctx);
            format!("{base}&ctx={token}")
        } else {
            base
        }
    };

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
        let logo = extract_mapped_field(item, field_map, "logo").map(|url| {
            // If logo_base_url is set and the logo is a relative path (no scheme),
            // prepend the base URL so the client gets a fully qualified URL.
            if let Some(base) = &mapping.logo_base_url
                && !url.starts_with("http://")
                && !url.starts_with("https://")
            {
                return format!("{}{}", base, url);
            }
            url
        });
        channels.push(Channel {
            id: extract_mapped_field(item, field_map, "id").unwrap_or_else(|| "unknown".into()),
            name: extract_mapped_field(item, field_map, "name").unwrap_or_else(|| "Unnamed".into()),
            logo,
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
            id: extract_mapped_field(item, field_map, "id").unwrap_or_else(|| "unknown".into()),
            name: extract_mapped_field(item, field_map, "name").unwrap_or_else(|| "Unknown".into()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_session_uid_global() {
        let scope = AuthScope::Global;
        let claims = Claims {
            sub: "user123".to_string(),
            username: "testuser".to_string(),
            role: "user".to_string(),
            exp: 0,
        };
        assert_eq!(session_uid(&scope, &claims), "");
    }

    #[test]
    fn test_session_uid_per_user() {
        let scope = AuthScope::PerUser;
        let claims = Claims {
            sub: "user456".to_string(),
            username: "testuser".to_string(),
            role: "user".to_string(),
            exp: 0,
        };
        assert_eq!(session_uid(&scope, &claims), "user456");
    }

    #[test]
    fn test_extract_mapped_field_found() {
        let item = json!({"id": "ch1", "title": "Channel One"});
        let mut field_map = HashMap::new();
        field_map.insert("id".to_string(), "$.id".to_string());
        field_map.insert("name".to_string(), "$.title".to_string());

        assert_eq!(
            extract_mapped_field(&item, &field_map, "id"),
            Some("ch1".to_string())
        );
        assert_eq!(
            extract_mapped_field(&item, &field_map, "name"),
            Some("Channel One".to_string())
        );
    }

    #[test]
    fn test_extract_mapped_field_not_found() {
        let item = json!({"id": "ch1"});
        let mut field_map = HashMap::new();
        field_map.insert("id".to_string(), "$.id".to_string());

        assert_eq!(extract_mapped_field(&item, &field_map, "missing"), None);
        assert_eq!(extract_mapped_field(&item, &field_map, "name"), None);
    }

    #[test]
    fn test_get_items_array_with_path() {
        let response = json!({"data": {"items": [{"id": 1}, {"id": 2}]}});
        let mapping = otvi_core::config::ResponseMapping {
            items_path: Some("$.data.items".to_string()),
            mapping: HashMap::new(),
            logo_base_url: None,
        };

        let result = get_items_array(&response, &mapping);
        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_get_items_array_no_path() {
        let response = json!([{"id": 1}, {"id": 2}, {"id": 3}]);
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: HashMap::new(),
            logo_base_url: None,
        };

        let result = get_items_array(&response, &mapping);
        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_get_items_array_path_not_found() {
        let response = json!({"other": "data"});
        let mapping = otvi_core::config::ResponseMapping {
            items_path: Some("$.data.items".to_string()),
            mapping: HashMap::new(),
            logo_base_url: None,
        };

        let result = get_items_array(&response, &mapping);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_items_array_not_array() {
        let response = json!({"data": "not an array"});
        let mapping = otvi_core::config::ResponseMapping {
            items_path: Some("$.data".to_string()),
            mapping: HashMap::new(),
            logo_base_url: None,
        };

        let result = get_items_array(&response, &mapping);
        assert!(result.is_err());
    }

    #[test]
    fn test_map_channels_basic() {
        let response = json!([
            {"id": "ch1", "name": "Channel 1"},
            {"id": "ch2", "name": "Channel 2"}
        ]);
        let mut map = HashMap::new();
        map.insert("id".to_string(), "$.id".to_string());
        map.insert("name".to_string(), "$.name".to_string());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: map,
            logo_base_url: None,
        };

        let result = map_channels(&response, &mapping);
        assert!(result.is_ok());
        let channels = result.unwrap();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].id, "ch1");
        assert_eq!(channels[0].name, "Channel 1");
    }

    #[test]
    fn test_map_channels_with_logo_base_url() {
        let response = json!([
            {"id": "ch1", "name": "Channel 1", "logo": "/images/ch1.png"}
        ]);
        let mut map = HashMap::new();
        map.insert("id".to_string(), "$.id".to_string());
        map.insert("name".to_string(), "$.name".to_string());
        map.insert("logo".to_string(), "$.logo".to_string());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: map,
            logo_base_url: Some("https://cdn.example.com".to_string()),
        };

        let result = map_channels(&response, &mapping);
        assert!(result.is_ok());
        let channels = result.unwrap();
        assert_eq!(
            channels[0].logo,
            Some("https://cdn.example.com/images/ch1.png".to_string())
        );
    }

    #[test]
    fn test_map_channels_with_absolute_logo() {
        let response = json!([
            {"id": "ch1", "name": "Channel 1", "logo": "https://other.com/ch1.png"}
        ]);
        let mut map = HashMap::new();
        map.insert("id".to_string(), "$.id".to_string());
        map.insert("name".to_string(), "$.name".to_string());
        map.insert("logo".to_string(), "$.logo".to_string());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: map,
            logo_base_url: Some("https://cdn.example.com".to_string()),
        };

        let result = map_channels(&response, &mapping);
        assert!(result.is_ok());
        let channels = result.unwrap();
        // Absolute URL should not be modified
        assert_eq!(
            channels[0].logo,
            Some("https://other.com/ch1.png".to_string())
        );
    }

    #[test]
    fn test_map_channels_missing_fields() {
        let response = json!([
            {"some": "data"}
        ]);
        let mut map = HashMap::new();
        map.insert("id".to_string(), "$.id".to_string());
        map.insert("name".to_string(), "$.name".to_string());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: map,
            logo_base_url: None,
        };

        let result = map_channels(&response, &mapping);
        assert!(result.is_ok());
        let channels = result.unwrap();
        assert_eq!(channels[0].id, "unknown");
        assert_eq!(channels[0].name, "Unnamed");
    }

    #[test]
    fn test_map_categories() {
        let response = json!([
            {"id": "cat1", "name": "Category 1"},
            {"id": "cat2", "name": "Category 2"}
        ]);
        let mut map = HashMap::new();
        map.insert("id".to_string(), "$.id".to_string());
        map.insert("name".to_string(), "$.name".to_string());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: map,
            logo_base_url: None,
        };

        let result = map_categories(&response, &mapping);
        assert!(result.is_ok());
        let categories = result.unwrap();
        assert_eq!(categories.len(), 2);
        assert_eq!(categories[0].id, "cat1");
        assert_eq!(categories[0].name, "Category 1");
    }

    #[test]
    fn test_map_categories_missing_fields() {
        let response = json!([
            {"other": "data"}
        ]);
        let mut map = HashMap::new();
        map.insert("id".to_string(), "$.id".to_string());
        map.insert("name".to_string(), "$.name".to_string());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: map,
            logo_base_url: None,
        };

        let result = map_categories(&response, &mapping);
        assert!(result.is_ok());
        let categories = result.unwrap();
        assert_eq!(categories[0].id, "unknown");
        assert_eq!(categories[0].name, "Unknown");
    }
}
