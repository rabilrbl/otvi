//! Shared request / response types used by both the backend REST API and the
//! frontend WASM client.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Provider info (read-only, returned to frontend) ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub logo: Option<String>,
    pub auth_flows: Vec<AuthFlowInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFlowInfo {
    pub id: String,
    pub name: String,
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub key: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
}

// ── Auth request / response ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub flow_id: String,
    pub step: usize,
    pub inputs: HashMap<String, String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub next_step: Option<NextStepInfo>,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Returned when a multi-step auth flow requires additional user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStepInfo {
    pub step_index: usize,
    pub step_name: String,
    pub fields: Vec<FieldInfo>,
}

// ── Channels ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelListResponse {
    pub channels: Vec<Channel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryListResponse {
    pub categories: Vec<Category>,
}

// ── Playback ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub url: String,
    pub stream_type: StreamType,
    #[serde(default)]
    pub drm: Option<DrmInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    Hls,
    Dash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrmInfo {
    pub system: String,
    pub license_url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

// ── OTVI user account (application-level auth, independent of providers) ────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    User,
}

/// Information about the currently authenticated OTVI user.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub role: UserRole,
    /// Provider IDs this user has access to.
    pub providers: Vec<String>,
    /// When `true` the user must change their password before proceeding.
    #[serde(default)]
    pub must_change_password: bool,
}

// ── OTVI app-level register / login / logout ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLoginResponse {
    pub token: String,
    pub user: UserInfo,
}

