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
use url::Url;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct ProxyQuery {
    pub url: String,
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

    // Fetch upstream
    let resp = state
        .http_client
        .get(parsed.as_str())
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
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
        let body_text = String::from_utf8_lossy(&body_bytes);
        let rewritten = rewrite_m3u8(&body_text, upstream_url);

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
fn rewrite_m3u8(content: &str, playlist_url: &str) -> String {
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
            // Rewrite URI="…" attributes in EXT tags (e.g. EXT-X-KEY, EXT-X-MAP)
            let rewritten_line = rewrite_uri_attributes(trimmed, &base);
            output.push_str(&rewritten_line);
            output.push('\n');
        } else {
            // This is a URL line (segment or sub-playlist)
            let resolved = resolve_and_proxy(trimmed, &base);
            output.push_str(&resolved);
            output.push('\n');
        }
    }

    output
}

/// Resolve a URL (potentially relative) against the playlist base and wrap it
/// in the proxy endpoint.
fn resolve_and_proxy(url_str: &str, base: &Url) -> String {
    let absolute = if url_str.starts_with("http://") || url_str.starts_with("https://") {
        url_str.to_string()
    } else {
        base.join(url_str)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| url_str.to_string())
    };

    format!(
        "/api/proxy?url={}",
        urlencoding::encode(&absolute)
    )
}

/// Rewrite `URI="…"` attributes inside EXT-X tags.
fn rewrite_uri_attributes(line: &str, base: &Url) -> String {
    // Find URI="…" pattern (case-insensitive)
    let mut result = line.to_string();

    // Handle URI="..." pattern
    if let Some(uri_start) = result.to_uppercase().find("URI=\"") {
        let actual_start = uri_start + 5; // skip past URI="
        if let Some(uri_end) = result[actual_start..].find('"') {
            let uri_val = &result[actual_start..actual_start + uri_end].to_string();
            let proxied = resolve_and_proxy(uri_val, base);
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
