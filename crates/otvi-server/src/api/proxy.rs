//! HLS / DASH stream proxy.
//!
//! Fetches upstream content on behalf of the browser (avoiding CORS issues)
//! and rewrites relative URLs inside `.m3u8` playlists so that subsequent
//! requests also go through this proxy.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tracing::debug;
use url::Url;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct ProxyQuery {
    pub url: String,
    /// Opaque proxy-context token issued by the stream endpoint.  The server
    /// looks up the associated headers internally; nothing sensitive travels
    /// in the URL.
    pub ctx: Option<String>,
}

/// `GET /api/proxy?url=<upstream_url>`
///
/// Fetches `url` from the upstream CDN and returns its body to the browser.
/// If the response is an m3u8 playlist, relative/absolute URLs inside it are
/// rewritten to go through this same proxy endpoint.
pub async fn proxy_stream(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ProxyQuery>,
) -> Result<Response, (StatusCode, String)> {
    let upstream_url = &query.url;

    // Validate URL
    let parsed = Url::parse(upstream_url)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid URL: {e}")))?;

    // Look up the proxy context for this token.
    let ctx = query
        .ctx
        .as_deref()
        .and_then(|token| {
            state
                .proxy_ctx
                .read()
                .ok()
                .and_then(|store| store.get(token).cloned())
        })
        .unwrap_or_default();

    // Fetch upstream
    let mut req = state.http_client.get(parsed.as_str());
    // Apply provider-specified headers (override default UA when present)
    for (k, v) in &ctx.headers {
        req = req.header(k.as_str(), v.as_str());
    }
    // Fall back to a generic UA only when none was supplied
    if !ctx.headers.keys().any(|k| k.to_lowercase() == "user-agent") {
        req = req.header("User-Agent", "Mozilla/5.0");
    }
    // Generic URL-param → cookie forwarding.
    // Two sources are combined:
    //  1. Params extracted from the *current* upstream URL query string.
    //     `url::Url::query_pairs()` is used so that percent-encoded values
    //     (e.g. `%3D`, `%7E` inside an Akamai `hdnea` token) are decoded
    //     before being forwarded as cookie values.
    //  2. Cookie values previously persisted in the context from when a
    //     manifest URL (which carries the auth token as a query param) was
    //     fetched.  Sub-requests like bare `.pkey` key files have no query
    //     params at all, so source (2) is essential for them.
    if !ctx.url_param_cookies.is_empty() || !ctx.resolved_cookies.is_empty() || !ctx.static_cookies.is_empty() {
        // Decode all query pairs once so we can look them up cheaply.
        let query_map: std::collections::HashMap<String, String> = parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        let mut cookie_pairs: Vec<String> = Vec::new();

        // Source 1 – params in the current URL (freshest value wins)
        for (param, cookie_name) in &ctx.url_param_cookies {
            if let Some(val) = query_map.get(param.as_str()) {
                cookie_pairs.push(format!("{cookie_name}={val}"));
            }
        }
        // Source 2 – persisted values from a prior manifest fetch (fill gaps)
        for (cookie_name, val) in &ctx.resolved_cookies {
            let already = cookie_pairs.iter().any(|p| p.starts_with(&format!("{cookie_name}=")));
            if !already {
                cookie_pairs.push(format!("{cookie_name}={val}"));
            }
        }
        // Source 3 – static cookies from provider YAML `proxy_cookies` field
        // (lowest priority; overridden by URL-extracted or manifest-extracted values)
        for (cookie_name, val) in &ctx.static_cookies {
            let already = cookie_pairs.iter().any(|p| p.starts_with(&format!("{cookie_name}=")));
            if !already {
                cookie_pairs.push(format!("{cookie_name}={val}"));
            }
        }
        let cookie_header = cookie_pairs.join("; ");
        debug!(
            url = %upstream_url,
            cookie = %cookie_header,
            resolved_cookies = ?ctx.resolved_cookies,
            static_cookies_count = ctx.static_cookies.len(),
            manifest_query_present = ctx.manifest_query.is_some(),
            "proxy cookie state"
        );
        if !cookie_header.is_empty() {
            req = req.header("Cookie", cookie_header);
        }
    }
    let resp = req.send().await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("Failed to fetch upstream: {e}"),
            )
        })?;

    let upstream_status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"));

    let body_bytes = resp.bytes().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Failed to read upstream body: {e}"),
        )
    })?;

    // If the response is an m3u8 playlist, rewrite URLs
    let ct_str = content_type.to_str().unwrap_or("");
    let is_m3u8 = ct_str.contains("mpegurl")
        || ct_str.contains("x-mpegurl")
        || upstream_url.contains(".m3u8");

    if is_m3u8 {
        // Before rewriting, persist any url_param_cookies values found in
        // this manifest URL into the context so that sub-requests that have
        // no query params (e.g. bare .pkey key files) still get the cookies.
        if let Some(token) = query.ctx.as_deref() {
            if !ctx.url_param_cookies.is_empty() {
                // Use query_pairs() so percent-encoded values are decoded before
                // being stored (same decoding used when building the Cookie header).
                let query_map: std::collections::HashMap<String, String> = parsed
                    .query_pairs()
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect();
                let freshened: std::collections::HashMap<String, String> = ctx
                    .url_param_cookies
                    .iter()
                    .filter_map(|(param, cookie_name)| {
                        query_map.get(param.as_str())
                            .map(|val| (cookie_name.clone(), val.clone()))
                    })
                    .collect();
                if !freshened.is_empty() {
                    debug!(url = %upstream_url, ?freshened, "persisting resolved_cookies from manifest");
                    if let Ok(mut store) = state.proxy_ctx.write() {
                        if let Some(entry) = store.get_mut(token) {
                            entry.resolved_cookies.extend(freshened);
                            // Also persist the raw query string so that key URLs (.pkey/.key)
                            // can have it appended before fetching upstream (JioTV-Go pattern).
                            if let Some(q) = parsed.query() {
                                if entry.manifest_query.is_none() {
                                    entry.manifest_query = Some(q.to_owned());
                                    debug!(url = %upstream_url, manifest_query = %q, "persisting manifest_query");
                                }
                            }
                        }
                    }
                }
            }
        }

        let body_text = String::from_utf8_lossy(&body_bytes);
        let key_extra_query = if ctx.append_manifest_query_to_key_uris {
            ctx.manifest_query.as_deref()
        } else {
            None
        };
        let rewritten = rewrite_m3u8(&body_text, upstream_url, query.ctx.as_deref(), key_extra_query);

        let mut headers = HeaderMap::new();
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/vnd.apple.mpegurl"),
        );
        headers.insert(
            "access-control-allow-origin",
            HeaderValue::from_static("*"),
        );

        return Ok((
            StatusCode::from_u16(upstream_status.as_u16()).unwrap_or(StatusCode::OK),
            headers,
            rewritten,
        )
            .into_response());
    }

    // For non-m3u8 (segments, keys, etc.) — pass through as-is
    let mut headers = HeaderMap::new();
    headers.insert("content-type", content_type);
    headers.insert(
        "access-control-allow-origin",
        HeaderValue::from_static("*"),
    );

    Ok((
        StatusCode::from_u16(upstream_status.as_u16()).unwrap_or(StatusCode::OK),
        headers,
        body_bytes.to_vec(),
    )
        .into_response())
}

