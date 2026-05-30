//! HLS / DASH stream proxy.
//!
//! Fetches upstream content on behalf of the browser (avoiding CORS issues)
//! and rewrites relative URLs inside `.m3u8` playlists so that subsequent
//! requests also go through this proxy.

mod drm;
mod rewrite;
mod validate;

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use url::Url;

use crate::state::AppState;

use rewrite::{rewrite_m3u8, rewrite_mpd};
use validate::{merge_allowed_host, merge_allowed_hosts, validate_proxy_target};

fn append_cookie(
    cookie_pairs: &mut Vec<String>,
    seen: &mut HashSet<String>,
    name: &str,
    value: &str,
) {
    if seen.insert(name.to_owned()) {
        cookie_pairs.push(format!("{name}={value}"));
    }
}

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
#[utoipa::path(
    get,
    path = "/api/proxy",
    tag = "proxy",
    params(
        ("url" = String, Query, description = "Upstream stream URL to fetch (HLS manifest, segment, or key file)"),
        ("ctx" = Option<String>, Query, description = "Opaque proxy-context token issued by the stream endpoint; carries provider headers server-side when required"),
    ),
    responses(
        (status = 200, description = "Upstream content, with m3u8 URLs rewritten to route through this proxy"),
        (status = 400, description = "Invalid or missing URL parameter"),
        (status = 502, description = "Upstream fetch failed"),
    ),
)]
pub async fn proxy_stream(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ProxyQuery>,
) -> Result<Response, (StatusCode, String)> {
    let upstream_url = &query.url;

    // Validate URL
    let parsed = Url::parse(upstream_url)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid URL: {e}")))?;

    let mut ctx = match query.ctx.as_deref() {
        Some(token) => {
            let ctx = state
                .proxy_ctx
                .get(token)
                .await
                .ok_or((StatusCode::BAD_REQUEST, "Invalid proxy context".to_string()))?;
            validate_proxy_target(&ctx, &parsed, state.allow_private_hosts)?;
            ctx
        }
        None => {
            return Err((StatusCode::BAD_REQUEST, "Missing proxy context".to_string()));
        }
    };

    // Fetch upstream
    let mut req = state.http_client.get(parsed.as_str());
    // Apply provider-specified headers (override default UA when present)
    for (k, v) in &ctx.headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let is_key_url = upstream_url.contains(".pkey") || upstream_url.contains(".key");
    // Fall back to a generic UA only when none was supplied
    if !ctx
        .headers
        .keys()
        .any(|k| k.eq_ignore_ascii_case("user-agent"))
    {
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
    if !ctx.url_param_cookies.is_empty()
        || !ctx.resolved_cookies.is_empty()
        || !ctx.static_cookies.is_empty()
    {
        // Decode all query pairs once so we can look them up cheaply.
        let query_map: std::collections::HashMap<String, String> = parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        let mut cookie_pairs = Vec::new();
        let mut seen_cookie_names = HashSet::new();

        // When `key_exclude_resolved_cookies` is set, skip URL-param-extracted
        // cookies (sources 1 & 2) for key requests.  This prevents CDN auth
        // tokens (e.g. Akamai `__hdnea__`) whose ACL covers only the segment
        // CDN path from being sent to a separate key server on a different
        // domain/path — which would cause a 403 from the CDN firewall.
        let skip_url_cookies = is_key_url && ctx.key_exclude_resolved_cookies;

        if !skip_url_cookies {
            // Source 1 – params in the current URL (freshest value wins)
            for (param, cookie_name) in &ctx.url_param_cookies {
                if let Some(val) = query_map.get(param.as_str()) {
                    append_cookie(&mut cookie_pairs, &mut seen_cookie_names, cookie_name, val);
                }
            }
            // Source 2 – persisted values from a prior manifest fetch (fill gaps)
            for (cookie_name, val) in &ctx.resolved_cookies {
                append_cookie(&mut cookie_pairs, &mut seen_cookie_names, cookie_name, val);
            }
        }
        // Source 3 – static cookies from provider YAML `proxy_cookies` field
        // (lowest priority; overridden by URL-extracted or manifest-extracted values)
        for (cookie_name, val) in &ctx.static_cookies {
            append_cookie(&mut cookie_pairs, &mut seen_cookie_names, cookie_name, val);
        }
        let cookie_header = cookie_pairs.join("; ");
        if !cookie_header.is_empty() {
            req = req.header("Cookie", cookie_header);
        }
    }
    let resp = req.send().await.map_err(|e| {
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
        let query_map: std::collections::HashMap<String, String> = parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let freshened: std::collections::HashMap<String, String> = ctx
            .url_param_cookies
            .iter()
            .filter_map(|(param, cookie_name)| {
                query_map
                    .get(param.as_str())
                    .map(|val| (cookie_name.clone(), val.clone()))
            })
            .collect();

        let mut changed = false;
        if !freshened.is_empty() {
            ctx.resolved_cookies.extend(freshened);
            changed = true;
        }
        if let Some(q) = parsed.query()
            && ctx.manifest_query.is_none()
        {
            ctx.manifest_query = Some(q.to_owned());
            changed = true;
        }

        let body_text = String::from_utf8_lossy(&body_bytes);
        let key_extra_query = if ctx.append_manifest_query_to_key_uris {
            ctx.manifest_query.as_deref()
        } else {
            None
        };
        let rewritten = rewrite_m3u8(
            &body_text,
            upstream_url,
            query.ctx.as_deref(),
            key_extra_query,
            &ctx.key_uri_patterns,
        );
        merge_allowed_host(&mut ctx.allowed_hosts, parsed.host_str());
        merge_allowed_hosts(&mut ctx.allowed_hosts, &rewritten.discovered_hosts);
        if let Some(token) = query.ctx.as_deref()
            && (changed || !rewritten.discovered_hosts.is_empty())
        {
            state.proxy_ctx.insert(token.to_string(), ctx).await;
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/vnd.apple.mpegurl"),
        );
        headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));

        return Ok((
            StatusCode::from_u16(upstream_status.as_u16()).unwrap_or(StatusCode::OK),
            headers,
            rewritten.content,
        )
            .into_response());
    }

    // ── DASH MPD manifest rewriting ───────────────────────────────────────
    //
    // Detect MPD manifests by content type or URL extension and rewrite
    // <BaseURL> elements so that segment requests route back through this
    // proxy.  On any rewriting error, fall back to the unmodified MPD.

    let is_mpd = ct_str.contains("dash+xml")
        || ct_str.contains("application/xml")
            && (upstream_url.contains(".mpd") || upstream_url.contains("mpd"))
        || upstream_url.ends_with(".mpd")
        || upstream_url.contains(".mpd?");

    if is_mpd {
        let body_text = String::from_utf8_lossy(&body_bytes);

        // Discover any new hosts from the MPD content and update allowed hosts.
        merge_allowed_host(&mut ctx.allowed_hosts, parsed.host_str());

        let rewritten = match rewrite_mpd(&body_text, upstream_url, query.ctx.as_deref()) {
            Ok(result) => {
                merge_allowed_hosts(&mut ctx.allowed_hosts, &result.discovered_hosts);
                if let Some(token) = query.ctx.as_deref()
                    && !result.discovered_hosts.is_empty()
                {
                    state.proxy_ctx.insert(token.to_string(), ctx).await;
                }
                result.content
            }
            Err(e) => {
                tracing::warn!(error = %e, "MPD rewriting failed — returning unmodified manifest");
                body_text.into_owned()
            }
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/dash+xml"),
        );
        headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));

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
    headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));

    Ok((
        StatusCode::from_u16(upstream_status.as_u16()).unwrap_or(StatusCode::OK),
        headers,
        body_bytes.to_vec(),
    )
        .into_response())
}

