---
sidebar_position: 1
title: Introduction
---

# OTVI – Open TV Interface

OTVI is a generic, **YAML-driven television interface** that lets any TV provider expose login, logout, channel browsing, and live playback (HLS / DASH + DRM) through simple configuration files. No custom code is needed per provider — just describe the API in a YAML file.

## Key Features

- **Zero-code provider integration** — define everything in YAML
- **Multi-step authentication** — phone + OTP, email + password, SSO, and more
- **Template engine** — dynamic request building with `{{input.X}}`, `{{stored.X}}`, `{{uuid}}`
- **JSONPath response extraction** — pull values from API responses using dot notation
- **HLS & DASH streaming** — with full DRM support (Widevine, PlayReady)
- **Stream proxying** — transparent CDN authentication and CORS handling
- **Multi-user system** — JWT-based auth with admin/user roles
- **Per-user provider access control** — restrict which providers each user can access
- **Database flexibility** — SQLite, PostgreSQL, or MySQL at runtime
- **Modern web UI** — responsive Leptos/WASM frontend with Tailwind CSS
- **Docker ready** — multi-stage build for easy deployment

## How It Works

```
┌─────────────────────────────────────────────────────────┐
│                    YAML Provider Configs                │
│  providers/acme.yaml   providers/streammax.yaml  …      │
└────────────────────────┬────────────────────────────────┘
                         │ loaded at startup
                         ▼
┌──────────────── otvi-server (Axum) ─────────────────────┐
│  REST API   ─── provider_client ──▶  Provider HTTP APIs │
│  /api/…           (reqwest)                             │
│                                                         │
│  Static files ──▶ serves compiled WASM frontend         │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │ fetch / JSON
┌──────────────── otvi-web (Leptos WASM) ─────────────────┐
│  Home   Login   Channels   Player (HLS.js / Shaka)      │
└─────────────────────────────────────────────────────────┘
```

1. Provider YAML configs are loaded at server startup.
2. The Axum-based REST API proxies requests to external provider APIs based on the YAML definitions.
3. The Leptos WASM frontend communicates with the REST API to display providers, handle login flows, browse channels, and play streams.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust + [Axum](https://github.com/tokio-rs/axum) |
| Frontend | Rust/WASM via [Leptos](https://leptos.dev/) + Tailwind CSS |
| Async Runtime | [Tokio](https://tokio.rs/) |
| HTTP Client | [Reqwest](https://docs.rs/reqwest) |
| Database | [SQLx](https://github.com/launchbadge/sqlx) (SQLite / PostgreSQL / MySQL) |
| Build | Cargo + [Trunk](https://trunkrs.dev/) (WASM bundler) |
| Auth | JWT + Argon2 password hashing |
| Containerization | Docker (multi-stage build) |

## Project Structure

```
otvi/
├── Cargo.toml                  # Workspace configuration
├── Dockerfile                  # Multi-stage production build
├── docker-compose.yml          # Local development container
├── .env.example                # Environment variable reference
├── providers/                  # Provider YAML configurations
│   └── example.yaml
├── crates/
│   ├── otvi-core/              # Shared types & template engine
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs        # API request/response types
│   │       ├── config.rs       # YAML schema definitions
│   │       └── template.rs     # Template variable resolution
│   └── otvi-server/            # Axum REST API server
│       ├── src/
│       │   ├── main.rs         # Entry point & bootstrap
│       │   ├── db.rs           # Database layer (SQLx)
│       │   ├── state.rs        # Application state
│       │   ├── error.rs        # Error handling
│       │   ├── auth_middleware.rs  # JWT middleware
│       │   ├── provider_client.rs  # HTTP proxy client
│       │   └── api/            # Route handlers
│       │       ├── auth.rs     # Provider authentication
│       │       ├── user_auth.rs # OTVI user auth
│       │       ├── channels.rs # Channel browsing
│       │       ├── providers.rs # Provider listing
│       │       ├── proxy.rs    # Stream proxy
│       │       └── admin.rs    # Admin endpoints
│       └── migrations/         # Database schema migrations
├── web/                        # Leptos WASM frontend
│   ├── Trunk.toml              # WASM build config
│   ├── index.html              # HTML entry + player scripts
│   ├── input.css / style.css   # Tailwind CSS
│   └── src/
│       ├── main.rs
│       ├── app.rs              # Root component & routing
│       ├── api.rs              # Backend HTTP client
│       └── pages/              # UI pages
└── docs/                       # This documentation (Docusaurus)
```

## License

OTVI is licensed under **CC BY-NC-SA 4.0**. See [LICENSE](https://github.com/rabilrbl/otvi/blob/main/LICENSE) for details.