/// Rewrite URLs in an m3u8 playlist so they are proxied through `/api/proxy`.
///
/// Handles:
/// - Absolute URLs (`https://…`)
/// - Relative paths (`segment001.ts`, `../fallback/…`)
/// - URI attributes in EXT tags (`URI="…"`)
///
/// `ctx_token` is the opaque proxy-context token that should be appended to
/// every rewritten proxy URL so that segment/key requests use the same
/// server-side header set as the initial manifest request.
///
/// `manifest_query` is the raw query string from the original manifest URL
/// (e.g. `minrate=80000&__hdnea__=st%3D…`).  It is appended to key file
/// URLs (`.pkey`/`.key`) before proxying so that the upstream CDN receives
/// the Akamai token as a URL param — matching JioTV-Go's `RenderKeyHandler`.
fn rewrite_m3u8(content: &str, playlist_url: &str, ctx_token: Option<&str>, manifest_query: Option<&str>) -> String {
    let base = Url::parse(playlist_url).unwrap_or_else(|_| {
        Url::parse("http://unknown").unwrap()
    });

    let mut output = String::with_capacity(content.len());

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            output.push('\n');
            continue;
        }

        if trimmed.starts_with('#') {
            // Rewrite URI="…" attributes in EXT tags (e.g. EXT-X-KEY, EXT-X-MAP).
            // Pass manifest_query only for EXT-X-KEY lines (HLS encryption key
            // declarations) — controlled by the caller via `manifest_query`.
            let is_key_tag = trimmed.to_uppercase().starts_with("#EXT-X-KEY");
            let extra = if is_key_tag { manifest_query } else { None };
            let rewritten_line = rewrite_uri_attributes(trimmed, &base, ctx_token, extra);
            output.push_str(&rewritten_line);
            output.push('\n');
        } else {
            // This is a URL line (segment or sub-playlist)
            let resolved = resolve_and_proxy(trimmed, &base, ctx_token, None);
            output.push_str(&resolved);
            output.push('\n');
        }
    }

    output
}

