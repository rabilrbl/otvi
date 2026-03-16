## Why

The JioTV-mobile provider YAML defines an `auth.refresh` block for automatic token refresh, but the server explicitly rejects any provider with this config at load time (`"unsupported auth.refresh; refresh flows are not implemented"`). This means the JioTV-mobile provider **cannot be loaded at all** — the server startup fails. JioTV access tokens are short-lived; without automatic refresh, sessions expire and users must re-authenticate frequently, breaking playback mid-stream.

## What Changes

- **Remove the validation guard** that rejects providers with `auth.refresh` config
- **Implement reactive token refresh**: when an upstream API call returns a configurable error status (e.g., 401/403), automatically execute the provider's refresh flow, update stored tokens, and retry the original request once
- **Add concurrency control** so that multiple concurrent requests hitting a token expiry don't trigger redundant refresh attempts — a single refresh per session at a time, with waiters reusing the result
- **Invalidate stale ProxyContext entries** after a refresh so that in-flight stream sessions pick up the new tokens
- **Extend `RefreshConfig`** with optional trigger metadata (`on_status_codes`) so providers can declare which upstream error codes should trigger a refresh

## Capabilities

### New Capabilities
- `provider-auth-refresh`: Automatic, reactive token refresh for provider sessions — triggered by upstream error codes, with concurrency control, retry, and session persistence

### Modified Capabilities
- `coherent-channel-and-playback-flow`: Upstream API calls (channel list, stream URL, proxy) now retry once after a successful token refresh instead of returning the upstream error directly
- `secure-provider-boundaries`: Refresh execution must respect the same scope rules (per-user vs global) and never leak tokens across user sessions

## Impact

- **Config**: `RefreshConfig` in `crates/otvi-core/src/config.rs` gains optional `on_status_codes` field
- **State**: `crates/otvi-server/src/state.rs` — remove validation guard, add per-session refresh mutex/coordination
- **Auth**: `crates/otvi-server/src/api/auth.rs` — new `execute_refresh` function reusing existing `execute_request` + `extract`/`persist` primitives
- **Handlers**: `crates/otvi-server/src/api/channels.rs`, `proxy.rs` — wrap upstream calls with refresh-on-error-and-retry logic
- **Provider client**: `crates/otvi-server/src/provider_client.rs` — potential wrapper for refresh-aware request execution
- **YAML**: `providers/jiotv-mobile.yaml` — already has the refresh block; optionally add `on_status_codes: [401, 403]`
- **Tests**: Update `load_providers_rejects_unsupported_refresh_config` test; add new unit tests for refresh flow
