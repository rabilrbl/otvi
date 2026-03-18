//! Channel browsing and stream endpoints.
//!
//! ## Caching
//!
//! The `list` and `categories` endpoints cache the **full upstream response**
//! in [`AppState::channel_cache`] keyed by [`ChannelCacheKey`].
//!
//! - For `AuthScope::Global` providers the key uses [`CacheScope::Global`] —
//!   one shared entry covers all OTVI users since they all share a single
//!   upstream session.
//! - For `AuthScope::PerUser` providers the key uses [`CacheScope::PerUser`]
//!   with the OTVI user ID — each user's cache entry is fully isolated.
//! - Server-side filtering, search, and pagination are **always** applied to
//!   the cached data on every request — only the raw upstream API call is
//!   cached, not the filtered result.
//! - Cache entries are invalidated automatically after the configured TTL
//!   (default 24 h, overridable via `CHANNEL_CACHE_TTL_SECS`).
//! - Entries are also invalidated explicitly on provider login / logout so
//!   that a credential refresh is always reflected immediately.
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

use otvi_core::template::{extract_json_path, select_json_path_value};
use otvi_core::types::*;

use tracing::{debug, error, instrument};

use crate::api::provider_access::authorize_provider_route;
use crate::auth_middleware::ActiveClaims;
use crate::error::AppError;
use crate::provider_client;
use crate::state::CacheScope;
use crate::state::{AppState, CachedCategories, CachedChannels, ChannelCacheKey};

use super::auth::{build_provider_context, with_refresh_retry};

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

// ── Handlers ──────────────────────────────────────────────────────────────