/// Resolve a URL (potentially relative) against the playlist base and wrap it
/// in the proxy endpoint.  `ctx_token` is forwarded as-is if present.
///
/// `extra_query` is an optional raw query string to append to the resolved URL
/// before percent-encoding it.  Used for key file URLs to carry the manifest's
/// Akamai token so the upstream CDN authorises the request.
fn resolve_and_proxy(url_str: &str, base: &Url, ctx_token: Option<&str>, extra_query: Option<&str>) -> String {
    let absolute = if url_str.starts_with("http://") || url_str.starts_with("https://") {
        url_str.to_string()
    } else {
        base.join(url_str)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| url_str.to_string())
    };

    let with_query = match extra_query {
        Some(q) if !q.is_empty() => {
            if absolute.contains('?') {
                format!("{absolute}&{q}")
            } else {
                format!("{absolute}?{q}")
            }
        }
        _ => absolute,
    };

    match ctx_token {
        Some(token) => format!(
            "/api/proxy?url={}&ctx={}",
            urlencoding::encode(&with_query),
            token,
        ),
        None => format!("/api/proxy?url={}", urlencoding::encode(&with_query)),
    }
}

/// Rewrite `URI="…"` attributes inside EXT-X tags.
///
/// For encryption-key URIs (`.pkey` / `.key`), `manifest_query` is appended
/// to the target URL so the upstream CDN receives the session token as a URL
/// param (as JioTV-Go's `RenderKeyHandler` does).
fn rewrite_uri_attributes(line: &str, base: &Url, ctx_token: Option<&str>, manifest_query: Option<&str>) -> String {
    // Find URI="…" pattern (case-insensitive)
    let mut result = line.to_string();

    // Handle URI="..." pattern
    if let Some(uri_start) = result.to_uppercase().find("URI=\"") {
        let actual_start = uri_start + 5; // skip past URI="
        if let Some(uri_end) = result[actual_start..].find('"') {
            let uri_val = &result[actual_start..actual_start + uri_end].to_string();
            // Append manifest query params to key file URLs so the upstream CDN
            // (e.g. tv.media.jio.com Akamai) receives the auth token in the URL.
            let lower = uri_val.to_lowercase();
            let is_key = lower.contains(".pkey") || lower.ends_with(".key")
                || lower.contains("aes128.key") || lower.contains("/key");
            let extra = if is_key { manifest_query } else { None };
            let proxied = resolve_and_proxy(uri_val, base, ctx_token, extra);
            result = format!(
                "{}URI=\"{}\"{}",
                &line[..uri_start],
                proxied,
                &line[actual_start + uri_end + 1..]
            );
        }
    }

    result
}
