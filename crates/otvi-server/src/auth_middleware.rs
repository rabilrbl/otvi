//! JWT-based authentication for OTVI application users.
//!
//! Each browser session gets a short-lived JWT that encodes the user ID,
//! username, role, and the `must_change_password` flag.  The token is sent
//! via the `Authorization: Bearer …` header on every API request.
//!
//! The signing secret is read from the `JWT_SECRET` environment variable;
//! if not set, a random secret is generated at startup (tokens will not
//! survive a server restart in that case).
//!
//! ## Why embed `must_change_password` in the JWT?
//!
//! Previously, every call to [`ActiveClaims`] or [`AdminClaims`] issued a
//! `SELECT` against the `users` table to check the flag.  Since these
//! extractors run on **every** protected request, this created an unnecessary
//! DB round-trip on each call.
//!
//! Instead the flag is now encoded directly in the JWT at token-issuance time
//! (login / change-password / admin-reset).  The token is re-issued whenever
//! the flag changes, so the in-token value is always authoritative without any
//! extra database access at extraction time.
//!
//! ## Extractors
//!
//! | Extractor        | Requirement                                                   |
//! |------------------|---------------------------------------------------------------|
//! | [`Claims`]       | Valid JWT only — use for `me` and `change-password`          |
//! | [`ActiveClaims`] | Valid JWT **and** `must_change_password == false`             |
//! | [`AdminClaims`]  | Valid JWT, role `admin`, and `must_change_password == false`  |

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
    /// Whether the user must change their password before using the API.
    ///
    /// Embedded directly in the JWT so that [`ActiveClaims`] and
    /// [`AdminClaims`] can enforce the guard **without** a database round-trip
    /// on every request.  The token is re-issued whenever this flag changes
    /// (login, change-password, admin password-reset), keeping it current.
    #[serde(default)]
    pub must_change_password: bool,
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

/// Mint a new JWT for the given user.
///
/// `must_change_password` is embedded directly in the token payload so that
/// all middleware guards can inspect it without touching the database.
pub fn create_token(
    keys: &JwtKeys,
    user_id: &str,
    username: &str,
    role: &UserRole,
    must_change_password: bool,
) -> String {
    let exp = jsonwebtoken::get_current_timestamp() + TOKEN_TTL_SECS;
    let claims = Claims {
        sub: user_id.to_owned(),
        username: username.to_owned(),
        role: match role {
            UserRole::Admin => "admin".to_owned(),
            UserRole::User => "user".to_owned(),
        },
        must_change_password,
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

// ── must_change_password guard ─────────────────────────────────────────────

/// Check whether the authenticated user still has `must_change_password = true`.
///
/// This reads the flag directly from the JWT [`Claims`] — **no database query
/// is issued**.  The flag is embedded in the token at issuance time and the
/// token is re-minted every time the flag changes, so the in-token value is
/// always authoritative.
///
/// Returns a `403 Forbidden` response when the flag is set, `Ok(())` otherwise.
/// Used by [`ActiveClaims`] and [`AdminClaims`] so the check lives in one place.
fn assert_password_not_forced(claims: &Claims) -> Result<(), Response> {
    if claims.must_change_password {
        return Err((
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "You must change your password before using the application. \
                          Please visit the change-password page."
            })),
        )
            .into_response());
    }
    Ok(())
}

// ── ActiveClaims ───────────────────────────────────────────────────────────

/// Extractor that requires a valid JWT **and** that the user does not have an
/// active `must_change_password` flag.
///
/// Use this on every handler except `GET /api/auth/me` and
/// `POST /api/auth/change-password`, which must remain reachable while the
/// flag is set.
///
/// Implements `Deref<Target = Claims>` so handler code accesses fields
/// directly: `claims.sub`, `claims.role()`, etc. — no destructuring needed.
pub struct ActiveClaims(pub Claims);

impl std::ops::Deref for ActiveClaims {
    type Target = Claims;
    fn deref(&self) -> &Claims {
        &self.0
    }
}

impl<S> FromRequestParts<S> for ActiveClaims
where
    S: Send + Sync + std::ops::Deref<Target = crate::state::AppState>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let claims = Claims::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;

        // Guard is evaluated against the JWT claim — zero DB round-trips.
        assert_password_not_forced(&claims)?;

        Ok(ActiveClaims(claims))
    }
}

// ── AdminClaims ────────────────────────────────────────────────────────────

/// Extractor that requires the user to be an **admin** and does not have an
/// active `must_change_password` flag.
/// Returns `403 Forbidden` when the token is valid but the role is `user`.
///
/// Implements `Deref<Target = Claims>` so handler code accesses fields
/// directly: `claims.sub`, `claims.is_admin()`, etc. — no destructuring needed.
pub struct AdminClaims(pub Claims);

impl std::ops::Deref for AdminClaims {
    type Target = Claims;
    fn deref(&self) -> &Claims {
        &self.0
    }
}