/// `GET /api/providers/:id/channels`
///
/// Returns a (optionally filtered and paginated) list of channels for the
/// given provider.
///
/// ## Caching strategy
///
/// The full, unfiltered channel list from the upstream provider is cached
/// server-side for the configured TTL.  Filtering, search, and pagination are
/// applied to the cached data on every request so that a single upstream call
/// serves many different filtered views without hitting the provider API each
/// time.
#[utoipa::path(
    get,
    path = "/api/providers/{id}/channels",
    tag = "channels",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "Provider ID"),
        ("category" = Option<String>, Query, description = "Filter by category ID"),
        ("search" = Option<String>, Query, description = "Case-insensitive substring search on channel names"),
        ("limit" = Option<usize>, Query, description = "Maximum number of channels to return"),
        ("offset" = Option<usize>, Query, description = "Zero-based offset for pagination"),
    ),
    responses(
        (status = 200, description = "Filtered and paginated channel list", body = ChannelListResponse),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Password change required"),
        (status = 404, description = "Provider not found"),
        (status = 500, description = "Upstream provider API error"),
    ),
)]
#[instrument(skip(state, claims), fields(provider = %provider_id))]
pub async fn list(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    Query(query): Query<ChannelListQuery>,
    claims: ActiveClaims,
) -> Result<Json<ChannelListResponse>, AppError> {
    let scope = authorize_provider_route(&state, &claims, &provider_id, false).await?;

    // Extract everything we need from the provider while holding the lock
    // for the shortest possible time.
    let provider_data = state
        .with_provider(&provider_id, |p| {
            (
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
                p.channels.list.request.clone(),
                p.channels.list.response.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (base_url, default_headers, list_request, list_response) = provider_data;
    let cache_key = ChannelCacheKey::from_auth_scope(&provider_id, &scope, &claims.sub);
    // The session UID used to build the provider context: empty for Global scope,
    // the OTVI user ID for PerUser scope.
    let uid = match &cache_key.scope {
        CacheScope::Global => String::new(),
        CacheScope::PerUser(uid) => uid.clone(),
    };

    // ── Cache lookup ──────────────────────────────────────────────────────
    let mut extra_ctx: Vec<(&str, &str)> = Vec::new();
    if let Some(cat) = &query.category {
        extra_ctx.push(("input.category", cat));
    }
    if let Some(s) = &query.search {
        extra_ctx.push(("input.search", s));
    }

    let all_channels = load_all_channels(
        &state,
        &provider_id,
        &uid,
        &cache_key,
        &base_url,
        &default_headers,
        &list_request,
        &list_response,
        &extra_ctx,
    )
    .await?;

    // ── Server-side filtering ─────────────────────────────────────────────
    let mut channels: Vec<&Channel> = all_channels.iter().collect();

    // ── Server-side category filter ──────────────────────────────────────
    if let Some(cat) = &query.category
        && !cat.is_empty()
    {
        channels.retain(|ch| ch.category.as_deref() == Some(cat.as_str()));
    }

    // ── Server-side text search ──────────────────────────────────────────
    if let Some(term) = &query.search
        && !term.is_empty()
    {
        let term_lower = term.to_lowercase();
        channels.retain(|ch| ch.name.to_lowercase().contains(&term_lower));
    }

    // ── Pagination ────────────────────────────────────────────────────────
    let total = channels.len();
    let offset = query.offset.unwrap_or(0);
    let channels = if let Some(limit) = query.limit {
        channels
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
    } else if offset > 0 {
        channels
            .into_iter()
            .skip(offset)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        channels.into_iter().cloned().collect::<Vec<_>>()
    };

    Ok(Json(ChannelListResponse {
        channels,
        total: Some(total),
    }))
}

/// `GET /api/providers/:id/channels/categories`
///
/// ## Caching strategy
///
/// Static categories (defined inline in the provider YAML) bypass the cache
/// entirely — they are free to return and never stale.  Dynamic categories
/// fetched from the upstream API are cached with the same TTL as channel lists.
#[utoipa::path(
    get,
    path = "/api/providers/{id}/channels/categories",
    tag = "channels",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "Provider ID"),
    ),
    responses(
        (status = 200, description = "List of channel categories for the provider", body = CategoryListResponse),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Password change required"),
        (status = 404, description = "Provider not found or categories not configured"),
        (status = 500, description = "Upstream provider API error"),
    ),
)]
#[instrument(skip(state, claims), fields(provider = %provider_id))]
pub async fn categories(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    claims: ActiveClaims,
) -> Result<Json<CategoryListResponse>, AppError> {
    let scope = authorize_provider_route(&state, &claims, &provider_id, false).await?;

    // Extract what we need under a short lock window.
    let provider_data = state
        .with_provider(&provider_id, |p| {
            (
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
                p.channels.static_categories.clone(),
                p.channels.categories.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (base_url, default_headers, static_cats, dynamic_endpoint) = provider_data;

    // ── Static categories: return immediately, no caching needed ──────────
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

    let cache_key = ChannelCacheKey::from_auth_scope(&provider_id, &scope, &claims.sub);
    let uid = match &cache_key.scope {
        CacheScope::Global => String::new(),
        CacheScope::PerUser(uid) => uid.clone(),
    };

    // ── Cache lookup ──────────────────────────────────────────────────────
    let categories: Arc<[Category]> =
        if let Some(cached) = state.channel_cache.categories.get(&cache_key).await {
            debug!(provider = %provider_id, "categories cache HIT");
            cached.categories
        } else {
            debug!(provider = %provider_id, "categories cache MISS — fetching from upstream");

            let base_url = base_url.clone();
            let default_headers = default_headers.clone();
            let cat_request = cat_endpoint.request.clone();
            let http_client = state.http_client.clone();

            let resp = with_refresh_retry(&state, &provider_id, &uid, |ctx| {
                let http_client = http_client.clone();
                let base_url = base_url.clone();
                let default_headers = default_headers.clone();
                let cat_request = cat_request.clone();
                async move {
                    provider_client::execute_request_raw(
                        &http_client,
                        &base_url,
                        &default_headers,
                        &cat_request,
                        &ctx,
                    )
                    .await
                }
            })
            .await?;

            if !(200..300).contains(&resp.status) {
                error!(
                    provider = %provider_id,
                    status = resp.status,
                    "Upstream categories error after refresh retry"
                );
                return Err(AppError::Internal(format!(
                    "Upstream categories returned status {}",
                    resp.status
                )));
            }

            let response = resp.body;

            let cats = Arc::<[Category]>::from(map_categories(&response, &cat_endpoint.response)?);

            state
                .channel_cache
                .categories
                .insert(
                    cache_key,
                    CachedCategories {
                        categories: cats.clone(),
                    },
                )
                .await;

            cats
        };

    Ok(Json(CategoryListResponse {
        categories: categories.iter().cloned().collect(),
    }))
}

/// `GET /api/providers/:id/channels/:channel_id/stream`
#[utoipa::path(
    get,
    path = "/api/providers/{id}/channels/{channel_id}/stream",
    tag = "channels",
    security(("bearer_token" = [])),
    params(
        ("id" = String, Path, description = "Provider ID"),
        ("channel_id" = String, Path, description = "Channel ID"),
    ),
    responses(
        (status = 200, description = "Proxied stream URL with optional DRM info", body = StreamInfo),
        (status = 401, description = "Missing or invalid token"),
        (status = 403, description = "Password change required"),
        (status = 404, description = "Provider not found"),
        (status = 500, description = "Upstream provider API error or stream URL not found in response"),
    ),
)]
#[instrument(skip(state, claims), fields(provider = %provider_id, channel = %channel_id))]
pub async fn stream(
    State(state): State<Arc<AppState>>,
    Path((provider_id, channel_id)): Path<(String, String)>,
    claims: ActiveClaims,
) -> Result<Json<StreamInfo>, AppError> {
    let scope = authorize_provider_route(&state, &claims, &provider_id, false).await?;

    // Extract everything we need from the provider config under a short lock.
    let provider_data = state
        .with_provider(&provider_id, |p| {
            (
                p.defaults.base_url.clone(),
                p.defaults.headers.clone(),
                p.channels.list.request.clone(),
                p.channels.list.response.clone(),
                p.playback.stream.clone(),
            )
        })
        .ok_or_else(|| AppError::NotFound("Provider not found".into()))?;

    let (base_url, default_headers, list_request, list_response, stream_endpoint) = provider_data;

    // Derive the session UID from the auth scope without the old string sentinel.
    let uid = match scope {
        otvi_core::config::AuthScope::Global => String::new(),
        otvi_core::config::AuthScope::PerUser => claims.sub.clone(),
    };
    let http_client = state.http_client.clone();
    let stream_base_url = base_url.clone();
    let stream_default_headers = default_headers.clone();
    let stream_request = stream_endpoint.request.clone();
    let channel_id_clone = channel_id.clone();

    let resp = with_refresh_retry(&state, &provider_id, &uid, |mut ctx| {
        let http_client = http_client.clone();
        let stream_base_url = stream_base_url.clone();
        let stream_default_headers = stream_default_headers.clone();
        let stream_request = stream_request.clone();
        let channel_id_clone = channel_id_clone.clone();
        async move {
            ctx.set("input.channel_id", &channel_id_clone);
            provider_client::execute_request_raw(
                &http_client,
                &stream_base_url,
                &stream_default_headers,
                &stream_request,
                &ctx,
            )
            .await
        }
    })
    .await?;

    if !(200..300).contains(&resp.status) {
        error!(
            channel_id = %channel_id,
            provider = %provider_id,
            status = resp.status,
            "Playback API error after refresh retry"
        );
        return Err(AppError::Internal(format!(
            "Playback API returned status {}",
            resp.status
        )));
    }

    let response = resp.body;

    // Build a template context for downstream template resolutions
    // (DRM fields, proxy headers/cookies). This uses the latest stored values
    // (which may have been refreshed by with_refresh_retry above).
    let mut context = build_provider_context(&state, &uid, &provider_id).await?;
    context.set("input.channel_id", &channel_id);

    let cache_key = ChannelCacheKey::from_auth_scope(&provider_id, &scope, &claims.sub);
    let channel_meta = load_all_channels(
        &state,
        &provider_id,
        &uid,
        &cache_key,
        &base_url,
        &default_headers,
        &list_request,
        &list_response,
        &[],
    )
    .await
    .ok()
    .and_then(|channels| {
        channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .cloned()
    });

    // ── DRM detection and extraction ──────────────────────────────────────
    //
    // Check for DRM FIRST before extracting stream URL, because some channels
    // are DRM-only and don't have an HLS fallback URL.
    //
    // When the provider YAML defines a `drm` block with an `is_drm` JSON path,
    // we check the upstream response for a truthy value.  If the channel is
    // DRM-protected:
    //   - Use the MPD URL as the primary stream URL if HLS URL is missing.
    //   - Override the stream type to Dash.
    //   - Collect DRM license proxy fields for ProxyContext.

    let drm_cfg = stream_endpoint.response.drm.as_ref();

    // Determine if this channel uses DRM.
    let is_drm = drm_cfg
        .and_then(|cfg| cfg.is_drm.as_ref())
        .and_then(|path| extract_json_path(&response, path))
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    // Extract stream URL - for DRM-only channels, fall back to MPD URL if HLS URL is missing
    let mut stream_url = extract_json_path(&response, &stream_endpoint.response.url)
        .or_else(|| {
            if is_drm {
                // Try to extract MPD URL as fallback for DRM-only channels
                drm_cfg
                    .and_then(|cfg| cfg.mpd_url.as_ref())
                    .and_then(|mpd_path| extract_json_path(&response, mpd_path))
            } else {
                None
            }
        })
        .ok_or_else(|| {
            error!(
                channel_id = %channel_id,
                provider   = %provider_id,
                url_path   = %stream_endpoint.response.url,
                is_drm     = is_drm,
                "Stream URL not found in response (neither HLS nor MPD URL available)"
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

    // For DRM-only channels where we used MPD URL as fallback, force stream type to DASH
    let mut stream_type =
        if is_drm && extract_json_path(&response, &stream_endpoint.response.url).is_none() {
            StreamType::Dash
        } else {
            match stream_type_str.to_lowercase().as_str() {
                "dash" | "mpd" => StreamType::Dash,
                _ => StreamType::Hls,
            }
        };

    // Extract DRM info and collect license-proxy fields.
    let mut drm_license_url: Option<String> = None;
    let mut drm_license_headers: Option<HashMap<String, String>> = None;
    let mut drm_license_cookies: Option<Vec<String>> = None;
    let mut drm_prefetch_url: Option<String> = None;

    let drm = if let Some(cfg) = drm_cfg {
        // Treat `system` as a literal when it doesn't look like a JSON path.
        let system = if cfg.system.starts_with("$.") {
            extract_json_path(&response, &cfg.system).unwrap_or_default()
        } else {
            cfg.system.clone()
        };
        let license_url = extract_json_path(&response, &cfg.license_url)
            .unwrap_or_else(|| context.resolve_lossy(&cfg.license_url));
        let mut headers_map = HashMap::new();
        for (k, v) in &cfg.headers {
            headers_map.insert(k.clone(), context.resolve_lossy(v));
        }

        if is_drm {
            // For hybrid channels that have both HLS and MPD URLs, prefer MPD.
            // (DRM-only channels already used MPD URL from the fallback logic above.)
            if let Some(mpd_path) = &cfg.mpd_url
                && let Some(mpd) = extract_json_path(&response, mpd_path)
                && mpd != stream_url
            // Only override if different from current URL
            {
                debug!(
                    channel_id = %channel_id,
                    mpd_url = %mpd,
                    "DRM channel — preferring MPD URL over HLS URL"
                );
                stream_url = mpd;
                stream_type = StreamType::Dash;
            }

            // Store DRM license proxy fields for ProxyContext.
            drm_license_url = Some(license_url.clone());
            drm_license_headers = Some(headers_map.clone());
            if !cfg.cookies.is_empty() {
                drm_license_cookies = Some(cfg.cookies.clone());
            }
            if let Some(prefetch) = &cfg.prefetch_url {
                // `prefetch_url` may be a JSON path (e.g. "$.mpd.result") or
                // a template.  Try JSON path extraction first.
                let resolved = if prefetch.starts_with("$.") {
                    extract_json_path(&response, prefetch)
                        .unwrap_or_else(|| context.resolve_lossy(prefetch))
                } else {
                    context.resolve_lossy(prefetch)
                };
                drm_prefetch_url = Some(resolved);
            }
        }

        Some(DrmInfo {
            system,
            license_url,
            headers: headers_map,
        })
    } else {
        None
    };

    // Proxy the stream URL through our backend to avoid CORS issues.
    // Build a ProxyContext with resolved headers / cookie mappings, store it
    // server-side under an opaque UUID, and embed only the token in the URL.
    let (proxied_url, ctx_token) = {
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

        // Always create a ProxyContext so the ctx token acts as a server-issued
        // capability; the proxy requires ctx on every request to prevent open-proxy
        // / SSRF abuse.
        let ctx = crate::state::ProxyContext {
            upstream_url: stream_url.clone(),
            headers: resolved_headers,
            allowed_hosts: allowed_hosts_from_url(&stream_url),
            url_param_cookies,
            resolved_cookies: Default::default(),
            static_cookies,
            manifest_query: None,
            append_manifest_query_to_key_uris: stream_endpoint.append_manifest_query_to_key_uris,
            key_exclude_resolved_cookies: stream_endpoint.key_exclude_resolved_cookies,
            key_uri_patterns: stream_endpoint.key_uri_patterns.clone(),
            stream_type: stream_type.clone(),
            drm_license_url,
            drm_license_headers,
            drm_license_cookies,
            drm_prefetch_url,
        };
        let token = uuid::Uuid::new_v4().to_string();
        state.proxy_ctx.insert(token.clone(), ctx).await;
        let url = format!(
            "/api/proxy?url={}&ctx={token}",
            urlencoding::encode(&stream_url)
        );
        (url, token)
    };

    // When DRM is active, replace the raw upstream license URL in the
    // response with our server-side DRM license proxy endpoint so that the
    // player sends license requests through us (where we attach auth
    // headers/cookies the browser cannot).
    let drm = drm.map(|mut info| {
        if is_drm {
            info.license_url = format!("/api/proxy/drm/{ctx_token}");
        }
        info
    });

    Ok(Json(StreamInfo {
        url: proxied_url,
        stream_type,
        drm,
        channel_name: channel_meta.as_ref().map(|channel| channel.name.clone()),
        channel_logo: channel_meta.and_then(|channel| channel.logo),
    }))
}

// ── Response mapping helpers ───────────────────────────────────────────────

fn map_channels(
    response: &Value,
    mapping: &otvi_core::config::ResponseMapping,
) -> Result<Vec<Channel>, AppError> {
    let items = get_items_array(response, mapping.items_path.as_deref())?;

    let logo_base = mapping.logo_base_url.as_deref().unwrap_or("");

    let channels = items
        .iter()
        .filter_map(|item| {
            let id = extract_mapped_field(item, &mapping.mapping, "id")?;
            let name = extract_mapped_field(item, &mapping.mapping, "name")?;
            let logo = extract_mapped_field(item, &mapping.mapping, "logo").map(|raw| {
                if raw.starts_with("http://") || raw.starts_with("https://") {
                    raw
                } else {
                    format!("{logo_base}{raw}")
                }
            });
            let category = extract_mapped_field(item, &mapping.mapping, "category");
            let number = extract_mapped_field(item, &mapping.mapping, "number");
            let description = extract_mapped_field(item, &mapping.mapping, "description");
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
    let items = get_items_array(response, mapping.items_path.as_deref())?;

    let categories = items
        .iter()
        .filter_map(|item| {
            let id = extract_mapped_field(item, &mapping.mapping, "id")?;
            let name = extract_mapped_field(item, &mapping.mapping, "name")?;
            Some(Category { id, name })
        })
        .collect();

    Ok(categories)
}

#[allow(clippy::too_many_arguments)]
async fn load_all_channels(
    state: &Arc<AppState>,
    provider_id: &str,
    user_id: &str,
    cache_key: &ChannelCacheKey,
    base_url: &str,
    default_headers: &HashMap<String, String>,
    list_request: &otvi_core::config::RequestSpec,
    list_response: &otvi_core::config::ResponseMapping,
    extra_context: &[(&str, &str)],
) -> Result<Arc<[Channel]>, AppError> {
    if let Some(cached) = state.channel_cache.channels.get(cache_key).await {
        debug!(provider = %provider_id, "channel list cache HIT");
        return Ok(cached.channels);
    }

    debug!(provider = %provider_id, "channel list cache MISS — fetching from upstream");

    let http_client = state.http_client.clone();
    let base_url = base_url.to_owned();
    let default_headers = default_headers.clone();
    let list_request = list_request.clone();
    let extra: Vec<(String, String)> = extra_context
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let resp = with_refresh_retry(state, provider_id, user_id, |mut ctx| {
        let http_client = http_client.clone();
        let base_url = base_url.clone();
        let default_headers = default_headers.clone();
        let list_request = list_request.clone();
        let extra = extra.clone();
        async move {
            for (k, v) in &extra {
                ctx.set(k, v);
            }
            provider_client::execute_request_raw(
                &http_client,
                &base_url,
                &default_headers,
                &list_request,
                &ctx,
            )
            .await
        }
    })
    .await?;

    if !(200..300).contains(&resp.status) {
        error!(
            provider = %provider_id,
            status = resp.status,
            "Upstream channel list error after refresh retry"
        );
        return Err(AppError::Internal(format!(
            "Upstream channel list returned status {}",
            resp.status
        )));
    }

    let response = resp.body;

    let channels = Arc::<[Channel]>::from(map_channels(&response, list_response)?);

    state
        .channel_cache
        .channels
        .insert(
            cache_key.clone(),
            CachedChannels {
                channels: channels.clone(),
            },
        )
        .await;

    Ok(channels)
}

fn allowed_hosts_from_url(url: &str) -> Vec<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .into_iter()
        .collect()
}

/// Navigate to the array indicated by `items_path` in the response JSON and
/// return it by reference.
///
/// When `items_path` is `None` the response itself is expected to be a JSON
/// array.  When a path is given it is resolved via [`navigate_json`] and
/// the matched node is expected to be an array.
fn get_items_array<'a>(
    response: &'a Value,
    items_path: Option<&str>,
) -> Result<&'a [Value], AppError> {
    match items_path {
        Some(path) => {
            let node = select_json_path_value(response, path).ok_or_else(|| {
                AppError::Internal(format!("items_path '{path}' not found in response"))
            })?;
            node.as_array()
                .map(Vec::as_slice)
                .ok_or_else(|| AppError::Internal(format!("items_path '{path}' is not an array")))
        }
        None => response
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| AppError::Internal("Response root is not an array".into())),
    }
}

/// Walk a JSON value using a simple dot-notation path (with `$.` prefix
/// stripped).  Returns a reference to the node at the path, or `None`.
///
/// This is used instead of `extract_json_path` in contexts where we need the
/// raw `Value` node rather than a scalar string.
#[cfg(test)]
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
    use std::time::Duration;

    use crate::state::{
        CacheScope, CachedCategories, CachedChannels, ChannelCache, ChannelCacheKey,
    };

    // ── Helpers ──────────────────────────────────────────────────────────

    fn channel(id: &str, name: &str, category: Option<&str>) -> Channel {
        Channel {
            id: id.to_string(),
            name: name.to_string(),
            logo: None,
            category: category.map(str::to_string),
            number: None,
            description: None,
        }
    }

    fn mapping_for(
        items_path: Option<&str>,
        id_path: &str,
        name_path: &str,
    ) -> otvi_core::config::ResponseMapping {
        let mut m = HashMap::new();
        m.insert("id".into(), id_path.to_string());
        m.insert("name".into(), name_path.to_string());
        otvi_core::config::ResponseMapping {
            items_path: items_path.map(str::to_string),
            mapping: m,
            logo_base_url: None,
        }
    }

    // ── CacheScope from handler logic ─────────────────────────────────────

    #[test]
    fn cache_scope_global_produces_global_key() {
        let key = ChannelCacheKey::from_auth_scope(
            "prov",
            &otvi_core::config::AuthScope::Global,
            "user-42",
        );
        assert_eq!(key.scope, CacheScope::Global);
    }

    #[test]
    fn cache_scope_per_user_produces_per_user_key() {
        let key = ChannelCacheKey::from_auth_scope(
            "prov",
            &otvi_core::config::AuthScope::PerUser,
            "user-42",
        );
        assert_eq!(key.scope, CacheScope::PerUser("user-42".into()));
    }

    #[test]
    fn cache_scope_global_same_key_for_any_user_id() {
        // Two different users looking up a Global provider must produce
        // the same cache key so they share the single upstream entry.
        let key_a = ChannelCacheKey::from_auth_scope(
            "prov",
            &otvi_core::config::AuthScope::Global,
            "alice",
        );
        let key_b =
            ChannelCacheKey::from_auth_scope("prov", &otvi_core::config::AuthScope::Global, "bob");
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn cache_scope_per_user_different_users_produce_different_keys() {
        let key_a = ChannelCacheKey::from_auth_scope(
            "prov",
            &otvi_core::config::AuthScope::PerUser,
            "alice",
        );
        let key_b =
            ChannelCacheKey::from_auth_scope("prov", &otvi_core::config::AuthScope::PerUser, "bob");
        assert_ne!(key_a, key_b);
    }

    // ── extract_mapped_field ──────────────────────────────────────────────

    #[test]
    fn test_extract_mapped_field_found() {
        let item = json!({ "title": "BBC One", "channel_id": "bbc1" });
        let mut mapping = HashMap::new();
        mapping.insert("name".into(), "$.title".into());
        mapping.insert("id".into(), "$.channel_id".into());

        assert_eq!(
            extract_mapped_field(&item, &mapping, "name"),
            Some("BBC One".to_string())
        );
        assert_eq!(
            extract_mapped_field(&item, &mapping, "id"),
            Some("bbc1".to_string())
        );
    }

    #[test]
    fn test_extract_mapped_field_not_found() {
        let item = json!({ "title": "BBC One" });
        let mapping: HashMap<String, String> = HashMap::new();
        assert_eq!(extract_mapped_field(&item, &mapping, "name"), None);
    }

    #[test]
    fn test_extract_mapped_field_path_missing_in_item() {
        let item = json!({ "other": "value" });
        let mut mapping = HashMap::new();
        mapping.insert("name".into(), "$.title".into());
        // The mapping key exists but the path doesn't resolve in the item.
        assert_eq!(extract_mapped_field(&item, &mapping, "name"), None);
    }

    // ── map_channels ──────────────────────────────────────────────────────

    #[test]
    fn test_map_channels_basic() {
        let response = json!({
            "channels": [
                { "id": "1", "title": "Channel One", "cat": "news" },
                { "id": "2", "title": "Channel Two", "cat": "sports" },
            ]
        });
        let mut m = HashMap::new();
        m.insert("id".into(), "$.id".into());
        m.insert("name".into(), "$.title".into());
        m.insert("category".into(), "$.cat".into());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: Some("$.channels".into()),
            mapping: m,
            logo_base_url: None,
        };

        let channels = map_channels(&response, &mapping).unwrap();
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].id, "1");
        assert_eq!(channels[0].name, "Channel One");
        assert_eq!(channels[0].category.as_deref(), Some("news"));
        assert_eq!(channels[1].id, "2");
        assert_eq!(channels[1].name, "Channel Two");
    }

    #[test]
    fn test_map_channels_with_logo_base_url() {
        let response = json!([{ "id": "1", "name": "Test", "logo": "/logos/test.png" }]);
        let mut m = HashMap::new();
        m.insert("id".into(), "$.id".into());
        m.insert("name".into(), "$.name".into());
        m.insert("logo".into(), "$.logo".into());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: m,
            logo_base_url: Some("https://cdn.example.com".into()),
        };

        let channels = map_channels(&response, &mapping).unwrap();
        assert_eq!(
            channels[0].logo.as_deref(),
            Some("https://cdn.example.com/logos/test.png")
        );
    }

    #[test]
    fn test_map_channels_with_absolute_logo() {
        let response =
            json!([{ "id": "1", "name": "Test", "logo": "https://external.com/logo.png" }]);
        let mut m = HashMap::new();
        m.insert("id".into(), "$.id".into());
        m.insert("name".into(), "$.name".into());
        m.insert("logo".into(), "$.logo".into());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: m,
            logo_base_url: Some("https://cdn.example.com".into()),
        };

        let channels = map_channels(&response, &mapping).unwrap();
        // Absolute URLs must not be prefixed with logo_base_url.
        assert_eq!(
            channels[0].logo.as_deref(),
            Some("https://external.com/logo.png")
        );
    }

    #[test]
    fn test_map_channels_missing_required_fields_skips_item() {
        // Items that have no "id" mapping should be silently skipped.
        let response = json!([
            { "name": "No ID Channel" },
            { "id": "2", "name": "Valid Channel" },
        ]);
        let mapping = mapping_for(None, "$.id", "$.name");
        let channels = map_channels(&response, &mapping).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].id, "2");
    }

    #[test]
    fn test_map_channels_empty_array() {
        let response = json!([]);
        let mapping = mapping_for(None, "$.id", "$.name");
        let channels = map_channels(&response, &mapping).unwrap();
        assert!(channels.is_empty());
    }

    #[test]
    fn test_map_channels_items_path_not_found_returns_error() {
        let response = json!({ "other": [] });
        let mapping = mapping_for(Some("$.channels"), "$.id", "$.name");
        assert!(map_channels(&response, &mapping).is_err());
    }

    #[test]
    fn test_map_channels_items_path_not_array_returns_error() {
        let response = json!({ "channels": "not-an-array" });
        let mapping = mapping_for(Some("$.channels"), "$.id", "$.name");
        assert!(map_channels(&response, &mapping).is_err());
    }

    #[test]
    fn test_map_channels_root_not_array_without_path_returns_error() {
        let response = json!({ "channels": [] });
        let mapping = mapping_for(None, "$.id", "$.name");
        assert!(map_channels(&response, &mapping).is_err());
    }

    // ── map_categories ────────────────────────────────────────────────────

    #[test]
    fn test_map_categories() {
        let response = json!([
            { "id": "1", "label": "News" },
            { "id": "2", "label": "Sports" },
        ]);
        let mut m = HashMap::new();
        m.insert("id".into(), "$.id".into());
        m.insert("name".into(), "$.label".into());
        let mapping = otvi_core::config::ResponseMapping {
            items_path: None,
            mapping: m,
            logo_base_url: None,
        };

        let cats = map_categories(&response, &mapping).unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0].id, "1");
        assert_eq!(cats[0].name, "News");
    }

    #[test]
    fn test_map_categories_missing_fields_skips_item() {
        let response = json!([
            { "name": "No ID" },
            { "id": "2", "name": "Valid" },
        ]);
        let mapping = mapping_for(None, "$.id", "$.name");
        let cats = map_categories(&response, &mapping).unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].id, "2");
    }

    // ── navigate_json ─────────────────────────────────────────────────────

    #[test]
    fn navigate_json_root_dollar() {
        let v = json!({"a": 1});
        assert_eq!(navigate_json(&v, "$"), Some(&v));
    }

    #[test]
    fn navigate_json_simple_key() {
        let v = json!({"a": {"b": 42}});
        assert_eq!(navigate_json(&v, "$.a.b"), Some(&json!(42)));
    }

    #[test]
    fn navigate_json_array_index() {
        let v = json!({"items": [10, 20, 30]});
        assert_eq!(navigate_json(&v, "$.items[1]"), Some(&json!(20)));
    }

    #[test]
    fn navigate_json_missing_key_returns_none() {
        let v = json!({"a": 1});
        assert_eq!(navigate_json(&v, "$.b"), None);
    }

    #[test]
    fn navigate_json_out_of_bounds_index_returns_none() {
        let v = json!({"items": [1]});
        assert_eq!(navigate_json(&v, "$.items[5]"), None);
    }

    // ── get_items_array ───────────────────────────────────────────────────

    #[test]
    fn get_items_array_no_path_uses_root() {
        let v = json!([{"id": "1"}, {"id": "2"}]);
        let items = get_items_array(&v, None).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn get_items_array_with_path() {
        let v = json!({"data": [{"id": "1"}]});
        let items = get_items_array(&v, Some("$.data")).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn get_items_array_path_not_found_returns_error() {
        let v = json!({"data": []});
        assert!(get_items_array(&v, Some("$.missing")).is_err());
    }

    #[test]
    fn get_items_array_root_not_array_returns_error() {
        let v = json!({"key": "value"});
        assert!(get_items_array(&v, None).is_err());
    }

    // ── ChannelListQuery defaults ─────────────────────────────────────────

    #[test]
    fn channel_list_query_defaults_to_all() {
        let q = ChannelListQuery::default();
        assert!(q.category.is_none());
        assert!(q.search.is_none());
        assert!(q.limit.is_none());
        assert!(q.offset.is_none());
    }

    // ── Server-side filtering logic ───────────────────────────────────────

    #[test]
    fn search_filter_case_insensitive() {
        let channels = vec![
            channel("1", "BBC News", None),
            channel("2", "Sky Sports", None),
            channel("3", "CNN INTERNATIONAL", None),
        ];

        let term = "bbc";
        let term_lower = term.to_lowercase();
        let filtered: Vec<_> = channels
            .into_iter()
            .filter(|ch| ch.name.to_lowercase().contains(&term_lower))
            .collect();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "1");
    }

    #[test]
    fn search_filter_empty_term_returns_all() {
        let channels = vec![
            channel("1", "BBC News", None),
            channel("2", "Sky Sports", None),
        ];
        let term = "";
        // Empty term should not filter anything.
        let filtered: Vec<_> = if !term.is_empty() {
            let t = term.to_lowercase();
            channels
                .into_iter()
                .filter(|ch| ch.name.to_lowercase().contains(&t))
                .collect()
        } else {
            channels
        };
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn category_filter_exact_match() {
        let channels = vec![
            channel("1", "BBC News", Some("news")),
            channel("2", "Sky Sports", Some("sports")),
            channel("3", "Al Jazeera", Some("news")),
        ];
        let filtered: Vec<_> = channels
            .into_iter()
            .filter(|ch| ch.category.as_deref() == Some("news"))
            .collect();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn pagination_limit_and_offset() {
        let channels: Vec<Channel> = (1..=10)
            .map(|i| channel(&i.to_string(), &format!("Ch {i}"), None))
            .collect();

        let paged: Vec<_> = channels.into_iter().skip(2).take(3).collect();
        assert_eq!(paged.len(), 3);
        assert_eq!(paged[0].id, "3");
        assert_eq!(paged[2].id, "5");
    }

    #[test]
    fn pagination_offset_only_skips_items() {
        let channels: Vec<Channel> = (1..=5)
            .map(|i| channel(&i.to_string(), &format!("Ch {i}"), None))
            .collect();

        let paged: Vec<_> = channels.into_iter().skip(3).collect();
        assert_eq!(paged.len(), 2);
        assert_eq!(paged[0].id, "4");
    }

    #[test]
    fn pagination_offset_beyond_total_returns_empty() {
        let channels: Vec<Channel> = (1..=3)
            .map(|i| channel(&i.to_string(), &format!("Ch {i}"), None))
            .collect();

        let paged: Vec<_> = channels.into_iter().skip(10).take(5).collect();
        assert!(paged.is_empty());
    }

    // ── ChannelCache integration tests ────────────────────────────────────

    #[tokio::test]
    async fn cache_miss_on_empty_cache() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::per_user("provider-a", "user-1");
        assert!(cache.channels.get(&key).await.is_none());
        assert!(cache.categories.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn cache_hit_after_channel_insert() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::per_user("provider-a", "user-1");
        let payload = CachedChannels {
            channels: vec![channel("ch1", "Channel 1", None)].into(),
        };
        cache.channels.insert(key.clone(), payload).await;

        let hit = cache.channels.get(&key).await;
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.channels.len(), 1);
        assert_eq!(hit.channels[0].id, "ch1");
    }

    #[tokio::test]
    async fn cache_hit_after_category_insert() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::global("provider-a");
        let payload = CachedCategories {
            categories: vec![
                Category {
                    id: "1".into(),
                    name: "News".into(),
                },
                Category {
                    id: "2".into(),
                    name: "Sports".into(),
                },
            ]
            .into(),
        };
        cache.categories.insert(key.clone(), payload).await;

        let hit = cache.categories.get(&key).await;
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().categories.len(), 2);
    }

    #[tokio::test]
    async fn cache_global_scope_single_entry_for_all_users() {
        // Global-scoped providers must share a single cache entry for all users.
        let cache = ChannelCache::new(Duration::from_secs(60));
        let global_key = ChannelCacheKey::global("prov");

        cache
            .channels
            .insert(
                global_key.clone(),
                CachedChannels {
                    channels: Vec::new().into(),
                },
            )
            .await;

        // Both user-a and user-b resolve to the same global key and must see the entry.
        assert!(cache.channels.get(&global_key).await.is_some());
        // A per-user key must NOT collide with the global key.
        let per_user_key = ChannelCacheKey::per_user("prov", "user-a");
        assert!(cache.channels.get(&per_user_key).await.is_none());
    }

    #[tokio::test]
    async fn cache_per_user_isolation() {
        // Per-user providers must not share cache entries across users.
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key_a = ChannelCacheKey::per_user("prov", "user-a");
        let key_b = ChannelCacheKey::per_user("prov", "user-b");

        cache
            .channels
            .insert(
                key_a.clone(),
                CachedChannels {
                    channels: vec![channel("1", "User A Channel", None)].into(),
                },
            )
            .await;

        let hit_a = cache.channels.get(&key_a).await;
        let hit_b = cache.channels.get(&key_b).await;

        assert!(hit_a.is_some());
        assert!(hit_b.is_none());
    }

    #[tokio::test]
    async fn cache_invalidate_clears_both_channel_and_category() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::per_user("prov", "uid");

        cache
            .channels
            .insert(
                key.clone(),
                CachedChannels {
                    channels: Vec::new().into(),
                },
            )
            .await;
        cache
            .categories
            .insert(
                key.clone(),
                CachedCategories {
                    categories: Vec::new().into(),
                },
            )
            .await;

        assert!(cache.channels.get(&key).await.is_some());
        assert!(cache.categories.get(&key).await.is_some());

        cache.invalidate(&key).await;

        assert!(cache.channels.get(&key).await.is_none());
        assert!(cache.categories.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn cache_invalidate_does_not_affect_other_provider() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key_target = ChannelCacheKey::per_user("prov-a", "uid");
        let key_other = ChannelCacheKey::per_user("prov-b", "uid");

        cache
            .channels
            .insert(
                key_target.clone(),
                CachedChannels {
                    channels: Vec::new().into(),
                },
            )
            .await;
        cache
            .channels
            .insert(
                key_other.clone(),
                CachedChannels {
                    channels: Vec::new().into(),
                },
            )
            .await;

        cache.invalidate(&key_target).await;

        assert!(
            cache.channels.get(&key_target).await.is_none(),
            "prov-a should be evicted"
        );
        assert!(
            cache.channels.get(&key_other).await.is_some(),
            "prov-b should remain"
        );
    }

    #[tokio::test]
    async fn cache_invalidate_does_not_affect_other_uid_same_provider() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key_a = ChannelCacheKey::per_user("prov", "uid-a");
        let key_b = ChannelCacheKey::per_user("prov", "uid-b");

        cache
            .channels
            .insert(
                key_a.clone(),
                CachedChannels {
                    channels: Vec::new().into(),
                },
            )
            .await;
        cache
            .channels
            .insert(
                key_b.clone(),
                CachedChannels {
                    channels: Vec::new().into(),
                },
            )
            .await;

        cache.invalidate(&key_a).await;

        assert!(cache.channels.get(&key_a).await.is_none());
        assert!(cache.channels.get(&key_b).await.is_some());
    }

    #[tokio::test]
    async fn cache_evicts_after_ttl() {
        let cache = ChannelCache::new(Duration::from_millis(1));
        let key = ChannelCacheKey::global("prov");

        cache
            .channels
            .insert(
                key.clone(),
                CachedChannels {
                    channels: Vec::new().into(),
                },
            )
            .await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        // moka evicts lazily on access; the entry should be gone after the TTL.
        assert!(cache.channels.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn cache_overwrite_updates_value() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::per_user("prov", "uid");

        cache
            .channels
            .insert(
                key.clone(),
                CachedChannels {
                    channels: vec![channel("1", "Old", None)].into(),
                },
            )
            .await;
        cache
            .channels
            .insert(
                key.clone(),
                CachedChannels {
                    channels: vec![channel("2", "New A", None), channel("3", "New B", None)].into(),
                },
            )
            .await;

        let hit = cache.channels.get(&key).await.unwrap();
        assert_eq!(hit.channels.len(), 2);
        assert_eq!(hit.channels[0].id, "2");
    }

    #[tokio::test]
    async fn cache_multiple_providers_coexist() {
        let cache = ChannelCache::new(Duration::from_secs(60));

        for i in 0..5u32 {
            let key = ChannelCacheKey::per_user(format!("provider-{i}"), "uid");
            let payload = CachedChannels {
                channels: vec![channel(&i.to_string(), &format!("Ch {i}"), None)].into(),
            };
            cache.channels.insert(key, payload).await;
        }

        for i in 0..5u32 {
            let key = ChannelCacheKey::per_user(format!("provider-{i}"), "uid");
            let hit = cache.channels.get(&key).await;
            assert!(hit.is_some(), "provider-{i} should be cached");
            assert_eq!(hit.unwrap().channels[0].id, i.to_string());
        }
    }
}