#[allow(unused_imports)]
pub use drm::__path_proxy_drm;
pub use drm::proxy_drm;

#[cfg(test)]
mod tests {
    use super::*;
    use rewrite::{resolve_and_proxy, rewrite_uri_attributes};
    use validate::is_private_host;

    #[test]
    fn rewrite_m3u8_rewrites_absolute_urls() {
        let content = "#EXTM3U\nhttps://cdn.example.com/seg1.ts\n";
        let result = rewrite_m3u8(
            content,
            "https://cdn.example.com/master.m3u8",
            None,
            None,
            &[],
        );
        assert!(result.contains("/api/proxy?url="));
        assert!(result.contains("cdn.example.com"));
        assert!(!result.contains("\nhttps://cdn.example.com/seg1.ts\n"));
    }

    #[test]
    fn rewrite_m3u8_rewrites_relative_urls() {
        let content = "#EXTM3U\nsegment001.ts\n";
        let result = rewrite_m3u8(
            content,
            "https://cdn.example.com/live/master.m3u8",
            None,
            None,
            &[],
        );
        assert!(result.contains("/api/proxy?url="));
        // Relative URL should be resolved against the playlist base
        assert!(result.contains("cdn.example.com"));
    }

    #[test]
    fn rewrite_m3u8_rewrites_uri_in_ext_x_key() {
        let content =
            "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"https://key.example.com/key.pkey\"\n";
        let result = rewrite_m3u8(
            content,
            "https://cdn.example.com/master.m3u8",
            Some("tok123"),
            None,
            &[],
        );
        assert!(result.contains("URI=\"/api/proxy?url="));
        assert!(result.contains("ctx=tok123"));
    }

