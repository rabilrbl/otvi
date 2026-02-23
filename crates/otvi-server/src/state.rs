use std::collections::HashMap;
use std::sync::RwLock;

use otvi_core::config::ProviderConfig;

use crate::auth_middleware::JwtKeys;
use crate::db::Db;

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
    /// Mirror of `PlaybackEndpoint::key_exclude_resolved_cookies`.
    /// When true, URL-param-extracted cookies (`resolved_cookies`) are not
    /// forwarded on AES-128 key file requests.
    pub key_exclude_resolved_cookies: bool,
}

fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to build HTTP client")
}

/// Shared application state injected into every Axum handler.
pub struct AppState {
    /// Provider ID → parsed YAML configuration.
    pub providers: HashMap<String, ProviderConfig>,
    /// Database connection pool (SQLite / PostgreSQL / MySQL via `AnyPool`).
    pub db: Db,
    /// JWT signing / verification keys.
    pub jwt_keys: JwtKeys,
    /// Shared HTTP client for outbound provider API calls.
    pub http_client: reqwest::Client,
    /// Opaque proxy-context token → per-stream proxy context.
    ///
    /// Populated by the stream endpoint; contains resolved headers and
    /// cookie mappings.  Only the opaque token is embedded in proxy URLs.
    pub proxy_ctx: RwLock<HashMap<String, ProxyContext>>,
}

impl AppState {
    /// Scan `dir` for `*.yaml` / `*.yml` files and parse each as a
    /// [`ProviderConfig`].
    pub fn load_providers(dir: &str, db: Db, jwt_keys: JwtKeys) -> anyhow::Result<Self> {
        let mut providers = HashMap::new();

        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    "Providers directory '{dir}' not found – starting with no providers"
                );
                return Ok(Self {
                    providers,
                    db,
                    jwt_keys,
                    http_client: build_http_client(),
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
                .is_some_and(|ext| ext == "yaml" || ext == "yml");
            if !is_yaml {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            match serde_yaml_ng::from_str::<ProviderConfig>(&content) {
                Ok(config) => {
                    tracing::info!(
                        "Loaded provider '{}' from {}",
                        config.provider.id,
                        path.display()
                    );
                    providers.insert(config.provider.id.clone(), config);
                }
                Err(e) => {
                    tracing::error!("Failed to parse {}: {e}", path.display());
                }
            }
        }

        Ok(Self {
            providers,
            db,
            jwt_keys,
            http_client: build_http_client(),
            proxy_ctx: RwLock::new(HashMap::new()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_middleware::JwtKeys;

    async fn test_db() -> (Db, tempfile::TempDir) {
        sqlx::any::install_default_drivers();
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}", db_path.display());
        let db = crate::db::init(&url).await.expect("test db init");
        (db, dir)
    }

    fn test_keys() -> JwtKeys {
        JwtKeys::new(b"test-secret")
    }

    #[tokio::test]
    async fn load_providers_nonexistent_dir_returns_empty() {
        let (db, _dir) = test_db().await;
        let tmp = tempfile::tempdir().expect("create temp dir");
        let nonexistent = tmp.path().join("does-not-exist");
        let state = AppState::load_providers(nonexistent.to_str().unwrap(), db, test_keys())
            .expect("should succeed with warning");
        assert!(state.providers.is_empty());
    }

    #[tokio::test]
    async fn load_providers_with_valid_yaml() {
        let (db, _db_dir) = test_db().await;
        let dir = tempfile::tempdir().expect("create temp dir");
        let yaml = r#"
provider:
  name: TestTV
  id: test_tv
auth:
  flows:
    - id: email
      name: Email Login
      inputs:
        - key: email
          label: Email
      steps:
        - name: login
          request:
            method: POST
            path: /api/login
channels:
  list:
    request:
      method: GET
      path: /api/channels
    response:
      items_path: "$.channels"
playback:
  stream:
    request:
      method: GET
      path: /api/play/{{input.id}}
    response:
      url: "$.url"
      type: "hls"
"#;
        std::fs::write(dir.path().join("test.yaml"), yaml).unwrap();
        let state =
            AppState::load_providers(dir.path().to_str().unwrap(), db, test_keys()).unwrap();
        assert_eq!(state.providers.len(), 1);
        assert!(state.providers.contains_key("test_tv"));
    }

    #[tokio::test]
    async fn load_providers_skips_non_yaml_files() {
        let (db, _db_dir) = test_db().await;
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("readme.txt"), "not yaml").unwrap();
        std::fs::write(dir.path().join("data.json"), "{}").unwrap();
        let state =
            AppState::load_providers(dir.path().to_str().unwrap(), db, test_keys()).unwrap();
        assert!(state.providers.is_empty());
    }

    #[tokio::test]
    async fn load_providers_loads_yml_extension() {
        let (db, _db_dir) = test_db().await;
        let dir = tempfile::tempdir().expect("create temp dir");
        let yaml = r#"
provider:
  name: YmlTV
  id: yml_tv
auth:
  flows:
    - id: email
      name: Email Login
      inputs:
        - key: email
          label: Email
      steps:
        - name: login
          request:
            method: POST
            path: /api/login
channels:
  list:
    request:
      method: GET
      path: /api/channels
    response:
      items_path: "$.channels"
playback:
  stream:
    request:
      method: GET
      path: /api/play/{{input.id}}
    response:
      url: "$.url"
      type: "hls"
"#;
        std::fs::write(dir.path().join("provider.yml"), yaml).unwrap();
        let state =
            AppState::load_providers(dir.path().to_str().unwrap(), db, test_keys()).unwrap();
        assert_eq!(state.providers.len(), 1);
        assert!(state.providers.contains_key("yml_tv"));
    }
}
