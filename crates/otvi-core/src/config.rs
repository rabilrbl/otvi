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

/// Controls who manages the provider's authentication credentials.
///
/// - `global`   – An admin logs in once; every authenticated user of this
///                OTVI instance shares those provider credentials.
/// - `per_user` – Every OTVI user supplies their own provider credentials.
///                Each user has an independent provider session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthScope {
    /// Admin supplies credentials once; shared across all users.
    Global,
    /// Each user must log in with their own provider credentials.
    #[default]
    PerUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Who manages credentials for this provider (default: `per_user`).
    #[serde(default)]
    pub scope: AuthScope,
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

/// A statically-defined category (used when the provider has no categories API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticCategory {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsConfig {
    pub list: ApiEndpoint,
    #[serde(default)]
    pub categories: Option<ApiEndpoint>,
    /// Inline static category list, used when the provider does not expose a
    /// categories API endpoint (e.g. categories are embedded in channel data).
    #[serde(default)]
    pub static_categories: Vec<StaticCategory>,
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
    /// Optional base URL prepended to relative logo URLs extracted from the
    /// channel list response.  Use this when the provider API returns only a
    /// filename or path for the channel logo rather than a full URL.
    ///
    /// Example: `"https://jiotv.cdn.jio.com/apis/v1.3/getLogoUrl/get/"`
    #[serde(default)]
    pub logo_base_url: Option<String>,
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
    /// Extra HTTP headers forwarded by the proxy when fetching stream segments
    /// and manifests on behalf of the browser.  Supports the same
    /// `{{stored.*}}` / `{{input.*}}` template variables as request specs.
    #[serde(default)]
    pub proxy_headers: HashMap<String, String>,
    /// Maps a URL query-parameter name in the upstream stream URL to a cookie
    /// name that the proxy should send on every sub-request (segments, keys…).
    ///
    /// Some CDNs (e.g. Akamai) embed auth tokens as URL query params in the
    /// manifest URL but authenticate segment/key requests via a cookie.  List
    /// them here so the proxy forwards them correctly.
    ///
    /// Example (JioTV / Akamai `hdnea`):
    /// ```yaml
    /// proxy_url_cookies:
    ///   hdnea: "__hdnea__"
    /// ```
    #[serde(default)]
    pub proxy_url_cookies: HashMap<String, String>,
    /// Static cookie values sent verbatim on every upstream proxy request.
    /// Supports the same `{{stored.*}}` template variables as `proxy_headers`.
    ///
    /// Use this when the upstream CDN or origin authenticates requests via
    /// HTTP cookies rather than (or in addition to) request headers.
    ///
    /// Example (JioTV key-file auth via user tokens):
    /// ```yaml
    /// proxy_cookies:
    ///   ssotoken: "{{stored.sso_token}}"
    ///   crmid: "{{stored.crm}}"
    /// ```
    #[serde(default)]
    pub proxy_cookies: HashMap<String, String>,
    /// When `true`, the raw query string from the first manifest URL that
    /// carries query params is appended to every `EXT-X-KEY` URI before
    /// the proxy fetches the key file from upstream.
    ///
    /// Set this when the upstream CDN requires the same auth token that
    /// appears in the manifest URL to also be present as a query param on
    /// encryption-key requests.
    #[serde(default)]
    pub append_manifest_query_to_key_uris: bool,
    /// When `true`, URL-param-extracted cookies (i.e. `resolved_cookies`
    /// populated from the manifest URL's query string via `proxy_url_cookies`)
    /// are **not** forwarded on AES-128 key requests.
    ///
    /// Set this when the key server lives on a different domain from the
    /// segment CDN and does not accept (or actively rejects) the CDN auth
    /// token (e.g. Akamai `__hdnea__` with an ACL that covers only the CDN
    /// path, not the key-server path).  Static cookies from `proxy_cookies`
    /// are still forwarded; only URL-extracted tokens are suppressed.
    #[serde(default)]
    pub key_exclude_resolved_cookies: bool,
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
