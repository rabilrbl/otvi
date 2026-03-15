# OTVI – Open TV Interface

A generic, YAML-driven television interface that lets any TV provider expose
login, logout, channel browsing, and live playback (HLS / DASH + DRM) through
simple configuration files. No custom code is needed per provider — just
describe the API in a YAML file.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    YAML Provider Configs                │
│  providers/acme.yaml   providers/streammax.yaml  …      │
└────────────────────────┬────────────────────────────────┘
                         │ loaded at startup + hot-reloaded
                         ▼
┌──────────────── otvi-server (Axum) ─────────────────────┐
│  REST API   ─── provider_client ──▶  Provider HTTP APIs │
│  /api/…           (reqwest)                             │
│                                                         │
│  /healthz  /readyz ── liveness & readiness probes       │
│  /api/schema/provider ── live JSON Schema for YAML      │
│                                                         │
│  Static files ──▶ serves compiled WASM frontend         │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │ fetch / JSON
┌──────────────── otvi-web (Leptos WASM) ─────────────────┐
│  Home   Login   Channels (search + filter)   Player     │
└─────────────────────────────────────────────────────────┘
```

| Crate           | Purpose                                                                              |
| --------------- | ------------------------------------------------------------------------------------ |
| **otvi-core**   | Shared types: YAML config schema, API request/response types, template engine        |
| **otvi-server** | Axum REST API, hot-reloads provider YAMLs, proxies API calls, serves frontend        |
| **otvi-web**    | Leptos CSR frontend compiled to WASM (trunk)                                         |

## Quick Start

### Prerequisites

- Rust stable (1.83+)
- `trunk` CLI for the WASM frontend: `cargo install trunk`
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- `wasm-pack` for frontend UI tests: `cargo binstall wasm-pack` (or `cargo install wasm-pack`)
- Firefox or Chrome/Chromium for browser UI tests

### Build & Run (development)

```bash
# 1. Build the frontend
cd web && trunk build && cd ..

# 2. Run the server
cargo run -p otvi-server
# → http://localhost:3000
```

### Frontend UI Tests

```bash
cd web
wasm-pack test --headless --firefox --features ui-test --lib
# or
bun run ui:test
```

The browser runner reads `web/webdriver.json` for Chrome headless capabilities when you use the Chrome backend.

### Docker

```bash
# Production
docker compose up --build

# Development (with hot-reload and verbose logging)
docker compose -f docker-compose.dev.yml up --build
# → http://localhost:3000
```

## Environment Variables

| Variable                  | Default              | Description                                                                                      |
| ------------------------- | -------------------- | ------------------------------------------------------------------------------------------------ |
| `DATABASE_URL`            | `sqlite://data.db`   | Database connection string. Supports `sqlite://`, `postgres://`, `mysql://`                      |
| `JWT_SECRET`              | *(random)*           | Secret for signing JWTs. **Always set a persistent value in production.**                        |
| `PORT`                    | `3000`               | Port the server listens on                                                                       |
| `PROVIDERS_DIR`           | `providers`          | Directory scanned for `*.yaml` / `*.yml` provider configs (hot-reloaded on change)              |
| `STATIC_DIR`              | `dist`               | Directory served as the static frontend build                                                    |
| `RUST_LOG`                | `otvi_server=info`   | Log filter ([`tracing` format](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/))   |
| `LOG_FORMAT`              | `text`               | Set to `json` for structured log output (Loki, Datadog, CloudWatch, etc.)                        |
| `CORS_ORIGINS`            | *(permissive)*       | Comma-separated allowed origins, e.g. `https://tv.example.com`. Unset = allow all (dev only)     |
| `CHANNEL_CACHE_TTL_SECS`  | `86400` (24 h)       | TTL for the server-side channel and category list cache. Entries are also invalidated immediately on provider login / logout. |

## Creating a Provider Config

Each provider is a single YAML file placed in the `providers/` directory.
The server **hot-reloads** changes automatically — no restart required.

### Step-by-step

1. **Capture traffic** from the provider's mobile / Android TV app using
   mitmproxy, Charles Proxy, or HTTP Toolkit.
