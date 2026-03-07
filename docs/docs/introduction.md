---
sidebar_position: 1
title: Introduction
---

# OTVI – Open TV Interface

OTVI is a generic, **YAML-driven television interface** that lets any TV provider expose login, logout, channel browsing, and live playback (HLS / DASH + DRM) through simple configuration files. No custom code is needed per provider — just describe the API in a YAML file.

## Key Features

- **Zero-code provider integration** — define everything in YAML
- **Hot-reload** — edit a provider YAML and the server picks it up within ~300 ms, no restart needed
- **Multi-step authentication** — phone + OTP, email + password, SSO, and more
- **Template engine** — dynamic request building with `{{input.X}}`, `{{stored.X}}`, `{{uuid}}`, with warnings logged for any unresolved placeholders
- **Full JSONPath extraction** — pull values from API responses using filter expressions, recursive descent, and wildcards (powered by `jsonpath-rust`)
- **HLS & DASH streaming** — with full DRM support (Widevine, PlayReady)
- **Stream proxying** — transparent CDN authentication and CORS handling
- **Multi-user system** — JWT-based auth with admin/user roles
- **Password policy** — min 8 chars, uppercase, digit; enforced consistently across registration, change-password, and admin reset
- **`must_change_password` enforcement** — admin-created accounts are blocked from all API calls until the user sets a personal password
- **Per-user provider access control** — restrict which providers each user can access
- **Channel search & pagination** — server-side text search (`?search=`) and limit/offset pagination on channel lists
- **Database flexibility** — SQLite, PostgreSQL, or MySQL at runtime
- **Health & readiness probes** — `/healthz` (liveness) and `/readyz` (DB check) for orchestrators
- **Provider JSON Schema** — live `GET /api/schema/provider` endpoint for VS Code YAML auto-complete
- **Structured logging** — human-readable text by default; set `LOG_FORMAT=json` for Loki / Datadog
- **Configurable CORS** — permissive in dev, locked to specific origins in production via `CORS_ORIGINS`
- **Modern web UI** — responsive Leptos/WASM frontend with channel search, skeleton loading states, URL-persisted category filter, and proper channel names in the player
- **Docker ready** — multi-stage build with built-in `HEALTHCHECK`, optimised release profile (LTO, symbol strip)

## How It Works

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

1. Provider YAML configs are loaded at server startup and **watched for changes** — any create, modify, or delete of a `.yaml`/`.yml` file is picked up automatically without restarting.
2. The Axum-based REST API proxies requests to external provider APIs based on the YAML definitions.
3. The Leptos WASM frontend communicates with the REST API to display providers, handle login flows, browse and search channels, and play streams.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust + [Axum](https://github.com/tokio-rs/axum) |
| Frontend | Rust/WASM via [Leptos](https://leptos.dev/) + Tailwind CSS |
| Async Runtime | [Tokio](https://tokio.rs/) |
| HTTP Client | [Reqwest](https://docs.rs/reqwest) |
| Database | [SQLx](https://github.com/launchbadge/sqlx) (SQLite / PostgreSQL / MySQL) |
| JSONPath | [jsonpath-rust](https://github.com/besok/jsonpath-rust) |
| JSON Schema | [schemars](https://graham.cool/schemars/) |
| File Watching | [notify](https://github.com/notify-rs/notify) |
| Build | Cargo + [Trunk](https://trunkrs.dev/) (WASM bundler) |
| Auth | JWT + Argon2id password hashing |
| Containerization | Docker (multi-stage build, built-in health check) |

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
│       │   ├── state.rs            # AppState (RwLock provider map, proxy context cache)
│       │   ├── watcher.rs          # File-system watcher for hot-reloading YAMLs
│       │   ├── db.rs               # SQLx database layer
│       │   ├── auth_middleware.rs  # JWT creation / validation / extractors
│       │   ├── provider_client.rs  # HTTP client with template resolution & unresolved-placeholder warnings
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
            ├── home.rs             # Provider listing
            ├── login.rs            # OTVI user login / registration
            ├── setup.rs            # First-run admin setup wizard
            ├── app_login.rs        # Provider authentication flows
            ├── channels.rs         # Channel grid (search, URL-persisted category, skeletons)
            ├── player.rs           # Video player (resolved name/logo, loading skeleton)
            ├── admin.rs            # User management dashboard
            ├── change_password.rs  # Forced + voluntary password change
            └── not_found.rs        # 404 page
```

## License

OTVI is licensed under **CC BY-NC-SA 4.0**. See [LICENSE](https://github.com/rabilrbl/otvi/blob/main/LICENSE) for details.