impl<S> FromRequestParts<S> for AdminClaims
where
    S: Send + Sync + std::ops::Deref<Target = crate::state::AppState>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let claims = Claims::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;

        // Guard is evaluated against the JWT claim — zero DB round-trips.
        assert_password_not_forced(&claims)?;

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

    // ── Claims helpers ────────────────────────────────────────────────────

    fn make_claims(role: &str, must_change_password: bool) -> Claims {
        Claims {
            sub: "user-1".into(),
            username: "alice".into(),
            role: role.into(),
            must_change_password,
            exp: u64::MAX,
        }
    }

    // ── Role mapping ──────────────────────────────────────────────────────

    #[test]
    fn claims_role_maps_admin() {
        assert_eq!(make_claims("admin", false).role(), UserRole::Admin);
    }

    #[test]
    fn claims_role_maps_user() {
        assert_eq!(make_claims("user", false).role(), UserRole::User);
    }

    #[test]
    fn claims_role_maps_unknown_to_user() {
        assert_eq!(make_claims("superuser", false).role(), UserRole::User);
    }

    #[test]
    fn is_admin_returns_true_for_admin() {
        assert!(make_claims("admin", false).is_admin());
    }

    #[test]
    fn is_admin_returns_false_for_user() {
        assert!(!make_claims("user", false).is_admin());
    }

    // ── must_change_password guard ────────────────────────────────────────

    #[test]
    fn assert_password_not_forced_passes_when_flag_is_false() {
        let claims = make_claims("user", false);
        assert!(assert_password_not_forced(&claims).is_ok());
    }

    #[test]
    fn assert_password_not_forced_rejects_when_flag_is_true() {
        let claims = make_claims("user", true);
        assert!(assert_password_not_forced(&claims).is_err());
    }

    #[test]
    fn assert_password_not_forced_admin_also_rejected_when_flag_set() {
        // Admins are not exempt from the must_change_password guard.
        let claims = make_claims("admin", true);
        assert!(assert_password_not_forced(&claims).is_err());
    }

    #[test]
    fn assert_password_not_forced_returns_403() {
        let claims = make_claims("user", true);
        let resp = assert_password_not_forced(&claims).unwrap_err();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    // ── Token round-trip ──────────────────────────────────────────────────

    #[test]
    fn create_and_validate_token_round_trip() {
        let keys = JwtKeys::new(b"test-secret");
        let token = create_token(&keys, "u1", "bob", &UserRole::User, false);
        let claims = validate_token(&keys, &token).expect("valid token");
        assert_eq!(claims.sub, "u1");
        assert_eq!(claims.username, "bob");
        assert_eq!(claims.role, "user");
        assert!(!claims.must_change_password);
    }

    #[test]
    fn create_token_embeds_must_change_password_true() {
        let keys = JwtKeys::new(b"test-secret");
        let token = create_token(&keys, "u2", "carol", &UserRole::Admin, true);
        let claims = validate_token(&keys, &token).expect("valid token");
        assert_eq!(claims.role, "admin");
        assert!(claims.must_change_password);
    }

    #[test]
    fn create_token_embeds_must_change_password_false() {
        let keys = JwtKeys::new(b"test-secret");
        let token = create_token(&keys, "u3", "dave", &UserRole::User, false);
        let claims = validate_token(&keys, &token).expect("valid token");
        assert!(!claims.must_change_password);
    }

    #[test]
    fn validate_token_rejects_wrong_secret() {
        let keys_a = JwtKeys::new(b"secret-a");
        let keys_b = JwtKeys::new(b"secret-b");
        let token = create_token(&keys_a, "u1", "alice", &UserRole::User, false);
        assert!(validate_token(&keys_b, &token).is_err());
    }

    #[test]
    fn validate_token_rejects_tampered_payload() {
        let keys = JwtKeys::new(b"test-secret");
        let token = create_token(&keys, "u1", "alice", &UserRole::User, false);
        // Flip a character in the payload segment (index 1 of the three JWT parts).
        let parts: Vec<&str> = token.splitn(3, '.').collect();
        assert_eq!(parts.len(), 3);
        let mut bad_payload = parts[1].to_string();
        // Replace the last character to corrupt the payload.
        bad_payload.pop();
        bad_payload.push(if bad_payload.ends_with('a') { 'b' } else { 'a' });
        let tampered = format!("{}.{}.{}", parts[0], bad_payload, parts[2]);
        assert!(validate_token(&keys, &tampered).is_err());
    }

    #[test]
    fn validate_token_rejects_expired_token() {
        let keys = JwtKeys::new(b"test-secret");
        // Manually build a token that expired in the past.
        let claims = Claims {
            sub: "u1".into(),
            username: "alice".into(),
            role: "user".into(),
            must_change_password: false,
            exp: 1, // Unix epoch + 1s — definitely in the past.
        };
        let token =
            jsonwebtoken::encode(&Header::default(), &claims, &keys.encoding).expect("encode");
        assert!(validate_token(&keys, &token).is_err());
    }

    // ── AuthError responses ───────────────────────────────────────────────

    #[test]
    fn auth_error_missing_token_is_401() {
        let resp = AuthError::MissingToken.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn auth_error_invalid_token_is_401() {
        let resp = AuthError::InvalidToken.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn auth_error_needs_setup_is_403() {
        let resp = AuthError::NeedsSetup.into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
