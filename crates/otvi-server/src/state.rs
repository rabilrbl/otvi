use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

// ── Rate limiting ──────────────────────────────────────────────────────────

/// Default burst size for the auth-tier rate limiter (login / register).
pub const DEFAULT_RATE_LIMIT_AUTH_BURST: u32 = 5;
/// Default replenishment period (seconds) for the auth-tier rate limiter.
pub const DEFAULT_RATE_LIMIT_AUTH_PERIOD_SECS: u64 = 10;
/// Default burst size for the general-tier rate limiter (all other API routes).
pub const DEFAULT_RATE_LIMIT_GENERAL_BURST: u32 = 20;
/// Default replenishment period (seconds) for the general-tier rate limiter.
pub const DEFAULT_RATE_LIMIT_GENERAL_PERIOD_SECS: u64 = 1;

/// Configuration for the two-tier IP-based rate limiter.
///
/// Both tiers use a [token-bucket](https://en.wikipedia.org/wiki/Token_bucket)
/// algorithm keyed by peer IP address.  Each IP starts with a full bucket of
/// `burst` tokens; one token is consumed per request, and one token is
/// replenished every `period_secs` seconds.
///
/// | Tier    | Protects                                          | Default quota                  |
/// |---------|---------------------------------------------------|--------------------------------|
/// | Auth    | `POST /api/auth/login`, `/api/auth/register`, `POST /api/*/auth/login` | 5 req burst, +1 every 10 s |
/// | General | All other `/api` routes                           | 20 req burst, +1 every 1 s    |
///
/// ## Environment variables
///
/// | Variable                          | Default | Description                                       |
/// |-----------------------------------|---------|---------------------------------------------------|
/// | `RATE_LIMIT_AUTH_BURST`           | `5`     | Auth-tier token bucket burst capacity             |
/// | `RATE_LIMIT_AUTH_PERIOD_SECS`     | `10`    | Auth-tier replenishment interval in seconds       |
/// | `RATE_LIMIT_GENERAL_BURST`        | `20`    | General-tier token bucket burst capacity          |
/// | `RATE_LIMIT_GENERAL_PERIOD_SECS`  | `1`     | General-tier replenishment interval in seconds    |
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Burst capacity for auth-sensitive endpoints.
    pub auth_burst: u32,
    /// Token replenishment interval (seconds) for auth-sensitive endpoints.
    pub auth_period_secs: u64,
    /// Burst capacity for general API endpoints.
    pub general_burst: u32,
    /// Token replenishment interval (seconds) for general API endpoints.
    pub general_period_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            auth_burst: DEFAULT_RATE_LIMIT_AUTH_BURST,
            auth_period_secs: DEFAULT_RATE_LIMIT_AUTH_PERIOD_SECS,
            general_burst: DEFAULT_RATE_LIMIT_GENERAL_BURST,
            general_period_secs: DEFAULT_RATE_LIMIT_GENERAL_PERIOD_SECS,
        }
    }
}

impl RateLimitConfig {
    /// Construct a `RateLimitConfig` from environment variables, falling back
    /// to the compiled-in defaults for any variable that is absent or cannot
    /// be parsed.
    pub fn from_env() -> Self {
        fn parse_env<T: std::str::FromStr>(name: &str, default: T) -> T {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }

        let cfg = Self {
            auth_burst: parse_env("RATE_LIMIT_AUTH_BURST", DEFAULT_RATE_LIMIT_AUTH_BURST),
            auth_period_secs: parse_env(
                "RATE_LIMIT_AUTH_PERIOD_SECS",
                DEFAULT_RATE_LIMIT_AUTH_PERIOD_SECS,
            ),
            general_burst: parse_env("RATE_LIMIT_GENERAL_BURST", DEFAULT_RATE_LIMIT_GENERAL_BURST),
            general_period_secs: parse_env(
                "RATE_LIMIT_GENERAL_PERIOD_SECS",
                DEFAULT_RATE_LIMIT_GENERAL_PERIOD_SECS,
            ),
        };

        tracing::info!(
            auth_burst = cfg.auth_burst,
            auth_period_secs = cfg.auth_period_secs,
            general_burst = cfg.general_burst,
            general_period_secs = cfg.general_period_secs,
            "Rate limit configured",
        );

        cfg
    }
}

