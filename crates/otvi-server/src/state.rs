use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

/// Server-side context stored per stream session so that sensitive values
/// (auth headers, cookie mappings) never travel in URLs.
#[derive(Debug, Clone, Default)]
pub struct ProxyContext {
    /// HTTP headers to apply to every upstream proxy request.
    pub headers: HashMap<String, String>,
    /// URL query-param name → cookie name.
    /// The proxy extracts the param from the upstream URL and sends it as the
    /// named cookie, enabling CDNs that authenticate via cookies (e.g. Akamai
    /// `hdnea`) to work correctly for segments and encryption-key requests.
    pub url_param_cookies: HashMap<String, String>,
    /// Cookie values extracted from a previously-seen manifest URL and cached
    /// here so that sub-requests whose URLs carry no query params (e.g. bare
    /// `.pkey` encryption-key files) still get the correct cookies.
    pub resolved_cookies: HashMap<String, String>,
    /// Static cookie values resolved from the provider YAML (`proxy_cookies`)
    /// and sent verbatim on every upstream request.  Unlike `url_param_cookies`
    /// these are not extracted from the upstream URL; they are resolved once at
    /// session creation time (template vars expanded) and stored here.
    pub static_cookies: HashMap<String, String>,
    /// The raw query string from the most recent manifest URL that carried
    /// query params.  Saved here so that `EXT-X-KEY` sub-requests, which
    /// normally have no query params, can have the manifest params appended
    /// when `append_manifest_query_to_key_uris` is `true`.
    pub manifest_query: Option<String>,
    /// Mirror of `PlaybackEndpoint::append_manifest_query_to_key_uris`.
    pub append_manifest_query_to_key_uris: bool,
}

fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to build HTTP client")
}

use serde::{Deserialize, Serialize};

use otvi_core::config::ProviderConfig;

/// Default file name used to persist sessions across server restarts.
const SESSIONS_FILE: &str = "sessions.json";

/// Shared application state.
pub struct AppState {
    /// Provider ID → parsed YAML configuration.
    pub providers: HashMap<String, ProviderConfig>,
    /// Session token → session data, persisted to disk.
    pub sessions: RwLock<HashMap<String, SessionData>>,
    /// Shared HTTP client for outbound provider API calls.
    pub http_client: reqwest::Client,
    /// Path to the sessions persistence file.
    pub sessions_path: PathBuf,
    /// Opaque proxy-context token → per-stream proxy context.
    ///
    /// Populated by the stream endpoint; contains resolved headers and
    /// cookie mappings.  Only the opaque token is embedded in proxy URLs.
    pub proxy_ctx: RwLock<HashMap<String, ProxyContext>>,
}

/// Per-session data stored on the server side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub provider_id: String,
    /// Long-lived values extracted during auth (access_token, user_id, …).
    pub stored_values: HashMap<String, String>,
    /// Values extracted during the most recent auth step (used for multi-step
    /// flows where an intermediate value like `request_id` is needed).
    pub step_extracts: HashMap<String, String>,
}

impl AppState {
    /// Scan `dir` for `*.yaml` / `*.yml` files and parse each as a
    /// [`ProviderConfig`].  Also loads any previously persisted sessions.
    pub fn load_providers(dir: &str) -> anyhow::Result<Self> {
        let mut providers = HashMap::new();

        // Derive sessions file path from the providers directory parent
        let sessions_path = PathBuf::from(dir)
            .parent()
            .map(|p| p.join(SESSIONS_FILE))
            .unwrap_or_else(|| PathBuf::from(SESSIONS_FILE));

        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    "Providers directory '{dir}' not found – starting with no providers"
                );
                let sessions = Self::load_sessions(&sessions_path);
                return Ok(Self {
                    providers,
                    sessions: RwLock::new(sessions),
                    http_client: build_http_client(),
                    sessions_path,
                    proxy_ctx: RwLock::new(HashMap::new()),
                });
            }
            Err(e) => return Err(e.into()),
        };

        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();
            let is_yaml = path
                .extension()
                .map_or(false, |ext| ext == "yaml" || ext == "yml");
            if !is_yaml {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            match serde_yaml::from_str::<ProviderConfig>(&content) {
                Ok(config) => {
                    tracing::info!("Loaded provider '{}' from {}", config.provider.id, path.display());
                    providers.insert(config.provider.id.clone(), config);
                }
                Err(e) => {
                    tracing::error!("Failed to parse {}: {e}", path.display());
                }
            }
        }

        let sessions = Self::load_sessions(&sessions_path);
        let n = sessions.len();
        if n > 0 {
            tracing::info!("Restored {n} session(s) from {}", sessions_path.display());
        }

        Ok(Self {
            providers,
            sessions: RwLock::new(sessions),
            http_client: build_http_client(),
            sessions_path,
            proxy_ctx: RwLock::new(HashMap::new()),
        })
    }

    /// Load sessions from the JSON file, returning an empty map on any error.
    fn load_sessions(path: &PathBuf) -> HashMap<String, SessionData> {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse sessions file: {e}");
                HashMap::new()
            }),
            Err(_) => HashMap::new(),
        }
    }

    /// Persist the current sessions map to disk.  Call this after any mutation.
    pub fn save_sessions(&self) {
        let sessions = self.sessions.read().unwrap();
        match serde_json::to_string(&*sessions) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.sessions_path, json) {
                    tracing::error!("Failed to persist sessions: {e}");
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize sessions: {e}");
            }
        }
    }
}
