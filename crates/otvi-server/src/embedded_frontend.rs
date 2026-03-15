use std::cmp::Ordering;

use axum::body::Body;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

include!(concat!(env!("OUT_DIR"), "/embedded_frontend_assets.rs"));

pub fn has_embedded_frontend() -> bool {
    !EMBEDDED_ASSETS.is_empty()
}

pub async fn serve_embedded_frontend(uri: Uri) -> Response {
    let original_path = uri.path();
    let path = normalize_path(original_path);

    if let Some(response) = asset_response(&path) {
        return response;
    }

    if should_fallback_to_index_html(original_path, &path)
        && let Some(response) = asset_response("/index.html")
    {
        return response;
    }

    StatusCode::NOT_FOUND.into_response()
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();

    if trimmed.is_empty() || trimmed == "/" {
        return "/index.html".to_string();
    }

    let normalized = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };

    if normalized.ends_with('/') {
        format!("{normalized}index.html")
    } else {
        normalized
    }
}

fn should_fallback_to_index_html(original_path: &str, normalized_path: &str) -> bool {
    let last_segment = normalized_path.rsplit('/').next().unwrap_or_default();
    normalized_path == "/index.html" || !last_segment.contains('.') || original_path.ends_with('/')
}

fn asset_response(path: &str) -> Option<Response> {
    let index = EMBEDDED_ASSETS
        .binary_search_by(|(asset_path, _)| compare_asset_path(asset_path, path))
        .ok()?;
    let (_, bytes) = EMBEDDED_ASSETS[index];
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).ok()?);
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );

    Some(response)
}

fn compare_asset_path(asset_path: &str, requested_path: &str) -> Ordering {
    asset_path.cmp(requested_path)
}

#[cfg(test)]
mod tests {
    use super::{normalize_path, should_fallback_to_index_html};

    #[test]
    fn normalize_root_path_to_index() {
        assert_eq!(normalize_path("/"), "/index.html");
    }

    #[test]
    fn normalize_directory_path_to_index() {
        assert_eq!(normalize_path("/channels/"), "/channels/index.html");
    }

    #[test]
    fn normalize_asset_path_without_changes() {
        assert_eq!(normalize_path("/assets/app.js"), "/assets/app.js");
    }

    #[test]
    fn spa_route_without_extension_falls_back() {
        assert!(should_fallback_to_index_html("/channels", "/channels"));
    }

    #[test]
    fn spa_route_with_trailing_slash_falls_back() {
        assert!(should_fallback_to_index_html(
            "/channels/",
            "/channels/index.html"
        ));
    }

    #[test]
    fn asset_path_does_not_fall_back() {
        assert!(!should_fallback_to_index_html(
            "/assets/app.js",
            "/assets/app.js"
        ));
    }

    #[test]
    fn root_index_falls_back() {
        assert!(should_fallback_to_index_html("/", "/index.html"));
    }
}