2. **Copy** `providers/example.yaml` and rename it.
3. **Fill in** the API endpoints, headers, and body templates from your
   traffic captures.
4. **Save** — the running server picks up the change within ~300 ms.

### YAML auto-complete in VS Code

Point the [YAML extension](https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml)
at the live schema endpoint and get inline validation and auto-complete while
editing provider files:

```jsonc
// .vscode/settings.json
{
  "yaml.schemas": {
    "http://localhost:3000/api/schema/provider": "providers/*.yaml"
  }
}
```

### Template Variables

| Variable        | Description                                                               |
| --------------- | ------------------------------------------------------------------------- |
| `{{input.X}}`   | Value entered by the user in a form field                                 |
| `{{stored.X}}`  | Value extracted from a previous API response and persisted in the session |
| `{{extract.X}}` | Value extracted in the previous auth step                                 |
| `{{uuid}}`      | Auto-generated UUID v4 (useful for device IDs)                            |
| `{{utcnow}}`    | Current UTC timestamp (`YYYYMMDDTHHmmSS`)                                 |
| `{{utcdate}}`   | Current UTC date (`YYYYMMDD`)                                             |

The template engine warns in the server log whenever a placeholder cannot be
resolved, making misconfigured YAMLs easy to spot.

### Response Extraction

