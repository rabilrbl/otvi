## Context

OTVI is a data-driven TV streaming platform where each provider is described by a YAML configuration file. Provider authentication is a multi-step flow (e.g., send OTP, verify OTP) that stores extracted tokens (`access_token`, `refresh_token`, `sso_token`, etc.) in a `provider_sessions` database table. All downstream API calls (channel listing, stream resolution, proxy) resolve `{{stored.*}}` template variables from these persisted tokens.

The `RefreshConfig` schema already exists in `otvi-core` and the JioTV YAML already declares a refresh block. However, `validate_provider_config` in `state.rs` explicitly rejects any provider with `auth.refresh`, preventing JioTV from loading. No runtime refresh execution logic exists.

The existing login flow in `auth.rs` demonstrates the full pattern: build a `TemplateContext` from stored values, call `provider_client::execute_request`, extract values via JSONPath, merge into stored values, persist via `db::upsert_provider_session`. A refresh flow is structurally identical but triggered automatically (no user input, no multi-step prompts).

## Goals / Non-Goals

**Goals:**
- Enable providers with `auth.refresh` to load and run (unblock JioTV-mobile)
- Automatically refresh tokens when upstream API calls fail with configurable status codes (default: 401)
- Retry the failed upstream request exactly once after a successful refresh
- Prevent concurrent refresh storms (multiple in-flight requests all triggering independent refreshes)
- Persist refreshed tokens to the database so they survive server restarts
- Respect auth scope isolation (per-user sessions refresh independently; global sessions refresh once for all users)

**Non-Goals:**
- Proactive/scheduled token refresh (no background timer; purely reactive to upstream errors)
- Token TTL tracking or `expires_at` metadata in the config schema
- Frontend-initiated manual refresh endpoint (users re-login if refresh fails)
- Refresh token rotation (JioTV's refresh token is long-lived; only `access_token` rotates)
- Modifying the `RefreshConfig` struct's core shape (it already has `request` + `on_success`)

## Decisions

### 1. Reactive refresh triggered by upstream HTTP status codes

**Decision:** Refresh is triggered when an upstream provider API call returns a status code in `RefreshConfig.on_status_codes` (default `[401]`). The handler retries the original request once with updated tokens.

**Alternatives considered:**
- *Proactive/periodic refresh*: Requires knowing token TTL, adds a background task with lifecycle complexity, and the YAML has no TTL field. Deferred as a future enhancement.
- *Middleware-layer retry*: Axum middleware operates on our responses, not upstream calls. Would require significant restructuring.

**Rationale:** Reactive refresh is the simplest approach that solves the problem. It reuses existing primitives (`execute_request`, `extract_json_path`, `upsert_provider_session`) and matches how OAuth2 clients typically handle token expiry.

### 2. Per-session refresh coordination via `tokio::sync::Mutex`

**Decision:** Add a `DashMap<(provider_id, user_id), Arc<tokio::sync::Mutex<()>>>` to `AppState`. When a refresh is needed, the first request acquires the mutex, performs the refresh, and updates stored tokens. Concurrent requests wait on the mutex, then re-read fresh tokens from the DB and retry without re-triggering refresh.

**Alternatives considered:**
- *No coordination*: Risk of N concurrent 401s triggering N refresh calls, wasting API quota and potentially causing rate-limiting. JioTV's refresh endpoint is sensitive to repeated calls.
- *`tokio::sync::Notify` + flag*: More complex; a mutex with a short critical section is sufficient since refresh is a single HTTP call (~100-500ms).

**Rationale:** DashMap + Mutex gives fine-grained per-session locking without blocking unrelated sessions. The mutex is held only during the refresh HTTP call and DB persist — waiters then proceed with fresh tokens.

### 3. Refresh execution as a reusable function in `auth.rs`

**Decision:** Add `pub async fn execute_refresh(state, provider_id, user_id) -> Result<(), AppError>` in `auth.rs` that:
1. Loads `RefreshConfig` from the provider YAML
2. Loads stored values from DB
3. Builds a `TemplateContext` (same as `build_provider_context`)
4. Calls `provider_client::execute_request` with the refresh `RequestSpec`
5. Extracts new values via `on_success.extract`
6. Merges new values into existing stored values (overwrites only extracted keys)
7. Persists via `db::upsert_provider_session`
8. Invalidates the channel/category cache for this session

**Rationale:** This function mirrors the extraction pattern already in `login()` (lines 183-206 of auth.rs) but without user inputs or multi-step logic. Placing it in `auth.rs` keeps all provider auth logic co-located.

### 4. Wrap upstream calls with a refresh-and-retry helper

**Decision:** Add a helper function `with_refresh_retry` (or integrate into the stream/channel handlers) that:
1. Executes the upstream call
2. If the response status is in `on_status_codes`, acquire the refresh mutex
3. Execute `execute_refresh`
4. Re-build the template context with fresh tokens
5. Retry the upstream call once
6. Return whatever the retry produces (success or failure)

This applies to:
- Channel list fetch in `channels.rs` (`load_all_channels` or its inner HTTP call)
- Stream URL fetch in `channels.rs` (the playback endpoint)

It does NOT apply to:
- Proxy requests (`proxy.rs`) — these use pre-resolved `ProxyContext` headers, not template-resolved calls. Staleness is handled by decision 5.

### 5. ProxyContext staleness after refresh

**Decision:** After a successful refresh, invalidate all `ProxyContext` entries for the affected session by scanning the `proxy_ctx` cache and removing entries whose `upstream_url` domain matches the provider's base URL. This is a best-effort approach — active DASH/HLS segment downloads may fail, but the player will request a new stream URL which creates a fresh ProxyContext with updated tokens.

**Alternatives considered:**
- *Make ProxyContext store template references instead of resolved values*: Major refactor of the proxy system, breaks the current security model where resolved values are server-side only.
- *Do nothing*: Active streams would fail with 401/403 until the user manually reloads. Acceptable for MVP but poor UX.

**Rationale:** ProxyContext entries have a TTL (moka cache) and are created per-stream-request. Invalidating stale entries forces the player to re-request the stream, which gets a fresh ProxyContext. This is a pragmatic trade-off.

### 6. Extend `RefreshConfig` with `on_status_codes`

**Decision:** Add `on_status_codes: Vec<u16>` (serde default `[401]`) to `RefreshConfig` in `otvi-core/config.rs`. Providers can override this (e.g., `on_status_codes: [401, 403]`).

**Rationale:** JioTV may return 401 or 403 when tokens expire. Other providers may use different codes. Making it configurable keeps the system generic.

## Risks / Trade-offs

- **[Refresh loop]** If the refresh endpoint itself returns a status in `on_status_codes`, we could loop. → Mitigation: Never retry the refresh call itself. If refresh fails, return the original error and let the user re-login.
- **[Stale ProxyContext]** Active streams may briefly break after a refresh until the player re-requests. → Mitigation: Players typically retry on segment failures. Acceptable trade-off vs. refactoring ProxyContext to use lazy resolution.
- **[Refresh token expiry]** If the refresh token itself has expired, the refresh call will fail silently. → Mitigation: Delete the session on refresh failure (configurable; for now, log a warning and return the original error). The user must re-login.
- **[Concurrent refresh under load]** The mutex serializes refresh but waiters still all retry their original requests simultaneously after refresh completes. → Mitigation: This is fine — the upstream API should accept valid tokens from multiple concurrent requests. The serialization prevents N refresh calls, not N retries.