    #[test]
    fn rewrite_m3u8_preserves_non_url_lines() {
        let content = "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:10\n#EXTINF:9.009,\n";
        let result = rewrite_m3u8(content, "https://example.com/m.m3u8", None, None, &[]);
        assert!(result.contains("#EXTM3U"));
        assert!(result.contains("#EXT-X-VERSION:3"));
        assert!(result.contains("#EXT-X-TARGETDURATION:10"));
    }

    #[test]
    fn rewrite_m3u8_appends_ctx_token() {
        let content = "#EXTM3U\nseg.ts\n";
        let result = rewrite_m3u8(
            content,
            "https://cdn.example.com/m.m3u8",
            Some("abc"),
            None,
            &[],
        );
        assert!(result.contains("ctx=abc"));
    }

    #[test]
    fn rewrite_m3u8_appends_manifest_query_to_key_uris() {
        let content =
            "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"https://key.example.com/key.pkey\"\n";
        let result = rewrite_m3u8(
            content,
            "https://cdn.example.com/master.m3u8",
            Some("tok"),
            Some("hdnea=token123"),
            &[],
        );
        // The manifest query should be appended to the key URI
        assert!(result.contains("hdnea%3Dtoken123") || result.contains("hdnea"));
    }

    #[test]
    fn resolve_and_proxy_absolute_url() {
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let result = resolve_and_proxy(
            "https://other.com/seg.ts",
            &base,
            None,
            None,
            &mut HashSet::new(),
        );
        assert!(result.starts_with("/api/proxy?url="));
        assert!(result.contains("other.com"));
    }

    #[test]
    fn resolve_and_proxy_relative_url() {
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let result = resolve_and_proxy("segment001.ts", &base, None, None, &mut HashSet::new());
        assert!(result.starts_with("/api/proxy?url="));
        // Should be resolved to https://cdn.example.com/live/segment001.ts
        assert!(result.contains("cdn.example.com"));
        assert!(result.contains("segment001.ts"));
    }

    #[test]
    fn resolve_and_proxy_with_ctx_token() {
        let base = Url::parse("https://cdn.example.com/m.m3u8").unwrap();
        let result = resolve_and_proxy("seg.ts", &base, Some("mytoken"), None, &mut HashSet::new());
        assert!(result.contains("ctx=mytoken"));
    }

    #[test]
    fn resolve_and_proxy_with_extra_query() {
        let base = Url::parse("https://cdn.example.com/m.m3u8").unwrap();
        let result = resolve_and_proxy(
            "https://key.com/key.pkey",
            &base,
            None,
            Some("tok=abc"),
            &mut HashSet::new(),
        );
        assert!(result.contains("tok%3Dabc") || result.contains("tok"));
    }

    #[test]
    fn rewrite_uri_attributes_rewrites_uri_value() {
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let line = "#EXT-X-KEY:METHOD=AES-128,URI=\"keys/enc.key\",IV=0x1234";
        let result =
            rewrite_uri_attributes(line, &base, Some("ctx1"), None, &[], &mut HashSet::new());
        assert!(result.contains("URI=\"/api/proxy?url="));
        assert!(result.contains("ctx=ctx1"));
        assert!(result.contains("IV=0x1234"));
    }