use moka::future::Cache;
use otvi_core::config::ProviderConfig;
use otvi_core::types::{Category, Channel};

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
    /// Mirror of `PlaybackEndpoint::key_uri_patterns`.
    /// Substring patterns used to identify key URIs within `EXT-X-KEY` tags
    /// when deciding whether to append the manifest query.  An empty list
    /// means "apply to all key-tag URIs".
    pub key_uri_patterns: Vec<String>,
}

// ── Channel cache ──────────────────────────────────────────────────────────

/// The default time-to-live for cached channel lists and category lists.
///
/// 24 hours is appropriate because channel lineups are stable and entries are
/// invalidated explicitly on provider login / logout, so stale data is never
/// served after a credential change regardless of the TTL.
/// Operators can override this via the `CHANNEL_CACHE_TTL_SECS` environment variable.
pub const DEFAULT_CHANNEL_CACHE_TTL_SECS: u64 = 86_400;

/// Maximum number of distinct cache entries (unique cache keys).
///
/// Each provider × auth-scope-uid combination generates its own key, so this
/// should comfortably cover realistic deployments (hundreds of providers × users).
const CHANNEL_CACHE_MAX_ENTRIES: u64 = 1_024;

/// Identifies whose session a cache entry belongs to.
///
/// Using an explicit enum rather than a stringly-typed sentinel (e.g. `""`)
/// makes the two cases self-documenting and eliminates any possibility of a
/// caller accidentally passing an empty string where a real user ID was meant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheScope {
    /// The provider uses `AuthScope::Global`: a single shared upstream session
    /// exists for all OTVI users, so one cache entry covers every user.
    Global,
    /// The provider uses `AuthScope::PerUser`: each OTVI user has their own
    /// upstream session, so cache entries are isolated by user ID.
    PerUser(String),
}

impl CacheScope {
    /// Construct a `CacheScope` from a provider's [`AuthScope`] and the
    /// requesting user's ID.
    pub fn from_auth_scope(scope: &otvi_core::config::AuthScope, user_id: &str) -> Self {
        match scope {
            otvi_core::config::AuthScope::Global => Self::Global,
            otvi_core::config::AuthScope::PerUser => Self::PerUser(user_id.to_owned()),
        }
    }
}

/// Cache key for the channel-list and category caches.
///
/// Keyed by `(provider_id, scope)` — `scope` encodes whether the entry is
/// shared across all users (global) or isolated to a specific user (per-user).
/// Query parameters (search, category, limit, offset) are applied server-side
/// on top of the full cached list, so only the raw upstream response is cached.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelCacheKey {
    pub provider_id: String,
    pub scope: CacheScope,
}

impl ChannelCacheKey {
    pub fn global(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            scope: CacheScope::Global,
        }
    }

    pub fn per_user(provider_id: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            scope: CacheScope::PerUser(user_id.into()),
        }
    }

    /// Construct from a provider's [`AuthScope`] and the requesting user's ID.
    /// Prefer this over calling `global` / `per_user` directly in handlers.
    pub fn from_auth_scope(
        provider_id: impl Into<String>,
        scope: &otvi_core::config::AuthScope,
        user_id: &str,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            scope: CacheScope::from_auth_scope(scope, user_id),
        }
    }
}

/// Cached payload for the channel-list endpoint.
#[derive(Debug, Clone)]
pub struct CachedChannels {
    /// The full, unfiltered, unpaginated list returned by the upstream provider.
    pub channels: Vec<Channel>,
}

/// Cached payload for the categories endpoint.
#[derive(Debug, Clone)]
pub struct CachedCategories {
    pub categories: Vec<Category>,
}

/// In-memory TTL caches for the channel and category listing endpoints.
///
/// Both caches are backed by [`moka`]'s async-aware `Cache`, which handles
/// concurrent access and automatic eviction without holding any synchronous
/// lock across an `await` point.
///
/// ## Invalidation
///
/// Entries are evicted automatically after their TTL expires.  They are also
/// invalidated explicitly when a provider session changes (login / logout) via
/// [`ChannelCache::invalidate_provider`], which removes all entries for a
/// given provider + uid combination.
pub struct ChannelCache {
    /// Cache for the full channel list per `(provider_id, session_uid)`.
    pub channels: Cache<ChannelCacheKey, CachedChannels>,
    /// Cache for the category list per `(provider_id, session_uid)`.
    pub categories: Cache<ChannelCacheKey, CachedCategories>,
}

