# Otvi Improvement Spec

## Objective

Improve the otvi server across security, performance, architecture, and observability. The project targets single-user self-hosted deployments. Breaking changes are acceptable with documentation. Implementation is phased to keep each increment reviewable.

**Target users:** Single admin running otvi on a home server for personal TV streaming. Simplicity matters more than enterprise hardening, but basic security and reliability must be solid.

**Scope:** Rust backend (`otvi-server`, `otvi-core`). The Leptos frontend is out of scope.

---

## Phase 1 — Security & Performance

Critical correctness and performance fixes. Ships first because they affect data integrity and runtime behavior.

### 1.1 Replace SELECT + conditional INSERT with UPSERT in `db.rs`

**File:** `crates/otvi-server/src/db.rs:395-441`

The `upsert_provider_session` function runs a SELECT, then either UPDATEs or INSERTs inside a transaction. Both SQLite and Postgres support `INSERT … ON CONFLICT DO UPDATE`. Replace the two-query transaction with a single UPSERT.

**Acceptance criteria:**
- `upsert_provider_session` uses a single `INSERT … ON CONFLICT DO UPDATE` query
- Existing tests pass
- New unit test verifies upsert behavior (insert-then-update same key)

### 1.2 Add connect and request timeouts to the HTTP client

**File:** `crates/otvi-server/src/state.rs:377-381`

`build_http_client()` creates a `reqwest::Client` with zero timeout configuration. A misbehaving upstream can hang connections indefinitely, consuming pool connections and proxy contexts.

**Acceptance criteria:**
- HTTP client has a connect timeout (default 10s, env `HTTP_CONNECT_TIMEOUT_SECS`)
- HTTP client has a request timeout (default 60s, env `HTTP_REQUEST_TIMEOUT_SECS`)
- Both timeouts are logged at startup
- Existing tests pass

### 1.3 Add acquire_timeout to the database pool

**File:** `crates/otvi-server/src/db.rs:74`

`AnyPoolOptions::new().max_connections(10)` sets a max but no acquire timeout. Under load, pool acquisition can hang indefinitely.

**Acceptance criteria:**
- `acquire_timeout` set on pool options (default 30s, env `DB_ACQUIRE_TIMEOUT_SECS`)
- Logged at startup
- Existing tests pass

### 1.4 Evict stale refresh locks

**File:** `crates/otvi-server/src/state.rs:387`

`RefreshLocks` is `Mutex<HashMap<(String, String), Arc<Mutex<()>>>>` with no eviction. Entries accumulate forever for users who log in once and never return.

**Acceptance criteria:**
- Periodic task (every 5 minutes) removes entries whose inner `Mutex` has zero strong references (i.e., no one is currently holding the lock)
- Eviction count logged at `debug!` level
- Existing tests pass

### 1.5 Strengthen password validation

**File:** `crates/otvi-server/src/account.rs:16-30`

Current validation: 8+ chars, 1 uppercase, 1 digit. Add:
- Minimum 1 special character (non-alphanumeric)
- Reject passwords > 128 chars (already present, keep it)

**Acceptance criteria:**
- `validate_password` rejects passwords without a special character
- Existing tests updated
- New test: password with special char passes
- New test: password without special char fails

### 1.6 Atomic RwLock swap during provider hot-reload

**File:** `crates/otvi-server/src/watcher.rs:106-143`

`reload_providers` builds the new map, then acquires a write lock and swaps. This is correct but the write lock is held while the old map is dropped. Replace with `std::sync::RwLock` → swap pattern: build new map outside the lock, then only hold the lock long enough to `std::mem::swap`.

**Acceptance criteria:**
- Write lock held for only the `swap` operation, not the YAML parsing
- Existing tests pass
- Hot-reload still works (integration test or manual verification)

---

## Phase 2 — Architecture Refactor

Structural improvements that make the codebase easier to work on. Pure refactoring with no behavior changes.

### 2.1 Extract `proxy/` module from `proxy.rs`

**File:** `crates/otvi-server/src/api/proxy.rs` (~1200 lines)

Split into:
- `proxy/mod.rs` — re-exports, `ProxyQuery`, `proxy_stream`, `proxy_drm`
- `proxy/validate.rs` — `validate_proxy_target`, SSRF protection, private-IP checks
- `proxy/rewrite.rs` — HLS/DASH manifest rewriting, cookie injection, key-URI handling
- `proxy/drm.rs` — DRM license proxy, prefetch, static cookie resolution
- `proxy/context.rs` — `ProxyContext` construction and cookie resolution helpers

**Acceptance criteria:**
- No behavior change — all existing proxy tests pass unchanged
- `pub` visibility only where needed by `mod.rs` re-exports
- Each sub-module is independently understandable

### 2.2 Derive `thiserror` for `AppError`

**File:** `crates/otvi-server/src/error.rs`

Replace manual `From<anyhow::Error>` with `thiserror` derives. Preserve the `Internal` variant's error-message-scrubbing (don't leak internal details in responses).

**Acceptance criteria:**
- `AppError` derives `thiserror::Error`
- `Internal` variant preserves the current behavior: log the full error, return generic `"Internal server error"` to client
- `From<anyhow::Error>` still converts to `Internal`
- All existing error-response tests pass

### 2.3 Derive `FromRow` for `UserRow` and `ProviderSessionRow`

**File:** `crates/otvi-server/src/db.rs`

Every query manually constructs `UserRow { id: r.get("id"), ... }`. sqlx `FromRow` derive generates this automatically and catches schema mismatches at compile time.