    #[test]
    fn rewrite_uri_attributes_no_uri_unchanged() {
        let base = Url::parse("https://cdn.example.com/m.m3u8").unwrap();
        let line = "#EXT-X-VERSION:3";
        let result = rewrite_uri_attributes(line, &base, None, None, &[], &mut HashSet::new());
        assert_eq!(result, "#EXT-X-VERSION:3");
    }

    #[test]
    fn rewrite_uri_attributes_key_with_manifest_query() {
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let line = "#EXT-X-KEY:METHOD=AES-128,URI=\"enc.pkey\"";
        let result = rewrite_uri_attributes(
            line,
            &base,
            Some("c1"),
            Some("hdnea=val"),
            &[],
            &mut HashSet::new(),
        );
        // Empty patterns → apply to all key-tag URIs; manifest query must be appended
        assert!(result.contains("hdnea"));
    }

    #[test]
    fn rewrite_m3u8_empty_lines_preserved() {
        let content = "#EXTM3U\n\n#EXT-X-VERSION:3\n";
        let result = rewrite_m3u8(content, "https://example.com/m.m3u8", None, None, &[]);
        assert!(result.contains("\n\n"));
    }

    #[test]
    fn resolve_and_proxy_parent_relative_path() {
        let base = Url::parse("https://cdn.example.com/live/hd/master.m3u8").unwrap();
        let result = resolve_and_proxy("../sd/stream.m3u8", &base, None, None, &mut HashSet::new());
        assert!(result.contains("cdn.example.com"));
        // Should resolve to /live/sd/stream.m3u8
        assert!(result.contains("sd"));
    }

    // ── Provider-specific key_uri_patterns tests ─────────────────────────────

    #[test]
    fn rewrite_uri_attributes_pattern_match_appends_query() {
        // When patterns are given and the URI matches, manifest query IS appended.
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let line = "#EXT-X-KEY:METHOD=AES-128,URI=\"https://keys.example.com/enc.pkey\"";
        let patterns = vec![".pkey".to_string()];
        let result = rewrite_uri_attributes(
            line,
            &base,
            Some("c1"),
            Some("tok=abc"),
            &patterns,
            &mut HashSet::new(),
        );
        assert!(result.contains("tok%3Dabc") || result.contains("tok"));
    }

    #[test]
    fn rewrite_uri_attributes_pattern_no_match_skips_query() {
        // When patterns are given but the URI does NOT match, manifest query is NOT appended.
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let line = "#EXT-X-KEY:METHOD=AES-128,URI=\"https://keys.example.com/enc.bin\"";
        let patterns = vec![".pkey".to_string()];
        let result = rewrite_uri_attributes(
            line,
            &base,
            Some("c1"),
            Some("tok=abc"),
            &patterns,
            &mut HashSet::new(),
        );
        // enc.bin doesn't match ".pkey" → query must NOT be present
        assert!(!result.contains("tok%3Dabc") && !result.contains("tok=abc"));
        // But the URI itself is still proxied
        assert!(result.contains("/api/proxy?url="));
    }

    #[test]
    fn rewrite_uri_attributes_multiple_patterns_one_matches() {
        // With multiple patterns, matching any one of them is sufficient.
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let line = "#EXT-X-KEY:METHOD=AES-128,URI=\"https://keys.example.com/enc.pkey\"";
        let patterns = vec![".bin".to_string(), ".pkey".to_string(), ".dat".to_string()];
        let result = rewrite_uri_attributes(
            line,
            &base,
            Some("c1"),
            Some("tok=abc"),
            &patterns,
            &mut HashSet::new(),
        );
        assert!(result.contains("tok%3Dabc") || result.contains("tok"));
        assert!(result.contains("/api/proxy?url="));
    }