// ── Admin: user management ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: UserRole,
    /// Provider IDs to grant access to (empty = all providers).
    #[serde(default)]
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserProvidersRequest {
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminResetPasswordRequest {
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    /// When `true`, the public `/api/auth/register` endpoint is disabled;
    /// only admins can create new accounts via `/api/admin/users`.
    pub signup_disabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_type_serialization() {
        assert_eq!(serde_json::to_string(&StreamType::Hls).unwrap(), "\"hls\"");
        assert_eq!(
            serde_json::to_string(&StreamType::Dash).unwrap(),
            "\"dash\""
        );
    }

    #[test]
    fn stream_type_deserialization() {
        let hls: StreamType = serde_json::from_str("\"hls\"").unwrap();
        assert!(matches!(hls, StreamType::Hls));
        let dash: StreamType = serde_json::from_str("\"dash\"").unwrap();
        assert!(matches!(dash, StreamType::Dash));
    }

    #[test]
    fn user_role_serialization() {
        assert_eq!(
            serde_json::to_string(&UserRole::Admin).unwrap(),
            "\"admin\""
        );
        assert_eq!(serde_json::to_string(&UserRole::User).unwrap(), "\"user\"");
    }

    #[test]
    fn user_role_deserialization() {
        let admin: UserRole = serde_json::from_str("\"admin\"").unwrap();
        assert_eq!(admin, UserRole::Admin);
        let user: UserRole = serde_json::from_str("\"user\"").unwrap();
        assert_eq!(user, UserRole::User);
    }

    #[test]
    fn login_request_roundtrip() {
        let mut inputs = HashMap::new();
        inputs.insert("email".into(), "user@example.com".into());
        let req = LoginRequest {
            flow_id: "email".into(),
            step: 0,
            inputs,
            session_id: Some("sess-123".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: LoginRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.flow_id, "email");
        assert_eq!(decoded.step, 0);
        assert_eq!(decoded.inputs.get("email").unwrap(), "user@example.com");
        assert_eq!(decoded.session_id, Some("sess-123".into()));
    }

    #[test]
    fn login_request_optional_session_id() {
        let json = r#"{"flow_id":"f","step":1,"inputs":{}}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert!(req.session_id.is_none());
    }

    #[test]
    fn login_response_roundtrip() {
        let resp = LoginResponse {
            success: true,
            session_id: Some("abc".into()),
            next_step: None,
            user_name: Some("Alice".into()),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: LoginResponse = serde_json::from_str(&json).unwrap();
        assert!(decoded.success);
        assert_eq!(decoded.session_id, Some("abc".into()));
        assert_eq!(decoded.user_name, Some("Alice".into()));
        assert!(decoded.error.is_none());
    }

    #[test]
    fn login_response_minimal() {
        let json = r#"{"success":false}"#;
        let resp: LoginResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.success);
        assert!(resp.session_id.is_none());
        assert!(resp.next_step.is_none());
        assert!(resp.user_name.is_none());
        assert!(resp.error.is_none());
    }

    #[test]
    fn channel_with_all_fields() {
        let ch = Channel {
            id: "ch1".into(),
            name: "News".into(),
            logo: Some("https://img.example.com/news.png".into()),
            category: Some("news".into()),
            number: Some("101".into()),
            description: Some("24/7 news channel".into()),
        };
        let json = serde_json::to_string(&ch).unwrap();
        let decoded: Channel = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "ch1");
        assert_eq!(
            decoded.logo,
            Some("https://img.example.com/news.png".into())
        );
        assert_eq!(decoded.category, Some("news".into()));
        assert_eq!(decoded.number, Some("101".into()));
        assert_eq!(decoded.description, Some("24/7 news channel".into()));
    }

    #[test]
    fn channel_with_optional_fields_omitted() {
        let json = r#"{"id":"ch2","name":"Sports"}"#;
        let ch: Channel = serde_json::from_str(json).unwrap();
        assert_eq!(ch.id, "ch2");
        assert_eq!(ch.name, "Sports");
        assert!(ch.logo.is_none());
        assert!(ch.category.is_none());
        assert!(ch.number.is_none());
        assert!(ch.description.is_none());
    }

    #[test]
    fn user_info_roundtrip() {
        let user = UserInfo {
            id: "u1".into(),
            username: "alice".into(),
            role: UserRole::Admin,
            providers: vec!["prov1".into(), "prov2".into()],
            must_change_password: false,
        };
        let json = serde_json::to_string(&user).unwrap();
        let decoded: UserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, user);
    }

    #[test]
    fn user_info_default_must_change_password() {
        let json = r#"{"id":"u2","username":"bob","role":"user","providers":[]}"#;
        let user: UserInfo = serde_json::from_str(json).unwrap();
        assert!(!user.must_change_password);
        assert_eq!(user.role, UserRole::User);
        assert!(user.providers.is_empty());
    }

    #[test]
    fn stream_info_with_drm() {
        let info = StreamInfo {
            url: "https://stream.example.com/manifest.m3u8".into(),
            stream_type: StreamType::Hls,
            drm: Some(DrmInfo {
                system: "widevine".into(),
                license_url: "https://drm.example.com/license".into(),
                headers: HashMap::new(),
            }),
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: StreamInfo = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded.stream_type, StreamType::Hls));
        assert!(decoded.drm.is_some());
        assert_eq!(decoded.drm.unwrap().system, "widevine");
    }

    #[test]
    fn stream_info_without_drm() {
        let json = r#"{"url":"https://stream.example.com/index.mpd","stream_type":"dash"}"#;
        let info: StreamInfo = serde_json::from_str(json).unwrap();
        assert!(matches!(info.stream_type, StreamType::Dash));
        assert!(info.drm.is_none());
    }

    #[test]
    fn channel_list_response_roundtrip() {
        let resp = ChannelListResponse {
            channels: vec![Channel {
                id: "c1".into(),
                name: "One".into(),
                logo: None,
                category: None,
                number: None,
                description: None,
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: ChannelListResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.channels.len(), 1);
        assert_eq!(decoded.channels[0].name, "One");
    }

    #[test]
    fn app_login_response_roundtrip() {
        let resp = AppLoginResponse {
            token: "jwt-token".into(),
            user: UserInfo {
                id: "u1".into(),
                username: "admin".into(),
                role: UserRole::Admin,
                providers: vec![],
                must_change_password: true,
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: AppLoginResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.token, "jwt-token");
        assert!(decoded.user.must_change_password);
    }
}
