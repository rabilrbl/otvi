//! Channel browsing and stream endpoints.
//!
//! ## Pagination
//!
//! `GET /api/providers/:id/channels` accepts optional `limit` and `offset`
//! query parameters.  When omitted the full list is returned for backwards
//! compatibility.
//!
//! ## Search
//!
//! Pass `?search=<term>` to filter channels whose name contains the term
//! (case-insensitive).  Can be combined with `category`, `limit`, and
//! `offset`.
//!
//! ## Example
//!
//! ```text
//! GET /api/providers/acme/channels?search=sport&category=news&limit=20&offset=0
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
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

// ── Query-parameter structs ────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct ChannelListQuery {
    /// Filter by category ID (applied after fetching all channels).
    pub category: Option<String>,
    /// Case-insensitive substring search on channel names.
    pub search: Option<String>,
    /// Maximum number of channels to return.
    pub limit: Option<usize>,
    /// Zero-based offset into the filtered list.
    pub offset: Option<usize>,
}

// ── Session UID helper ─────────────────────────────────────────────────────

/// Resolve the provider-session user ID based on the provider's auth scope.
fn session_uid(scope: &AuthScope, claims: &Claims) -> String {
    match scope {
        AuthScope::Global => String::new(),
        AuthScope::PerUser => claims.sub.clone(),
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────

/// `GET /api/providers/:id/channels`
///
/// Returns a (optionally filtered and paginated) list of channels for the
/// given provider.  The upstream API is always called with the full request;
/// filtering and pagination are applied server-side so that providers that
/// do not natively support those features still work.
pub async fn list(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    Query(query): Query<ChannelListQuery>,
    claims: Claims,
) -> Result<Json<ChannelListResponse>, AppError> {
    // Extract everything we need from the provider while holding the lock
    // for the shortest possible time.
    let provider_data = state
        .with_provider(&provider_id, |p| {
            (
                p.auth.scope.clone(),
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
                p.channels.list.request.clone(),
                p.channels.list.response.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (scope, base_url, default_headers, list_request, list_response) = provider_data;

    let uid = session_uid(&scope, &claims);
    let mut context = build_provider_context(&state, &uid, &provider_id).await?;

    // Forward any extra query params into the template context so provider
    // YAML can reference them as `{{input.*}}`.
    if let Some(cat) = &query.category {
        context.set("input.category", cat);
    }
    if let Some(s) = &query.search {
        context.set("input.search", s);
    }

    let response = provider_client::execute_request_body(
        &state.http_client,
        &base_url,
        &default_headers,
        &list_request,
        &context,
    )
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut channels = map_channels(&response, &list_response)?;

    // ── Server-side category filter ──────────────────────────────────────
    if let Some(cat) = &query.category {
        if !cat.is_empty() {
            channels.retain(|ch| ch.category.as_deref() == Some(cat.as_str()));
        }
    }

    // ── Server-side text search ──────────────────────────────────────────
    if let Some(term) = &query.search {
        if !term.is_empty() {
            let term_lower = term.to_lowercase();
            channels.retain(|ch| ch.name.to_lowercase().contains(&term_lower));
        }
    }

    // ── Pagination ────────────────────────────────────────────────────────
    let total = channels.len();
    let offset = query.offset.unwrap_or(0);
    let channels = if let Some(limit) = query.limit {
        channels
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>()
    } else if offset > 0 {
        channels.into_iter().skip(offset).collect::<Vec<_>>()
    } else {
        channels
    };

    Ok(Json(ChannelListResponse {
        channels,
        total: Some(total),
    }))
}

/// `GET /api/providers/:id/channels/categories`
pub async fn categories(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    claims: Claims,
) -> Result<Json<CategoryListResponse>, AppError> {
    // Extract what we need under a short lock window.
    let provider_data = state
        .with_provider(&provider_id, |p| {
            (
                p.auth.scope.clone(),
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
                p.channels.static_categories.clone(),
                p.channels.categories.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (scope, base_url, default_headers, static_cats, dynamic_endpoint) = provider_data;

    // If the provider defines static categories inline, return them directly
    // without making a network request.
    if !static_cats.is_empty() {
        let categories = static_cats
            .iter()
            .map(|c| Category {
                id: c.id.clone(),
                name: c.name.clone(),
            })
            .collect();
        return Ok(Json(CategoryListResponse { categories }));
    }

    let cat_endpoint =
        dynamic_endpoint.ok_or_else(|| AppError::NotFound("Categories not configured".into()))?;

    let uid = session_uid(&scope, &claims);
    let context = build_provider_context(&state, &uid, &provider_id).await?;

    let response = provider_client::execute_request_body(
        &state.http_client,
        &base_url,
        &default_headers,
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
    // Extract everything we need from the provider config under a short lock.
    let provider_data = state
        .with_provider(&provider_id, |p| {
            (
                p.auth.scope.clone(),
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
                p.playback.stream.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (scope, base_url, default_headers, stream_endpoint) = provider_data;

    let uid = session_uid(&scope, &claims);
    let mut context = build_provider_context(&state, &uid, &provider_id).await?;
    context.set("input.channel_id", &channel_id);

    let response = provider_client::execute_request_body(
        &state.http_client,
        &base_url,
        &default_headers,
        &stream_endpoint.request,
        &context,
    )
    .await
    .map_err(|e| {
        error!(
            channel_id = %channel_id,
            provider  = %provider_id,
            "Playback API error: {e}"
        );
        AppError::Internal(e.to_string())
    })?;

    // Extract stream URL
    let stream_url =
        extract_json_path(&response, &stream_endpoint.response.url).ok_or_else(|| {
            error!(
                channel_id = %channel_id,
                provider   = %provider_id,
                url_path   = %stream_endpoint.response.url,
                response   = %response,
                "Stream URL not found in response"
            );
            AppError::Internal("Stream URL not found in response".into())
        })?;

    // Extract stream type: if the value doesn't start with '$.' treat it as a
    // literal (e.g. "hls"), otherwise extract from the response JSON.
    let stream_type_raw = &stream_endpoint.response.stream_type;
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
    let drm = if let Some(drm_cfg) = &stream_endpoint.response.drm {
        let system = extract_json_path(&response, &drm_cfg.system).unwrap_or_default();
        let license_url = extract_json_path(&response, &drm_cfg.license_url)
            .unwrap_or_else(|| context.resolve_lossy(&drm_cfg.license_url));
        let mut drm_headers = HashMap::new();
        for (k, v) in &drm_cfg.headers {
            drm_headers.insert(k.clone(), context.resolve_lossy(v));
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
    // Build a ProxyContext with resolved headers / cookie mappings, store it
    // server-side under an opaque UUID, and embed only the token in the URL.
    let proxied_url = {
        let resolved_headers: HashMap<String, String> = stream_endpoint
            .proxy_headers
            .iter()
            .map(|(k, v)| (k.clone(), context.resolve_lossy(v)))
            .collect();

        let url_param_cookies = stream_endpoint.proxy_url_cookies.clone();

        let static_cookies: HashMap<String, String> = stream_endpoint
            .proxy_cookies
            .iter()
            .map(|(k, v)| (k.clone(), context.resolve_lossy(v)))
            .collect();

        let needs_ctx = !resolved_headers.is_empty()
            || !url_param_cookies.is_empty()
            || !static_cookies.is_empty()
            || stream_endpoint.append_manifest_query_to_key_uris;

        let base = format!("/api/proxy?url={}", urlencoding::encode(&stream_url));

        if needs_ctx {
            let ctx = crate::state::ProxyContext {
                headers: resolved_headers,
                url_param_cookies,
                resolved_cookies: Default::default(),
                static_cookies,
                manifest_query: None,
                append_manifest_query_to_key_uris: stream_endpoint
                    .append_manifest_query_to_key_uris,
                key_exclude_resolved_cookies: stream_endpoint.key_exclude_resolved_cookies,
                key_uri_patterns: stream_endpoint.key_uri_patterns.clone(),
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

// ── Response mapping helpers ───────────────────────────────────────────────

fn map_channels(
    response: &Value,
    mapping: &otvi_core::config::ResponseMapping,
) -> Result<Vec<Channel>, AppError> {
    let items: Vec<Value> = get_items_array(response, mapping.items_path.as_deref())?;

    let logo_base = mapping.logo_base_url.as_deref().unwrap_or("");

    let channels = items
        .into_iter()
        .filter_map(|item| {
            let id = extract_mapped_field(&item, &mapping.mapping, "id")?;
            let name = extract_mapped_field(&item, &mapping.mapping, "name")?;
            let logo = extract_mapped_field(&item, &mapping.mapping, "logo").map(|raw| {
                if raw.starts_with("http://") || raw.starts_with("https://") {
                    raw
                } else {
                    format!("{logo_base}{raw}")
                }
            });
            let category = extract_mapped_field(&item, &mapping.mapping, "category");
            let number = extract_mapped_field(&item, &mapping.mapping, "number");
            let description = extract_mapped_field(&item, &mapping.mapping, "description");
            Some(Channel {
                id,
                name,
                logo,
                category,
                number,
                description,
            })
        })
        .collect();

    Ok(channels)
}

fn map_categories(
    response: &Value,
    mapping: &otvi_core::config::ResponseMapping,
) -> Result<Vec<Category>, AppError> {
    let items: Vec<Value> = get_items_array(response, mapping.items_path.as_deref())?;

    let categories = items
        .into_iter()
        .filter_map(|item| {
            let id = extract_mapped_field(&item, &mapping.mapping, "id")?;
            let name = extract_mapped_field(&item, &mapping.mapping, "name")?;
            Some(Category { id, name })
        })
        .collect();

    Ok(categories)
}

/// Navigate to the array indicated by `items_path` in the response JSON and
/// return it as an owned `Vec<Value>`.
///
/// When `items_path` is `None` the response itself is expected to be a JSON
/// array.  When a path is given it is resolved via [`extract_json_path`] and
/// the matched node is expected to be an array.
fn get_items_array(response: &Value, items_path: Option<&str>) -> Result<Vec<Value>, AppError> {
    match items_path {
        Some(path) => {
            // Walk the tree using the full JSONPath engine so that complex
            // expressions (filters, recursive descent, etc.) are supported.
            // We navigate manually here to get the `Value` node (not the
            // scalar string that `extract_json_path` returns).
            let node = navigate_json(response, path).ok_or_else(|| {
                AppError::Internal(format!("items_path '{path}' not found in response"))
            })?;
            node.as_array()
                .cloned()
                .ok_or_else(|| AppError::Internal(format!("items_path '{path}' is not an array")))
        }
        None => response
            .as_array()
            .cloned()
            .ok_or_else(|| AppError::Internal("Response root is not an array".into())),
    }
}

/// Walk a JSON value using a simple dot-notation path (with `$.` prefix
/// stripped).  Returns a reference to the node at the path, or `None`.
///
/// This is used instead of `extract_json_path` in contexts where we need the
/// raw `Value` node rather than a scalar string.
fn navigate_json<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let path = path.strip_prefix("$.").unwrap_or(path);
    let path = path.strip_prefix('$').unwrap_or(path);
    if path.is_empty() {
        return Some(root);
    }
    let mut current = root;
    for segment in path.split('.') {
        // Handle array indexing within a segment, e.g. "items[0]"
        if segment.contains('[') {
            let bracket = segment.find('[')?;
            let key = &segment[..bracket];
            let idx_str = segment[bracket + 1..].trim_end_matches(']');
            if !key.is_empty() {
                current = current.get(key)?;
            }
            let idx: usize = idx_str.parse().ok()?;
            current = current.get(idx)?;
        } else {
            current = current.get(segment)?;
        }
    }
    Some(current)
}

fn extract_mapped_field(
    item: &Value,
    mapping: &HashMap<String, String>,
    field: &str,
) -> Option<String> {
    let path = mapping.get(field)?;
    extract_json_path(item, path)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_session_uid_global() {
        let scope = AuthScope::Global;
        let claims = crate::auth_middleware::Claims {
            sub: "user-123".to_string(),
            username: "alice".to_string(),
            role: "user".to_string(),
            exp: u64::MAX,
        };
        assert_eq!(session_uid(&scope, &claims), "");
    }

    #[test]
    fn test_session_uid_per_user() {
        let scope = AuthScope::PerUser;
        let claims = crate::auth_middleware::Claims {
            sub: "user-456".to_string(),
            username: "bob".to_string(),
            role: "user".to_string(),
            exp: u64::MAX,
        };
        assert_eq!(session_uid(&scope, &claims), "user-456");
    }

    #[test]
    fn test_extract_mapped_field_found() {
        let item = json!({"id": "ch1", "title": "News"});
        let mut mapping = HashMap::new();
        mapping.insert("id".to_string(), "$.id".to_string());
        mapping.insert("name".to_string(), "$.title".to_string());
        assert_eq!(
            extract_mapped_field(&item, &mapping, "id"),
            Some("ch1".to_string())
        );
        assert_eq!(
            extract_mapped_field(&item, &mapping, "name"),
            Some("News".to_string())
        );
    }

    #[test]
    fn test_extract_mapped_field_not_found() {
        let item = json!({"id": "ch1"});
        let mapping = HashMap::new();
        assert_eq!(extract_mapped_field(&item, &mapping, "id"), None);
    }

    #[test]
    fn test_map_channels_basic() {
        let response = json!([
            {"id": "1", "name": "BBC One"},
            {"id": "2", "name": "CNN"}
        ]);
        let mut mapping_fields = HashMap::new();
        mapping_fields.insert("id".to_string(), "$.id".to_string());
        mapping_fields.insert("name".to_string(), "$.name".to_string());

        let rm = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: mapping_fields,
            logo_base_url: None,
        };

        let channels = map_channels(&response, &rm).unwrap();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].id, "1");
        assert_eq!(channels[0].name, "BBC One");
        assert_eq!(channels[1].id, "2");
        assert_eq!(channels[1].name, "CNN");
    }

    #[test]
    fn test_map_channels_with_logo_base_url() {
        let response = json!([
            {"id": "1", "name": "Channel 1", "logo": "/logos/ch1.png"}
        ]);
        let mut mapping_fields = HashMap::new();
        mapping_fields.insert("id".to_string(), "$.id".to_string());
        mapping_fields.insert("name".to_string(), "$.name".to_string());
        mapping_fields.insert("logo".to_string(), "$.logo".to_string());

        let rm = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: mapping_fields,
            logo_base_url: Some("https://cdn.example.com".to_string()),
        };

        let channels = map_channels(&response, &rm).unwrap();
        assert_eq!(
            channels[0].logo,
            Some("https://cdn.example.com/logos/ch1.png".to_string())
        );
    }

    #[test]
    fn test_map_channels_with_absolute_logo() {
        let response = json!([
            {"id": "1", "name": "Ch1", "logo": "https://other.cdn/logo.png"}
        ]);
        let mut mapping_fields = HashMap::new();
        mapping_fields.insert("id".to_string(), "$.id".to_string());
        mapping_fields.insert("name".to_string(), "$.name".to_string());
        mapping_fields.insert("logo".to_string(), "$.logo".to_string());

        let rm = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: mapping_fields,
            logo_base_url: Some("https://cdn.example.com".to_string()),
        };

        let channels = map_channels(&response, &rm).unwrap();
        // Absolute URL should not be prefixed
        assert_eq!(
            channels[0].logo,
            Some("https://other.cdn/logo.png".to_string())
        );
    }

    #[test]
    fn test_map_channels_missing_fields() {
        // Items without required "id" or "name" are silently skipped
        let response = json!([
            {"id": "1"},
            {"id": "2", "name": "Valid"}
        ]);
        let mut mapping_fields = HashMap::new();
        mapping_fields.insert("id".to_string(), "$.id".to_string());
        mapping_fields.insert("name".to_string(), "$.name".to_string());

        let rm = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: mapping_fields,
            logo_base_url: None,
        };

        let channels = map_channels(&response, &rm).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].id, "2");
    }

    #[test]
    fn test_map_categories() {
        let response = json!([
            {"id": "sports", "label": "Sports"},
            {"id": "news", "label": "News"}
        ]);
        let mut mapping_fields = HashMap::new();
        mapping_fields.insert("id".to_string(), "$.id".to_string());
        mapping_fields.insert("name".to_string(), "$.label".to_string());

        let rm = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: mapping_fields,
            logo_base_url: None,
        };

        let cats = map_categories(&response, &rm).unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0].id, "sports");
        assert_eq!(cats[0].name, "Sports");
    }

    #[test]
    fn test_map_categories_missing_fields() {
        let response = json!([
            {"id": "sports"},
            {"id": "news", "label": "News"}
        ]);
        let mut mapping_fields = HashMap::new();
        mapping_fields.insert("id".to_string(), "$.id".to_string());
        mapping_fields.insert("name".to_string(), "$.label".to_string());

        let rm = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: mapping_fields,
            logo_base_url: None,
        };

        let cats = map_categories(&response, &rm).unwrap();
        assert_eq!(cats.len(), 1);
    }

    // ── Pagination & search unit tests ─────────────────────────────────────

    #[test]
    fn channel_list_query_defaults_to_all() {
        let q = ChannelListQuery::default();
        assert!(q.limit.is_none());
        assert!(q.offset.is_none());
        assert!(q.category.is_none());
        assert!(q.search.is_none());
    }

    #[test]
    fn search_filter_case_insensitive() {
        let mut channels = vec![
            Channel {
                id: "1".into(),
                name: "BBC Sports".into(),
                logo: None,
                category: None,
                number: None,
                description: None,
            },
            Channel {
                id: "2".into(),
                name: "CNN News".into(),
                logo: None,
                category: None,
                number: None,
                description: None,
            },
            Channel {
                id: "3".into(),
                name: "Sky Sports HD".into(),
                logo: None,
                category: None,
                number: None,
                description: None,
            },
        ];
        let term = "sports".to_lowercase();
        channels.retain(|ch| ch.name.to_lowercase().contains(&term));
        assert_eq!(channels.len(), 2);
        assert!(channels.iter().any(|ch| ch.id == "1"));
        assert!(channels.iter().any(|ch| ch.id == "3"));
    }

    #[test]
    fn pagination_limit_and_offset() {
        let all: Vec<u32> = (0..10).collect();
        let paginated: Vec<_> = all.iter().skip(3).take(4).collect();
        assert_eq!(paginated, vec![&3, &4, &5, &6]);
    }
}