    #[test]
    fn rewrite_uri_attributes_multiple_patterns_none_match() {
        // With multiple patterns, if none match, query is NOT appended.
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let line = "#EXT-X-KEY:METHOD=AES-128,URI=\"https://keys.example.com/enc.key\"";
        let patterns = vec![".pkey".to_string(), "/custom-ks/".to_string()];
        let result = rewrite_uri_attributes(
            line,
            &base,
            Some("c1"),
            Some("tok=abc"),
            &patterns,
            &mut HashSet::new(),
        );
        assert!(!result.contains("tok%3Dabc") && !result.contains("tok=abc"));
        assert!(result.contains("/api/proxy?url="));
    }

    #[test]
    fn rewrite_uri_attributes_pattern_match_case_insensitive() {
        // Patterns must match case-insensitively (URI may be uppercase).
        let base = Url::parse("https://cdn.example.com/live/master.m3u8").unwrap();
        let line = "#EXT-X-KEY:METHOD=AES-128,URI=\"https://keys.example.com/ENC.PKEY\"";
        let patterns = vec![".pkey".to_string()]; // lowercase pattern, uppercase URI
        let result = rewrite_uri_attributes(
            line,
            &base,
            Some("c1"),
            Some("tok=abc"),
            &patterns,
            &mut HashSet::new(),
        );
        assert!(result.contains("tok%3Dabc") || result.contains("tok"));
    }

    #[test]
    fn rewrite_m3u8_provider_patterns_applied_to_key_uris() {
        // Verify that key_uri_patterns are respected end-to-end through rewrite_m3u8.
        let content =
            "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"https://key.example.com/enc.customkey\"\n";
        let patterns = vec![".customkey".to_string()];
        let result = rewrite_m3u8(
            content,
            "https://cdn.example.com/master.m3u8",
            Some("tok"),
            Some("auth=secret"),
            &patterns,
        );
        assert!(result.contains("auth%3Dsecret") || result.contains("auth"));
    }

    #[test]
    fn rewrite_m3u8_patterns_skip_non_matching_key_uri() {
        // When a URI does not match any pattern, manifest query must NOT be appended.
        let content =
            "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"https://key.example.com/enc.bin\"\n";
        let patterns = vec![".pkey".to_string()];
        let result = rewrite_m3u8(
            content,
            "https://cdn.example.com/master.m3u8",
            Some("tok"),
            Some("auth=secret"),
            &patterns,
        );
        // enc.bin doesn't match .pkey → manifest query must NOT appear in the URL
        assert!(!result.contains("auth%3Dsecret") && !result.contains("auth=secret"));
        // The URI is still proxied
        assert!(result.contains("/api/proxy?url="));
    }

    // ── is_private_host ───────────────────────────────────────────────────

    #[test]
    fn private_host_loopback_ipv4() {
        assert!(is_private_host("127.0.0.1"));
        assert!(is_private_host("127.255.255.255"));
    }

    #[test]
    fn private_host_loopback_ipv6() {
        assert!(is_private_host("::1"));
        assert!(is_private_host("[::1]"));
    }

    #[test]
    fn private_host_unspecified_ipv6() {
        // :: (IPv6 unspecified / INADDR_ANY6) routes to ::1 on Linux.
        assert!(is_private_host("::"));
        assert!(is_private_host("[::]"));
    }

    #[test]
    fn private_host_private_ranges() {
        assert!(is_private_host("10.0.0.1"));
        assert!(is_private_host("10.255.255.255"));
        assert!(is_private_host("172.16.0.1"));
        assert!(is_private_host("172.31.255.255"));
        assert!(is_private_host("192.168.0.1"));
        assert!(is_private_host("192.168.255.255"));
    }

    #[test]
    fn private_host_link_local() {
        assert!(is_private_host("169.254.169.254")); // AWS metadata
        assert!(is_private_host("169.254.0.1"));
    }

    #[test]
    fn private_host_localhost_string() {
        assert!(is_private_host("localhost"));
        assert!(is_private_host("LOCALHOST"));
    }

    #[test]
    fn private_host_public_ips_not_blocked() {
        assert!(!is_private_host("1.1.1.1"));
        assert!(!is_private_host("8.8.8.8"));
        assert!(!is_private_host("cdn.example.com"));
    }

    // ── validate_proxy_target ─────────────────────────────────────────────