**Acceptance criteria:**
- `UserRow` and `ProviderSessionRow` derive `FromRow`
- All manual row construction replaced
- Existing tests pass
- `must_change_password` field maps `i64` → `bool` correctly (custom `FromRow` impl or sqlx `#[sqlx(rename = "must_change_password")]` attribute)

### 2.4 Document the vendor sqlx patch

**File:** `Cargo.toml`, new file `vendor/sqlx/README.otvi.md`

The workspace patches sqlx to a local vendored copy. Add a README explaining:
- Why the patch exists (what change it carries)
- The upstream PR or issue it tracks
- How to re-vendor when upgrading

**Acceptance criteria:**
- README exists in `vendor/sqlx/`
- Contains the reason, upstream tracking info, and re-vendoring instructions

---

## Phase 3 — Observability & Features

Prometheus metrics, request tracing, and graceful shutdown.

### 3.1 Add Prometheus metrics endpoint

Expose a `GET /metrics` endpoint using `prometheus` or `metrics` crate. Instrument:

**Metrics to expose:**

| Name | Type | Labels | Description |
|------|------|--------|-------------|
| `otvi_http_requests_total` | Counter | method, path, status | Total HTTP requests |
| `otvi_http_request_duration_seconds` | Histogram | method, path | Request latency |
| `otvi_proxy_contexts_active` | Gauge | — | Current proxy context cache size |
| `otvi_channel_cache_entries` | Gauge | provider, scope | Cached channel list entries |
| `otvi_channel_cache_hits_total` | Counter | provider, scope | Cache hits |
| `otvi_channel_cache_misses_total` | Counter | provider, scope | Cache misses |
| `otvi_provider_upstream_requests_total` | Counter | provider, method, status | Upstream API calls |
| `otvi_db_pool_connections` | Gauge | state | DB pool connections (idle/active) |
| `otvi_refresh_locks_active` | Gauge | — | Current refresh lock entries |

**Acceptance criteria:**
- `GET /metrics` returns Prometheus-format text
- All counters and gauges above are registered and updated
- Metrics endpoint is unauthenticated (intended for internal scrape)
- Existing tests pass

### 3.2 Add request ID middleware

Add `tower-http` request ID + tracing layers so every request gets a unique ID propagated through logs.

**Acceptance criteria:**
- Every request log line includes `request_id`
- `x-request-id` response header set
- Request ID propagates through `tracing::Span`
- Configurable via `REQUEST_ID_HEADER` env var (default: `x-request-id`)

### 3.3 Graceful shutdown with connection draining

**File:** `crates/otvi-server/src/main.rs`

`shutdown_drain_secs` reads a timeout but the current implementation doesn't drain in-flight requests. Add proper Axum graceful shutdown that:
- Stops accepting new connections
- Waits for in-flight requests to complete (up to drain timeout)
- Force-closes after timeout

**Acceptance criteria:**
- SIGTERM → server stops accepting new connections
- In-flight requests complete within the drain timeout
- After timeout, all connections are forcefully closed
- Logged: "Shutting down" + "Drain complete" or "Drain timeout, forcing shutdown"

---

## Project Structure

No new crates. All changes within existing `crates/otvi-server/src/`.

```
crates/otvi-server/src/
├── api/
│   ├── proxy/           # NEW — split from proxy.rs
│   │   ├── mod.rs
│   │   ├── validate.rs
│   │   ├── rewrite.rs
│   │   ├── drm.rs
│   │   └── context.rs
│   ├── admin.rs
│   ├── auth.rs
│   ├── channels.rs
│   ├── providers.rs
│   ├── provider_access.rs
│   └── user_auth.rs
├── metrics.rs           # NEW — Prometheus registry + handlers
├── account.rs
├── auth_middleware.rs
├── channel_catalog.rs
├── db.rs
├── embedded_frontend.rs
├── error.rs
├── lib.rs
├── main.rs
├── playback.rs
├── provider_client.rs
├── state.rs
└── watcher.rs
```

---

## Code Style

- Follow existing conventions: `rustfmt` + `clippy` with project defaults
- Error messages: lowercase, no trailing period
- Public functions get doc comments; private ones don't unless non-obvious
- `tracing` macros for all logging (info, warn, error, debug)
- Env var defaults are documented in code comments and logged at startup

## Testing Strategy

- **Unit tests:** Every new function gets unit tests in the same file (`#[cfg(test)] mod tests`).
- **Integration tests:** API route tests in `crates/otvi-server/tests/integration.rs` using `axum::test` helpers.
- **Wiremock:** Upstream provider calls mocked with `wiremock` where applicable (already a dependency).
- **Test database:** SQLite in-memory (`:memory:`) for all DB tests, as already established.

Phase gates: each phase must have all tests passing before the next phase starts.

## Boundaries

### Always do
- Preserve existing API contracts (endpoint paths, request/response shapes) unless a phase explicitly changes them
- Log new env vars at startup
- Use existing `parse_env_or_warn` / `from_env` patterns for configuration
- Keep `pub` visibility minimal — only expose what other modules need

### Ask first about
- New crate dependencies (check if the functionality exists in already-imported crates)
- Changes to the YAML provider config schema (breaking change for existing configs)
- Removing or renaming existing API endpoints

### Never do
- Change the SQLite migration files (append-only)
- Add authentication to the `/metrics` endpoint (internal-only by design)
- Modify the Leptos frontend (out of scope)
- Vendor new dependencies without documenting why