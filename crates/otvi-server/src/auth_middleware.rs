//! JWT-based authentication for OTVI application users.
//!
//! Each browser session gets a short-lived JWT that encodes the user ID,
//! username, and role.  The token is sent via the `Authorization: Bearer …`
//! header on every API request.
//!
//! The signing secret is read from the `JWT_SECRET` environment variable;
//! if not set, a random secret is generated at startup (tokens will not
//! survive a server restart in that case).

use axum::RequestPartsExt;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use otvi_core::types::UserRole;

use crate::db;

// ── JWT secret ────────────────────────────────────────────────────────────

/// Application-wide JWT keys.  Created once at startup; held in `AppState`.
pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JwtKeys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

// ── Claims ─────────────────────────────────────────────────────────────────

/// Payload embedded in every OTVI JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject – OTVI user ID.
    pub sub: String,
    /// Display username.
    pub username: String,
    /// Role: `"admin"` or `"user"`.
    pub role: String,
    /// Expiry (Unix timestamp).
    pub exp: u64,
}

impl Claims {
    pub fn role(&self) -> UserRole {
        match self.role.as_str() {
            "admin" => UserRole::Admin,
            _ => UserRole::User,
        }
    }

    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

/// Token lifetime in seconds (24 hours).
const TOKEN_TTL_SECS: u64 = 86_400;

// ── Token creation / validation ────────────────────────────────────────────

pub fn create_token(keys: &JwtKeys, user_id: &str, username: &str, role: &UserRole) -> String {
    let exp = jsonwebtoken::get_current_timestamp() + TOKEN_TTL_SECS;
    let claims = Claims {
        sub: user_id.to_owned(),
        username: username.to_owned(),
        role: match role {
            UserRole::Admin => "admin".to_owned(),
            UserRole::User => "user".to_owned(),
        },
        exp,
    };
    encode(&Header::default(), &claims, &keys.encoding).expect("JWT encode failed")
}

pub fn validate_token(keys: &JwtKeys, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(token, &keys.decoding, &Validation::default())?;
    Ok(data.claims)
}

// ── Axum extractor ─────────────────────────────────────────────────────────

/// Axum extractor: validates the `Authorization: Bearer <token>` header and
/// returns the decoded [`Claims`].  Returns `401 Unauthorized` if the header
/// is missing or the token is invalid/expired.
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync + std::ops::Deref<Target = crate::state::AppState>,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let header_result = parts.extract::<TypedHeader<Authorization<Bearer>>>().await;

        match header_result {
            Ok(TypedHeader(Authorization(bearer))) => {
                validate_token(&state.jwt_keys, bearer.token()).map_err(|_| AuthError::InvalidToken)
            }
            Err(_) => {
                // No token – check whether this is a first-run (no users yet).
                let count = db::user_count(&state.db).await.unwrap_or(1);
                if count == 0 {
                    Err(AuthError::NeedsSetup)
                } else {
                    Err(AuthError::MissingToken)
                }
            }
        }
    }
}

/// Rejection type for the `Claims` extractor.
pub enum AuthError {
    /// No bearer token and users already exist → send to login.
    MissingToken,
    /// Token present but invalid/expired.
    InvalidToken,
    /// No bearer token AND no users in the database → first-run setup needed.
    NeedsSetup,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            Self::MissingToken => (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "error": "Missing Bearer token" })),
            )
                .into_response(),
            Self::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "error": "Invalid or expired token" })),
            )
                .into_response(),
            Self::NeedsSetup => (
                StatusCode::FORBIDDEN,
                axum::Json(serde_json::json!({ "needs_setup": true })),
            )
                .into_response(),
        }
    }
}

/// Extractor that requires the user to be an **admin**.
/// Returns `403 Forbidden` when the token is valid but the role is `user`.
pub struct AdminClaims(pub Claims);

impl<S> FromRequestParts<S> for AdminClaims
where
    S: Send + Sync + std::ops::Deref<Target = crate::state::AppState>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let claims = Claims::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;

        if !claims.is_admin() {
            return Err((
                StatusCode::FORBIDDEN,
                axum::Json(serde_json::json!({ "error": "Admin access required" })),
            )
                .into_response());
        }
        Ok(AdminClaims(claims))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keys() -> JwtKeys {
        JwtKeys::new(b"test-secret-key-for-unit-tests")
    }

    #[test]
    fn jwt_keys_new_creates_valid_keys() {
        let keys = test_keys();
        // Verify we can create and validate a token with the keys.
        let token = create_token(&keys, "u1", "alice", &UserRole::Admin);
        assert!(!token.is_empty());
    }

    #[test]
    fn create_token_produces_valid_token() {
        let keys = test_keys();
        let token = create_token(&keys, "user-42", "bob", &UserRole::User);
        let claims = validate_token(&keys, &token).expect("token should be valid");
        assert_eq!(claims.sub, "user-42");
        assert_eq!(claims.username, "bob");
        assert_eq!(claims.role, "user");
    }

    #[test]
    fn validate_token_accepts_valid_token() {
        let keys = test_keys();
        let token = create_token(&keys, "id-1", "carol", &UserRole::Admin);
        let claims = validate_token(&keys, &token).unwrap();
        assert_eq!(claims.sub, "id-1");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn validate_token_rejects_tampered_token() {
        let keys = test_keys();
        let token = create_token(&keys, "id-1", "carol", &UserRole::Admin);
        let tampered = format!("{token}x");
        assert!(validate_token(&keys, &tampered).is_err());
    }

    #[test]
    fn validate_token_rejects_wrong_secret() {
        let keys = test_keys();
        let other_keys = JwtKeys::new(b"different-secret");
        let token = create_token(&keys, "id-1", "carol", &UserRole::Admin);
        assert!(validate_token(&other_keys, &token).is_err());
    }

    #[test]
    fn claims_role_maps_admin() {
        let c = Claims {
            sub: String::new(),
            username: String::new(),
            role: "admin".into(),
            exp: 0,
        };
        assert_eq!(c.role(), UserRole::Admin);
    }

    #[test]
    fn claims_role_maps_user() {
        let c = Claims {
            sub: String::new(),
            username: String::new(),
            role: "user".into(),
            exp: 0,
        };
        assert_eq!(c.role(), UserRole::User);
    }

    #[test]
    fn claims_role_maps_unknown_to_user() {
        let c = Claims {
            sub: String::new(),
            username: String::new(),
            role: "unknown".into(),
            exp: 0,
        };
        assert_eq!(c.role(), UserRole::User);
    }

    #[test]
    fn is_admin_returns_true_for_admin() {
        let c = Claims {
            sub: String::new(),
            username: String::new(),
            role: "admin".into(),
            exp: 0,
        };
        assert!(c.is_admin());
    }

    #[test]
    fn is_admin_returns_false_for_user() {
        let c = Claims {
            sub: String::new(),
            username: String::new(),
            role: "user".into(),
            exp: 0,
        };
        assert!(!c.is_admin());
    }

    #[test]
    fn create_token_admin_role_roundtrip() {
        let keys = test_keys();
        let token = create_token(&keys, "id-a", "admin_user", &UserRole::Admin);
        let claims = validate_token(&keys, &token).unwrap();
        assert!(claims.is_admin());
        assert_eq!(claims.role(), UserRole::Admin);
    }

    #[test]
    fn create_token_user_role_roundtrip() {
        let keys = test_keys();
        let token = create_token(&keys, "id-u", "normal_user", &UserRole::User);
        let claims = validate_token(&keys, &token).unwrap();
        assert!(!claims.is_admin());
        assert_eq!(claims.role(), UserRole::User);
    }
}