    fn make_ctx(upstream_url: &str, allowed_hosts: Vec<&str>) -> crate::state::ProxyContext {
        use std::collections::HashMap;
        crate::state::ProxyContext {
            upstream_url: upstream_url.to_string(),
            headers: HashMap::new(),
            allowed_hosts: allowed_hosts.into_iter().map(str::to_string).collect(),
            url_param_cookies: HashMap::new(),
            resolved_cookies: HashMap::new(),
            static_cookies: HashMap::new(),
            manifest_query: None,
            append_manifest_query_to_key_uris: false,
            key_exclude_resolved_cookies: false,
            key_uri_patterns: vec![],
            stream_type: otvi_core::types::StreamType::Hls,
            drm_license_url: None,
            drm_license_headers: None,
            drm_license_cookies: None,
            drm_prefetch_url: None,
        }
    }

    #[test]
    fn validate_proxy_empty_allowed_hosts_denied() {
        let ctx = make_ctx("https://cdn.example.com/stream.m3u8", vec![]);
        let url = Url::parse("https://cdn.example.com/seg1.ts").unwrap();
        assert!(validate_proxy_target(&ctx, &url, false).is_err());
    }

    #[test]
    fn validate_proxy_private_ip_always_denied() {
        // Even when the IP is in allowed_hosts, private addresses are blocked.
        let ctx = make_ctx("https://10.0.0.1/stream.m3u8", vec!["10.0.0.1"]);
        let url = Url::parse("https://10.0.0.1/seg1.ts").unwrap();
        assert!(validate_proxy_target(&ctx, &url, false).is_err());
    }

    #[test]
    fn validate_proxy_metadata_endpoint_denied() {
        let ctx = make_ctx(
            "https://cdn.example.com/stream.m3u8",
            vec!["169.254.169.254"],
        );
        let url = Url::parse("http://169.254.169.254/latest/meta-data/").unwrap();
        assert!(validate_proxy_target(&ctx, &url, false).is_err());
    }

    #[test]
    fn validate_proxy_allowed_host_passes() {
        let ctx = make_ctx(
            "https://cdn.example.com/stream.m3u8",
            vec!["cdn.example.com"],
        );
        let url = Url::parse("https://cdn.example.com/seg1.ts").unwrap();
        assert!(validate_proxy_target(&ctx, &url, false).is_ok());
    }

    #[test]
    fn validate_proxy_disallowed_host_blocked() {
        let ctx = make_ctx(
            "https://cdn.example.com/stream.m3u8",
            vec!["cdn.example.com"],
        );
        let url = Url::parse("https://evil.example.com/payload").unwrap();
        assert!(validate_proxy_target(&ctx, &url, false).is_err());
    }

    #[test]
    fn private_host_ipv6_link_local() {
        // fe80::/10 — bare and bracket-wrapped forms must both be detected.
        assert!(is_private_host("fe80::1"));
        assert!(is_private_host("[fe80::1]"));
        assert!(is_private_host("fe80::dead:beef"));
        assert!(is_private_host("[fe80::dead:beef]"));
    }

    #[test]
    fn private_host_unspecified_ipv4() {
        // 0.0.0.0 (INADDR_ANY) — on Linux connecting to this routes to loopback.
        assert!(is_private_host("0.0.0.0"));
    }

    #[test]
    fn validate_proxy_exact_upstream_private_ip_denied() {
        // The exact upstream URL matching ctx.upstream_url must still be blocked
        // when the host is a private address (a malicious provider YAML could set
        // upstream_url to an internal endpoint and the proxy must reject it).
        let ctx = make_ctx("https://10.0.0.1/stream.m3u8", vec!["10.0.0.1"]);
        let exact_upstream = Url::parse("https://10.0.0.1/stream.m3u8").unwrap();
        assert!(
            validate_proxy_target(&ctx, &exact_upstream, false).is_err(),
            "exact upstream URL pointing to private IP must be denied"
        );
    }

    #[test]
    fn validate_proxy_private_host_allowed_in_test_mode() {
        let ctx = make_ctx("http://localhost:8888/stream.m3u8", vec!["localhost"]);
        let url = Url::parse("http://localhost:8888/seg1.ts").unwrap();
        assert!(
            validate_proxy_target(&ctx, &url, true).is_ok(),
            "private hosts should be allowed when allow_private_hosts is true"
        );
    }
}
