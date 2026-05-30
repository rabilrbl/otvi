use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Application-level error type that converts into an Axum response.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    /// Internal errors are logged with the full message but the HTTP response
    /// always returns the generic "Internal server error" string to avoid
    /// leaking implementation details.
    #[error("Internal server error")]
    Internal(#[source] InternalSource),
}

/// Carries the real error message for logging.  `Display` returns the message
/// so `tracing::error!(error = %self)` logs the full context, but the HTTP
/// response body uses the `#[error("Internal server error")]` annotation.
#[derive(Debug)]
pub struct InternalSource(pub String);

impl std::fmt::Display for InternalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InternalSource {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            Self::Internal(_) => {
                tracing::error!(error = %self, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };
        let body = serde_json::json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(InternalSource(e.to_string()))
    }
}

impl From<String> for InternalSource {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for InternalSource {
    fn from(s: &str) -> Self {
        Self(s.to_string())
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
            status_of(AppError::Internal(InternalSource("boom".into()))),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn from_anyhow_error_produces_internal() {
        let err: AppError = anyhow::anyhow!("something failed").into();
        assert_eq!(status_of(err), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn internal_error_message_not_leaked() {
        let err = AppError::Internal(InternalSource("secret db connection string".into()));
        let resp = err.into_response();
        let body = axum::body::to_bytes(resp.into_body(), 1024)
            .await
            .expect("read body");
        let body_str = String::from_utf8_lossy(&body);
        assert!(
            !body_str.contains("secret"),
            "internal error message leaked: {body_str}"
        );
    }
}
