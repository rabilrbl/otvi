//! YAML provider configuration schema.
//!
//! Each TV provider is described by a YAML file that maps closely to the HTTP
//! requests captured from the provider's mobile/Android TV app.  A developer
//! can use a proxy like mitmproxy or Charles, record the traffic, and convert
//! the captured requests into this YAML format.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Root ────────────────────────────────────────────────────────────────────

/// Top-level provider configuration loaded from a single YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: ProviderMeta,
    #[serde(default)]
    pub defaults: RequestDefaults,
    pub auth: AuthConfig,
    pub channels: ChannelsConfig,
    pub playback: PlaybackConfig,
}

// ── Provider metadata ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMeta {
    pub name: String,
    pub id: String,
    #[serde(default)]
    pub logo: Option<String>,
}

// ── Request defaults ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestDefaults {
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

// ── Auth ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub flows: Vec<AuthFlow>,
    #[serde(default)]
    pub logout: Option<ApiCall>,
    #[serde(default)]
    pub refresh: Option<RefreshConfig>,
}

/// A single authentication flow (e.g. email+password, phone+OTP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFlow {
    pub id: String,
    pub name: String,
    pub inputs: Vec<FieldDef>,
    pub steps: Vec<AuthStep>,
}

/// Definition of a user-facing form field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub key: String,
    pub label: String,
    #[serde(rename = "type", default = "default_field_type")]
    pub field_type: String,
    #[serde(default = "default_true")]
    pub required: bool,
    /// Optional transform applied to the input value before it is used in
    /// templates.  Supported: `"base64"` (base64-encode the raw value).
    #[serde(default)]
    pub transform: Option<String>,
}

fn default_field_type() -> String {
    "text".to_string()
}

fn default_true() -> bool {
    true
}

/// A single step within an authentication flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStep {
    pub name: String,
    pub request: RequestSpec,
    #[serde(default)]
    pub on_success: Option<OnSuccess>,
    /// Expected HTTP status code for a successful response (default: any 2xx).
    /// Useful when a provider returns 204 No Content with an empty body.
    #[serde(default)]
    pub success_status: Option<u16>,
}

/// Actions to perform on a successful API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnSuccess {
    /// Key → JSONPath pairs.  Values extracted from the response body are
    /// stored in the session and available as `{{stored.<key>}}` in later
    /// template strings.
    #[serde(default)]
    pub extract: HashMap<String, String>,
    /// If present, the frontend will prompt for these additional fields before
    /// continuing to the next step.
    #[serde(default)]
    pub prompt: Option<Vec<FieldDef>>,
}

// ── Request specification ───────────────────────────────────────────────────

/// Describes a single HTTP request.  All string fields support template
/// variables such as `{{input.email}}`, `{{stored.access_token}}`, `{{uuid}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSpec {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub params: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    /// Encoding for the request body.  Defaults to `"json"`.  Set to
    /// `"form"` to send `application/x-www-form-urlencoded` data.
    #[serde(default = "default_body_encoding")]
    pub body_encoding: String,
}

fn default_body_encoding() -> String {
    "json".to_string()
}

/// Wrapper for a standalone API call (e.g. logout).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCall {
    pub request: RequestSpec,
}

/// Configuration for automatic token refresh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshConfig {
    pub request: RequestSpec,
    pub on_success: OnSuccess,
}

// ── Channels ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsConfig {
    pub list: ApiEndpoint,
    #[serde(default)]
    pub categories: Option<ApiEndpoint>,
}

/// Generic API endpoint with request and response-mapping information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub request: RequestSpec,
    pub response: ResponseMapping,
}

/// Describes how to extract a list of items from a JSON response and map
/// provider-specific field names to the canonical schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMapping {
    /// JSONPath to the array of items, e.g. `$.data.channels`.
    #[serde(default)]
    pub items_path: Option<String>,
    /// Map of canonical field name → JSONPath within each item.
    #[serde(default)]
    pub mapping: HashMap<String, String>,
}

// ── Playback ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackConfig {
    pub stream: PlaybackEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackEndpoint {
    pub request: RequestSpec,
    pub response: PlaybackResponse,
}

/// Describes how to extract stream URL, type and optional DRM information from
/// the playback API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackResponse {
    /// JSONPath to the manifest / playlist URL.
    pub url: String,
    /// JSONPath to the stream type string (`"hls"` or `"dash"`).  Can also be
    /// a literal value (not starting with `$.`) when the type is fixed.
    #[serde(rename = "type")]
    pub stream_type: String,
    #[serde(default)]
    pub drm: Option<DrmResponseConfig>,
}

/// Describes how to extract DRM parameters from the playback response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrmResponseConfig {
    /// JSONPath to the DRM system name (e.g. `"widevine"`).
    pub system: String,
    /// Template or JSONPath for the license server URL.
    pub license_url: String,
    /// Extra headers to send with DRM license requests.
    #[serde(default)]
    pub headers: HashMap<String, String>,
}
