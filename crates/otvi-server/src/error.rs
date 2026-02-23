use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Application-level error type that converts into an Axum response.
#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Unauthorized,
    Forbidden(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let body = serde_json::json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    fn status_of(err: AppError) -> StatusCode {
        err.into_response().status()
    }

    #[test]
    fn not_found_produces_404() {
        assert_eq!(
            status_of(AppError::NotFound("gone".into())),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn bad_request_produces_400() {
        assert_eq!(
            status_of(AppError::BadRequest("bad".into())),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn unauthorized_produces_401() {
        assert_eq!(status_of(AppError::Unauthorized), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn forbidden_produces_403() {
        assert_eq!(
            status_of(AppError::Forbidden("nope".into())),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn internal_produces_500() {
        assert_eq!(
            status_of(AppError::Internal("boom".into())),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn from_anyhow_error_produces_internal() {
        let err: AppError = anyhow::anyhow!("something failed").into();
        assert_eq!(status_of(err), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
