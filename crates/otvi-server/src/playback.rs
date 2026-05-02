use std::collections::HashMap;
use std::sync::Arc;

use otvi_core::template::extract_json_path;
use otvi_core::types::{DrmInfo, StreamInfo, StreamType};
use tracing::{debug, error};

use crate::api::auth::{build_provider_context, with_refresh_retry};
use crate::api::channels::load_all_channels;
use crate::api::provider_access::authorize_provider_route;
use crate::auth_middleware::ActiveClaims;
use crate::error::AppError;
use crate::provider_client;
use crate::state::{AppState, ChannelCacheKey, ProxyContext};

/// Resolve a playable stream for one provider channel.
///
/// This keeps playback rules behind one interface: provider auth scope,
/// upstream playback call, token refresh, DRM extraction, channel metadata, and
/// proxy-context creation all move together.
pub async fn resolve_stream(
    state: &Arc<AppState>,
    claims: &ActiveClaims,
    provider_id: &str,
    channel_id: &str,
) -> Result<StreamInfo, AppError> {
    let scope = authorize_provider_route(state, claims, provider_id, false).await?;

    let provider_data = state
        .with_provider(provider_id, |p| {
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

    let uid = match scope {
        otvi_core::config::AuthScope::Global => String::new(),
        otvi_core::config::AuthScope::PerUser => claims.sub.clone(),
    };
    let http_client = state.http_client.clone();
    let stream_base_url = base_url.clone();
    let stream_default_headers = default_headers.clone();
    let stream_request = stream_endpoint.request.clone();
    let channel_id_for_call = channel_id.to_owned();

    let resp = with_refresh_retry(state, provider_id, &uid, |mut ctx| {
        let http_client = http_client.clone();
        let stream_base_url = stream_base_url.clone();
        let stream_default_headers = stream_default_headers.clone();
        let stream_request = stream_request.clone();
        let channel_id_for_call = channel_id_for_call.clone();
        async move {
            ctx.set("input.channel_id", &channel_id_for_call);
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

    let mut context = build_provider_context(state, &uid, provider_id).await?;
    context.set("input.channel_id", channel_id);

    let cache_key = ChannelCacheKey::from_auth_scope(provider_id, &scope, &claims.sub);
    let channel_meta = load_all_channels(
        state,
        provider_id,
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

    let drm_cfg = stream_endpoint.response.drm.as_ref();
    let is_drm = drm_cfg
        .and_then(|cfg| cfg.is_drm.as_ref())
        .and_then(|path| extract_json_path(&response, path))
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    let mut stream_url = extract_json_path(&response, &stream_endpoint.response.url)
        .or_else(|| {
            if is_drm {
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
                provider = %provider_id,
                url_path = %stream_endpoint.response.url,
                is_drm = is_drm,
                "Stream URL not found in response (neither HLS nor MPD URL available)"
            );
            AppError::Internal("Stream URL not found in response".into())
        })?;

    let stream_type_raw = &stream_endpoint.response.stream_type;
    let stream_type_str = if stream_type_raw.starts_with("$.") {
        extract_json_path(&response, stream_type_raw).unwrap_or_else(|| "hls".to_string())
    } else {
        stream_type_raw.clone()
    };

    let mut stream_type =
        if is_drm && extract_json_path(&response, &stream_endpoint.response.url).is_none() {
            StreamType::Dash
        } else {
            match stream_type_str.to_lowercase().as_str() {
                "dash" | "mpd" => StreamType::Dash,
                _ => StreamType::Hls,
            }
        };

    let mut drm_license_url: Option<String> = None;
    let mut drm_license_headers: Option<HashMap<String, String>> = None;
    let mut drm_license_cookies: Option<Vec<String>> = None;
    let mut drm_prefetch_url: Option<String> = None;

    let drm = if let Some(cfg) = drm_cfg {
        let system = if cfg.system.starts_with("$.") {
            extract_json_path(&response, &cfg.system).unwrap_or_default()
        } else {
            cfg.system.clone()
        };
        let license_url = extract_json_path(&response, &cfg.license_url)
            .unwrap_or_else(|| context.resolve_lossy(&cfg.license_url));
        let headers_map = cfg
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), context.resolve_lossy(v)))
            .collect::<HashMap<_, _>>();

        if is_drm {
            if let Some(mpd_path) = &cfg.mpd_url
                && let Some(mpd) = extract_json_path(&response, mpd_path)
                && mpd != stream_url
            {
                debug!(
                    channel_id = %channel_id,
                    mpd_url = %mpd,
                    "DRM channel - preferring MPD URL over HLS URL"
                );
                stream_url = mpd;
                stream_type = StreamType::Dash;
            }

            drm_license_url = Some(license_url.clone());
            drm_license_headers = Some(headers_map.clone());
            if !cfg.cookies.is_empty() {
                drm_license_cookies = Some(cfg.cookies.clone());
            }
            if let Some(prefetch) = &cfg.prefetch_url {
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

    let (proxied_url, ctx_token) = {
        let resolved_headers = stream_endpoint
            .proxy_headers
            .iter()
            .map(|(k, v)| (k.clone(), context.resolve_lossy(v)))
            .collect();
        let static_cookies = stream_endpoint
            .proxy_cookies
            .iter()
            .map(|(k, v)| (k.clone(), context.resolve_lossy(v)))
            .collect();

        let ctx = ProxyContext {
            upstream_url: stream_url.clone(),
            headers: resolved_headers,
            allowed_hosts: allowed_hosts_from_url(&stream_url),
            url_param_cookies: stream_endpoint.proxy_url_cookies.clone(),
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

    let drm = drm.map(|mut info| {
        if is_drm {
            info.license_url = format!("/api/proxy/drm/{ctx_token}");
        }
        info
    });

    Ok(StreamInfo {
        url: proxied_url,
        stream_type,
        drm,
        channel_name: channel_meta.as_ref().map(|channel| channel.name.clone()),
        channel_logo: channel_meta.and_then(|channel| channel.logo),
    })
}

fn allowed_hosts_from_url(url: &str) -> Vec<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .into_iter()
        .collect()
}
