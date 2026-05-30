use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

/// `POST /api/proxy/drm/{token}`
///
/// Proxies the player's Widevine (or other DRM) license request to the
/// upstream license server.  The opaque `token` identifies a `ProxyContext`
/// created by the stream endpoint; all sensitive auth headers and cookies are
/// stored server-side and never exposed to the browser.
///
/// ## Flow
///
/// 1. Look up `ProxyContext` by `token`; reject if missing or has no DRM config.
/// 2. Optionally issue a HEAD request to `drm_prefetch_url` (cookie refresh).
/// 3. Forward the raw request body (the DRM challenge) to the upstream
///    license URL with the provider-configured headers and cookies.
/// 4. Return the upstream response body (the license) with the upstream
///    status code.
#[utoipa::path(
    post,
    path = "/api/proxy/drm/{token}",
    tag = "proxy",
    params(
        ("token" = String, Path, description = "Opaque proxy-context token issued by the stream endpoint"),
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "DRM license challenge (binary)"),
    responses(
        (status = 200, description = "DRM license response from upstream"),
        (status = 400, description = "Missing or empty request body, or context has no DRM config"),
        (status = 404, description = "Invalid or expired proxy context token"),
        (status = 502, description = "Upstream license server error"),
    ),
)]
pub async fn proxy_drm(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    body: Bytes,
) -> Result<Response, (StatusCode, String)> {
    // 1. Look up the proxy context.
    let ctx = state.proxy_ctx.get(&token).await.ok_or((
        StatusCode::NOT_FOUND,
        "Invalid or expired proxy context token".to_string(),
    ))?;

    let license_url = ctx.drm_license_url.as_ref().ok_or((
        StatusCode::BAD_REQUEST,
        "Proxy context has no DRM license URL configured".to_string(),
    ))?;

    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "DRM license request body is empty".to_string(),
        ));
    }

    // 2. Optional HEAD prefetch (cookie refresh).
    if let Some(prefetch_url) = &ctx.drm_prefetch_url {
        tracing::debug!(prefetch_url = %prefetch_url, "DRM prefetch HEAD request");
        let mut prefetch_req = state.http_client.head(prefetch_url.as_str());
        for (k, v) in &ctx.headers {
            prefetch_req = prefetch_req.header(k.as_str(), v.as_str());
        }
        // Fire-and-forget: we only care about refreshing cookies, not the response.
        if let Err(e) = prefetch_req.send().await {
            tracing::warn!(error = %e, "DRM prefetch HEAD request failed — continuing anyway");
        }
    }

    // 3. Build and send the upstream license request.
    let mut req = state.http_client.post(license_url.as_str());

    // Apply DRM-specific headers.
    if let Some(headers) = &ctx.drm_license_headers {
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
    }

    // Apply provider proxy headers (User-Agent etc.).
    for (k, v) in &ctx.headers {
        // Don't override DRM-specific headers already set above.
        if ctx
            .drm_license_headers
            .as_ref()
            .is_some_and(|h| h.contains_key(k))
        {
            continue;
        }
        req = req.header(k.as_str(), v.as_str());
    }

    // Apply cookies from the DRM cookie list (resolved from static_cookies).
    if let Some(cookie_names) = &ctx.drm_license_cookies {
        let cookie_pairs: Vec<String> = cookie_names
            .iter()
            .filter_map(|name| {
                ctx.static_cookies
                    .get(name)
                    .map(|val| format!("{name}={val}"))
            })
            .collect();
        if !cookie_pairs.is_empty() {
            req = req.header("Cookie", cookie_pairs.join("; "));
        }
    }

    // Set content type for the license request body.
    req = req.header("Content-Type", "application/octet-stream");

    let resp = req.body(body.to_vec()).send().await.map_err(|e| {
        tracing::error!(error = %e, license_url = %license_url, "DRM license upstream request failed");
        (
            StatusCode::BAD_GATEWAY,
            format!("Failed to fetch DRM license: {e}"),
        )
    })?;

    // 4. Return the upstream response.
    let upstream_status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"));

    let body_bytes = resp.bytes().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Failed to read DRM license response body: {e}"),
        )
    })?;

    use axum::http::HeaderMap;
    let mut headers = HeaderMap::new();
    headers.insert("content-type", content_type);
    headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
    headers.insert(
        "access-control-allow-methods",
        HeaderValue::from_static("POST, OPTIONS"),
    );
    headers.insert(
        "access-control-allow-headers",
        HeaderValue::from_static("Content-Type"),
    );

    Ok((
        StatusCode::from_u16(upstream_status.as_u16()).unwrap_or(StatusCode::OK),
        headers,
        body_bytes.to_vec(),
    )
        .into_response())
}
