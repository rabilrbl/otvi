use axum::body::Body;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

include!(concat!(env!("OUT_DIR"), "/embedded_frontend_assets.rs"));

pub fn has_embedded_frontend() -> bool {
    !EMBEDDED_ASSETS.is_empty()
}

pub async fn serve_embedded_frontend(uri: Uri) -> Response {
    let path = normalize_path(uri.path());

    if let Some(response) = asset_response(&path) {
        return response;
    }

    let is_spa_route = !path.rsplit('/').next().unwrap_or_default().contains('.');
    if (is_spa_route || path == "/index.html")
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

fn asset_response(path: &str) -> Option<Response> {
    let (_, bytes) = EMBEDDED_ASSETS
        .iter()
        .find(|(asset_path, _)| *asset_path == path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    let mut response = Response::new(Body::from(*bytes));
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