Values are extracted from JSON responses using full **JSONPath** expressions
(powered by [`jsonpath-rust`](https://github.com/besok/jsonpath-rust)):

```yaml
on_success:
  extract:
    access_token: "$.data.access_token"
    user_name:    "$.data.user.display_name"
    # Filter expression
    first_active: "$.items[?(@.active == true)].id"
    # Recursive descent
    any_token:    "$..token"
```

### Multi-step Auth (e.g. Phone + OTP)

Add a `prompt` section to the `on_success` of an intermediate step — the
frontend will show additional form fields before continuing:

```yaml
steps:
  - name: "Send OTP"
    request: …
    on_success:
      extract:
        request_id: "$.data.request_id"
      prompt:
        - key: "otp"
          label: "Enter Verification Code"
          type: "text"
          required: true
  - name: "Verify OTP"
    request: …
```

## Password Policy

All passwords (self-registration, change-password, admin-created accounts) must satisfy:

- Minimum **8 characters**
- At least one **uppercase** ASCII letter
- At least one **digit**

Accounts created by an admin are flagged `must_change_password = true`. The server
**rejects all API calls** from such accounts (returning `403 Forbidden`) until the
user sets a personal password via `POST /api/auth/change-password`.

## REST API Reference

### Infrastructure

| Method | Path                    | Auth  | Description                              |
| ------ | ----------------------- | ----- | ---------------------------------------- |
| GET    | `/healthz`              | None  | Liveness probe — instant `200 OK`        |
| GET    | `/readyz`               | None  | Readiness probe — checks DB connectivity |
| GET    | `/api/schema/provider`  | None  | JSON Schema for provider YAML files      |

### User Authentication

| Method | Path                          | Auth | Description                        |
| ------ | ----------------------------- | ---- | ---------------------------------- |
| POST   | `/api/auth/register`          | —    | Create account (first = admin)     |
| POST   | `/api/auth/login`             | —    | Exchange credentials for JWT       |
| GET    | `/api/auth/me`                | JWT  | Current user info                  |
| POST   | `/api/auth/change-password`   | JWT  | Change password; clears force-flag |
| POST   | `/api/auth/logout`            | JWT  | No-op (client drops token)         |

### Providers

| Method | Path                                      | Auth  | Description                                  |
| ------ | ----------------------------------------- | ----- | -------------------------------------------- |
| GET    | `/api/providers`                          | JWT   | List accessible providers                    |
| GET    | `/api/providers/:id`                      | JWT   | Provider details + auth flows                |
| POST   | `/api/providers/:id/auth/login`           | JWT   | Login (handles multi-step)                   |
| GET    | `/api/providers/:id/auth/check`           | JWT   | Check whether a session is active            |
| POST   | `/api/providers/:id/auth/logout`          | JWT   | Logout from provider                         |
| GET    | `/api/providers/:id/channels`             | JWT   | Browse channels (supports search/pagination) |
| GET    | `/api/providers/:id/channels/categories`  | JWT   | List categories                              |
| GET    | `/api/providers/:id/channels/:cid/stream` | JWT   | Get stream URL + DRM info                    |
| GET    | `/api/proxy`                              | None  | Transparent stream proxy                     |

#### Channel list query parameters

| Parameter  | Description                                        |
| ---------- | -------------------------------------------------- |
| `category` | Filter by category ID                              |
| `search`   | Case-insensitive substring search on channel names |
| `limit`    | Maximum number of channels to return               |
| `offset`   | Zero-based offset for pagination                   |

### Admin

| Method | Path                              | Auth  | Description                         |
| ------ | --------------------------------- | ----- | ----------------------------------- |
| GET    | `/api/admin/users`                | Admin | List all users                      |
| POST   | `/api/admin/users`                | Admin | Create a user                       |
| DELETE | `/api/admin/users/:id`            | Admin | Delete a user                       |
| PUT    | `/api/admin/users/:id/providers`  | Admin | Set a user's provider allow-list    |
| PUT    | `/api/admin/users/:id/password`   | Admin | Reset password (sets force-flag)    |
| GET    | `/api/admin/settings`             | Admin | Get server settings                 |
| PUT    | `/api/admin/settings`             | Admin | Update server settings              |

## Project Structure

```
otvi/
├── Cargo.toml                      # Workspace + [profile.release] (LTO, strip)
├── Dockerfile                      # Multi-stage build (web → server → runtime)
├── docker-compose.yml              # Production compose
├── docker-compose.dev.yml          # Development compose (hot-reload, verbose logging)
├── providers/
│   ├── example.yaml                # Annotated example provider config
│   └── jiotv-mobile.yaml
├── crates/
│   ├── otvi-core/                  # Shared types & template engine
│   │   └── src/
│   │       ├── config.rs           # YAML schema (+ JSON Schema via schemars)
│   │       ├── template.rs         # Template engine + full JSONPath extraction
│   │       └── types.rs            # API request/response types
│   └── otvi-server/                # Axum REST API server
│       ├── src/
│       │   ├── main.rs             # Bootstrap (logging format, hot-reload watcher)
│       │   ├── lib.rs              # Router, CORS, /healthz, /readyz, /api/schema/provider
│       │   ├── state.rs            # AppState (RwLock provider map, channel cache, proxy context cache)
│       │   ├── watcher.rs          # File-system watcher for hot-reloading YAMLs
│       │   ├── db.rs               # SQLx database layer
│       │   ├── auth_middleware.rs  # JWT creation / validation / extractors
│       │   ├── provider_client.rs  # HTTP client with template resolution & warn-on-unresolved
│       │   ├── error.rs            # AppError → HTTP response mapping
│       │   └── api/
│       │       ├── auth.rs         # Provider-level auth (login / logout / check)
│       │       ├── channels.rs     # Channel list (search, pagination), categories, stream
│       │       ├── providers.rs    # Provider listing + must_change_password guard
│       │       ├── proxy.rs        # HLS/DASH stream proxy (M3U8 rewriting, CDN cookies)
│       │       ├── user_auth.rs    # OTVI user auth + password policy + force-change guard
│       │       └── admin.rs        # User & settings management
│       ├── migrations/             # SQLx database migrations
│       └── tests/
│           └── integration.rs      # End-to-end integration tests
└── web/                            # Leptos WASM frontend
    ├── Trunk.toml
    ├── index.html                  # HTML entry + HLS.js / Shaka Player bridge
    ├── input.css / style.css       # Tailwind CSS
    └── src/
        ├── app.rs                  # Root component, routing, auth state machine
        ├── api.rs                  # Backend HTTP client (token storage, typed calls)
        └── pages/
            ├── channels.rs         # Channel grid (search box, URL-persisted category, skeletons)
            └── player.rs           # Video player (resolved channel name/logo, loading skeleton)
```

## License

CC BY-NC-SA 4.0. See [LICENSE](LICENSE) for details.
