## 1. Extend Config Schema and Remove Validation Guard

- [x] 1.1 In `crates/otvi-core/src/config.rs`, add `on_status_codes: Vec<u16>` field to `RefreshConfig` with `#[serde(default = "default_refresh_status_codes")]` defaulting to `[401]`
- [x] 1.2 In `crates/otvi-server/src/state.rs`, remove the `if config.auth.refresh.is_some() { bail!(...) }` validation guard in `validate_provider_config`
- [x] 1.3 Update the test `load_providers_rejects_unsupported_refresh_config` to expect success instead of failure (rename to `load_providers_accepts_refresh_config`)
- [x] 1.4 Run `cargo check` across the workspace to verify schema changes compile

## 2. Add Refresh Coordination State

- [x] 2.1 In `crates/otvi-server/src/state.rs`, add a `refresh_locks: Mutex<HashMap<(String, String), Arc<tokio::sync::Mutex<()>>>>` field to `AppState` for per-session refresh coordination
- [x] 2.2 Add a helper method `AppState::refresh_lock(&self, provider_id: &str, user_id: &str) -> Arc<tokio::sync::Mutex<()>>` that returns or creates the mutex for a given session
- [x] 2.3 Run `cargo check` to verify the new state field compiles

## 3. Implement Refresh Execution Function

- [x] 3.1 In `crates/otvi-server/src/api/auth.rs`, add `pub async fn execute_refresh(state: &AppState, provider_id: &str, user_id: &str) -> Result<(), AppError>` that loads RefreshConfig from the provider YAML, builds a TemplateContext from stored values, executes the refresh request via `provider_client::execute_request`, extracts new values, merges into existing stored values, and persists via `db::upsert_provider_session`
- [x] 3.2 After successful refresh, invalidate the channel/category cache for the affected session via `state.channel_cache.invalidate`
- [x] 3.3 After successful refresh, scan `state.proxy_ctx` and remove entries whose upstream_url host matches the provider's base URL domain (best-effort stale ProxyContext cleanup — relies on TTL due to moka limitation)
- [x] 3.4 Add unit tests for `execute_refresh`: successful refresh updates stored values, failed refresh returns error without modifying stored values (7 tests total)

## 4. Implement Refresh-and-Retry Wrapper

- [x] 4.1 In `crates/otvi-server/src/api/auth.rs`, add `with_refresh_retry` that takes a closure executing an upstream call via `execute_request_raw`, checks the response status against `on_status_codes`, acquires the refresh lock, calls `execute_refresh`, re-builds the template context, and retries the closure once
- [x] 4.2 Ensure the retry does NOT trigger another refresh if it also returns a refresh-triggering status (prevent infinite loop)
- [x] 4.3 Ensure waiters on the refresh lock re-read tokens from DB and retry without re-executing refresh

## 5. Integrate Refresh into Channel and Stream Handlers

- [x] 5.1 In `crates/otvi-server/src/api/channels.rs`, wrap the channel list upstream fetch (`load_all_channels`) with `with_refresh_retry` so that 401/403 responses trigger refresh and retry
- [x] 5.2 In `crates/otvi-server/src/api/channels.rs`, wrap the stream URL upstream fetch (in the stream handler) and categories fetch with `with_refresh_retry`
- [x] 5.3 Verify that the retried request uses freshly-loaded stored values (`with_refresh_retry` re-calls `build_provider_context` after refresh)

## 6. Update JioTV Provider YAML

- [x] 6.1 In `providers/jiotv-mobile.yaml`, add `on_status_codes: [401, 403]` to the existing `auth.refresh` block
- [x] 6.2 Verify the provider loads successfully via `cargo check`

## 7. Verification and Cleanup

- [x] 7.1 Run `cargo fmt` across the workspace
- [x] 7.2 Run `cargo clippy` across the workspace and fix any warnings
- [x] 7.3 Run `cargo test` and ensure all existing tests pass plus new refresh tests pass (151 unit + 57 integration = 208 tests pass)
- [x] 7.4 Manually verified via `cargo check` and test suite that jiotv-mobile.yaml loads without errors