impl ChannelCache {
    /// Create a new `ChannelCache` with the given TTL.
    pub fn new(ttl: Duration) -> Self {
        let channels = Cache::builder()
            .max_capacity(CHANNEL_CACHE_MAX_ENTRIES)
            .time_to_live(ttl)
            .build();

        let categories = Cache::builder()
            .max_capacity(CHANNEL_CACHE_MAX_ENTRIES)
            .time_to_live(ttl)
            .build();

        Self {
            channels,
            categories,
        }
    }

    /// Construct a `ChannelCache` reading the TTL from the
    /// `CHANNEL_CACHE_TTL_SECS` environment variable, falling back to
    /// [`DEFAULT_CHANNEL_CACHE_TTL_SECS`] when the variable is absent or
    /// cannot be parsed.
    pub fn from_env() -> Self {
        let secs = std::env::var("CHANNEL_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_CHANNEL_CACHE_TTL_SECS);

        tracing::info!(
            ttl_secs = secs,
            ttl_hours = secs / 3600,
            "Channel cache TTL configured",
        );
        Self::new(Duration::from_secs(secs))
    }

    /// Evict all channel-list and category-list entries for the given
    /// `(provider_id, scope)` pair.
    ///
    /// Call this after a provider session is created or destroyed (login /
    /// logout) so that the next listing reflects the updated credentials.
    pub async fn invalidate(&self, key: &ChannelCacheKey) {
        self.channels.invalidate(key).await;
        self.categories.invalidate(key).await;
    }
}

fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to build HTTP client")
}

/// Shared application state injected into every Axum handler.
///
/// The provider map is stored in an `RwLock` so the hot-reload watcher can
/// atomically swap its contents without restarting the server.  All read
/// accesses go through [`AppState::with_provider`] or
/// [`AppState::with_providers`] which acquire a short-lived read guard.
pub struct AppState {
    /// Provider ID → parsed YAML configuration (hot-reloadable).
    pub providers_rw: RwLock<HashMap<String, ProviderConfig>>,
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
    /// In-memory TTL cache for channel list and category responses.
    ///
    /// Caching the full upstream response server-side means that server-side
    /// filtering, search, and pagination are applied against the cached data
    /// on every request, while expensive upstream HTTP calls are amortised
    /// across the TTL window.
    pub channel_cache: ChannelCache,
}

impl AppState {
    /// Run `f` with an immutable reference to the provider identified by `id`.
    ///
    /// Returns `None` when the provider is not found or the lock is poisoned.
    pub fn with_provider<F, R>(&self, id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&ProviderConfig) -> R,
    {
        self.providers_rw
            .read()
            .ok()
            .and_then(|guard| guard.get(id).map(f))
    }

