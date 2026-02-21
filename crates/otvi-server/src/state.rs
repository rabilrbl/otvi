use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

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
                    http_client: reqwest::Client::new(),
                    sessions_path,
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
            http_client: reqwest::Client::new(),
            sessions_path,
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