    /// Run `f` with a snapshot clone of the provider map.
    ///
    /// Clones the entire map so the lock is held for the shortest possible
    /// time.  Prefer [`with_provider`] for single-provider lookups.
    pub fn with_providers<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<String, ProviderConfig>) -> R,
    {
        match self.providers_rw.read() {
            Ok(guard) => f(&guard),
            Err(_) => f(&HashMap::new()),
        }
    }

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
                    providers_rw: RwLock::new(providers),
                    db,
                    jwt_keys,
                    http_client: build_http_client(),
                    proxy_ctx: RwLock::new(HashMap::new()),
                    channel_cache: ChannelCache::from_env(),
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
            providers_rw: RwLock::new(providers),
            db,
            jwt_keys,
            http_client: build_http_client(),
            proxy_ctx: RwLock::new(HashMap::new()),
            channel_cache: ChannelCache::from_env(),
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

    // ── CacheScope ────────────────────────────────────────────────────────

    #[test]
    fn cache_scope_from_auth_scope_global() {
        let scope = CacheScope::from_auth_scope(&otvi_core::config::AuthScope::Global, "user-1");
        assert_eq!(scope, CacheScope::Global);
    }

    #[test]
    fn cache_scope_from_auth_scope_per_user() {
        let scope = CacheScope::from_auth_scope(&otvi_core::config::AuthScope::PerUser, "user-42");
        assert_eq!(scope, CacheScope::PerUser("user-42".into()));
    }

    #[test]
    fn cache_scope_global_ignores_user_id() {
        // For global scope the user_id argument is irrelevant — two different
        // user IDs must both produce CacheScope::Global.
        let a = CacheScope::from_auth_scope(&otvi_core::config::AuthScope::Global, "alice");
        let b = CacheScope::from_auth_scope(&otvi_core::config::AuthScope::Global, "bob");
        assert_eq!(a, b);
        assert_eq!(a, CacheScope::Global);
    }

    #[test]
    fn cache_scope_per_user_different_ids_are_not_equal() {
        let a = CacheScope::from_auth_scope(&otvi_core::config::AuthScope::PerUser, "alice");
        let b = CacheScope::from_auth_scope(&otvi_core::config::AuthScope::PerUser, "bob");
        assert_ne!(a, b);
    }

    // ── ChannelCacheKey ───────────────────────────────────────────────────

    #[test]
    fn cache_key_global_equals_global_for_same_provider() {
        let a = ChannelCacheKey::global("prov");
        let b = ChannelCacheKey::global("prov");
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_global_differs_from_per_user() {
        let global = ChannelCacheKey::global("prov");
        let per_user = ChannelCacheKey::per_user("prov", "user-1");
        assert_ne!(global, per_user);
    }

    #[test]
    fn cache_key_per_user_same_user_same_provider_equal() {
        let a = ChannelCacheKey::per_user("prov", "user-1");
        let b = ChannelCacheKey::per_user("prov", "user-1");
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_per_user_different_users_not_equal() {
        let a = ChannelCacheKey::per_user("prov", "user-a");
        let b = ChannelCacheKey::per_user("prov", "user-b");
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_different_providers_not_equal() {
        let a = ChannelCacheKey::global("prov-a");
        let b = ChannelCacheKey::global("prov-b");
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_from_auth_scope_global() {
        let key = ChannelCacheKey::from_auth_scope(
            "prov",
            &otvi_core::config::AuthScope::Global,
            "any-user",
        );
        assert_eq!(key, ChannelCacheKey::global("prov"));
    }

    #[test]
    fn cache_key_from_auth_scope_per_user() {
        let key = ChannelCacheKey::from_auth_scope(
            "prov",
            &otvi_core::config::AuthScope::PerUser,
            "user-99",
        );
        assert_eq!(key, ChannelCacheKey::per_user("prov", "user-99"));
    }

    // ── ChannelCache ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn channel_cache_miss_returns_none() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::per_user("provider1", "user1");
        assert!(cache.channels.get(&key).await.is_none());
        assert!(cache.categories.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn channel_cache_hit_after_insert() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::per_user("provider1", "user1");
        let payload = CachedChannels {
            channels: vec![otvi_core::types::Channel {
                id: "ch1".into(),
                name: "Channel 1".into(),
                logo: None,
                category: None,
                number: None,
                description: None,
            }],
        };
        cache.channels.insert(key.clone(), payload).await;
        let hit = cache.channels.get(&key).await;
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().channels.len(), 1);
    }

    #[tokio::test]
    async fn category_cache_hit_after_insert() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::global("provider1");
        let payload = CachedCategories {
            categories: vec![
                otvi_core::types::Category {
                    id: "1".into(),
                    name: "News".into(),
                },
                otvi_core::types::Category {
                    id: "2".into(),
                    name: "Sports".into(),
                },
            ],
        };
        cache.categories.insert(key.clone(), payload).await;
        let hit = cache.categories.get(&key).await;
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().categories.len(), 2);
    }

    #[tokio::test]
    async fn invalidate_clears_both_channel_and_category_caches() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key = ChannelCacheKey::per_user("prov", "uid1");

        cache
            .channels
            .insert(key.clone(), CachedChannels { channels: vec![] })
            .await;
        cache
            .categories
            .insert(key.clone(), CachedCategories { categories: vec![] })
            .await;

        assert!(cache.channels.get(&key).await.is_some());
        assert!(cache.categories.get(&key).await.is_some());

        cache.invalidate(&key).await;

        assert!(cache.channels.get(&key).await.is_none());
        assert!(cache.categories.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn invalidate_does_not_affect_other_keys() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key_a = ChannelCacheKey::per_user("prov", "uid-a");
        let key_b = ChannelCacheKey::per_user("prov", "uid-b");

        cache
            .channels
            .insert(key_a.clone(), CachedChannels { channels: vec![] })
            .await;
        cache
            .channels
            .insert(key_b.clone(), CachedChannels { channels: vec![] })
            .await;

        cache.invalidate(&key_a).await;

        assert!(
            cache.channels.get(&key_a).await.is_none(),
            "uid-a should be evicted"
        );
        assert!(
            cache.channels.get(&key_b).await.is_some(),
            "uid-b should still be cached"
        );
    }

    #[tokio::test]
    async fn channel_cache_global_key_not_affected_by_per_user_invalidation() {
        // Invalidating a per-user key must never evict the shared global entry.
        let cache = ChannelCache::new(Duration::from_secs(60));
        let global_key = ChannelCacheKey::global("prov");
        let per_user_key = ChannelCacheKey::per_user("prov", "uid-a");

        cache
            .channels
            .insert(global_key.clone(), CachedChannels { channels: vec![] })
            .await;
        cache
            .channels
            .insert(per_user_key.clone(), CachedChannels { channels: vec![] })
            .await;

        cache.invalidate(&per_user_key).await;

        assert!(
            cache.channels.get(&global_key).await.is_some(),
            "global key must survive"
        );
        assert!(cache.channels.get(&per_user_key).await.is_none());
    }

    #[tokio::test]
    async fn channel_cache_different_providers_are_independent() {
        let cache = ChannelCache::new(Duration::from_secs(60));
        let key_p1 = ChannelCacheKey::global("provider-1");
        let key_p2 = ChannelCacheKey::global("provider-2");

        cache
            .channels
            .insert(key_p1.clone(), CachedChannels { channels: vec![] })
            .await;

        assert!(cache.channels.get(&key_p1).await.is_some());
        assert!(cache.channels.get(&key_p2).await.is_none());
    }

    #[tokio::test]
    async fn channel_cache_evicts_after_ttl() {
        // Use a 1ms TTL so we can observe expiry without sleeping long.
        let cache = ChannelCache::new(Duration::from_millis(1));
        let key = ChannelCacheKey::global("prov");

        cache
            .channels
            .insert(key.clone(), CachedChannels { channels: vec![] })
            .await;

        // Sleep well beyond the TTL.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // moka evicts lazily on access; the entry should be gone.
        assert!(cache.channels.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn default_ttl_is_one_day() {
        assert_eq!(DEFAULT_CHANNEL_CACHE_TTL_SECS, 86_400);
    }

    // ── AppState construction tests ───────────────────────────────────────

    #[tokio::test]
    async fn load_providers_nonexistent_dir_returns_empty() {
        let (db, _dir) = test_db().await;
        let tmp = tempfile::tempdir().expect("create temp dir");
        let nonexistent = tmp.path().join("does-not-exist");
        let state = AppState::load_providers(nonexistent.to_str().unwrap(), db, test_keys())
            .expect("should succeed with warning");
        assert!(state.providers_rw.read().unwrap().is_empty());
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
        assert_eq!(state.providers_rw.read().unwrap().len(), 1);
        assert!(state.providers_rw.read().unwrap().contains_key("test_tv"));
    }

    #[tokio::test]
    async fn with_provider_returns_some_for_existing() {
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

        let name = state.with_provider("test_tv", |p| p.provider.name.clone());
        assert_eq!(name, Some("TestTV".to_string()));
    }

    #[tokio::test]
    async fn with_provider_returns_none_for_missing() {
        let (db, _db_dir) = test_db().await;
        let dir = tempfile::tempdir().expect("create temp dir");
        let state =
            AppState::load_providers(dir.path().to_str().unwrap(), db, test_keys()).unwrap();

        let result = state.with_provider("nonexistent", |p| p.provider.name.clone());
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn load_providers_skips_non_yaml_files() {
        let (db, _db_dir) = test_db().await;
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("readme.txt"), "not yaml").unwrap();
        std::fs::write(dir.path().join("data.json"), "{}").unwrap();
        let state =
            AppState::load_providers(dir.path().to_str().unwrap(), db, test_keys()).unwrap();
        assert!(state.providers_rw.read().unwrap().is_empty());
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
        assert_eq!(state.providers_rw.read().unwrap().len(), 1);
        assert!(state.providers_rw.read().unwrap().contains_key("yml_tv"));
    }
}
