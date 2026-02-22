---
sidebar_position: 3
title: Architecture
---

# Architecture

OTVI is built as a Rust workspace with three main crates, each handling a distinct layer of the application.

## System Overview

```
┌─────────────────────────────────────────────────────────┐
│                    YAML Provider Configs                │
│  providers/acme.yaml   providers/streammax.yaml  …      │
└────────────────────────┬────────────────────────────────┘
                         │ loaded at startup
                         ▼
┌──────────────── otvi-server (Axum) ─────────────────────┐
│                                                         │
│  ┌──────────┐  ┌───────────────┐  ┌──────────────────┐  │
│  │ REST API │──│ provider_client│──│ Provider HTTP    │  │
│  │ /api/…   │  │  (reqwest)    │  │ APIs (external)  │  │
│  └──────────┘  └───────────────┘  └──────────────────┘  │
│                                                         │
│  ┌──────────┐  ┌───────────────┐  ┌──────────────────┐  │
│  │ Auth MW  │  │   Database    │  │  Static Files    │  │
│  │  (JWT)   │  │   (SQLx)     │  │  (WASM frontend) │  │
│  └──────────┘  └───────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────┘
                         ▲
                         │ fetch / JSON
┌──────────────── otvi-web (Leptos WASM) ─────────────────┐
│  Home   Login   Channels   Player (HLS.js / Shaka)      │
└─────────────────────────────────────────────────────────┘
```

## Crate Overview

| Crate | Path | Purpose |
|-------|------|---------|
| **otvi-core** | `crates/otvi-core/` | Shared types: YAML config schema, API request/response types, template engine |
| **otvi-server** | `crates/otvi-server/` | Axum REST API, loads provider YAMLs, proxies API calls, serves frontend |
| **otvi-web** | `web/` | Leptos CSR frontend compiled to WASM via Trunk |

## otvi-core

The shared library that defines the contract between server and frontend.

### Key Modules

- **`config.rs`** — YAML schema types for provider configuration
  - `ProviderConfig`: top-level provider definition
  - `AuthFlow`, `AuthStep`: authentication flow definitions
  - `RequestSpec`: generic HTTP request specification with template support
  - `ResponseMapping`: JSONPath-based response field extraction
  - `PlaybackEndpoint`: stream URL and DRM configuration
  - `ProxyConfig`: stream proxy settings

- **`types.rs`** — API request/response types shared between server and client
  - Provider info, auth flow info, field info
  - Login request/response, multi-step session handling
  - Channel and category data structures
  - Stream info with DRM details
  - User management types (roles, registration, sessions)

- **`template.rs`** — Template variable resolution engine
  - `TemplateContext`: key-value store for variable bindings
  - `extract_json_path()`: JSONPath-like extraction from JSON values
  - Built-in variables: `{{uuid}}`, `{{utcnow}}`, `{{utcdate}}`

## otvi-server

The backend REST API built on [Axum](https://github.com/tokio-rs/axum).

### Key Modules

- **`main.rs`** — Application bootstrap
  - Loads `.env` configuration
  - Initializes database pool (SQLite/PostgreSQL/MySQL)
  - Creates JWT signing keys
  - Loads all provider YAML files
  - Sets up Axum router with nested API routes
  - Serves compiled WASM frontend as static files

- **`state.rs`** — Application state management
  - `AppState`: holds providers, database pool, JWT keys, HTTP client
  - `ProxyContext`: per-stream cache for headers and cookie mappings
  - `load_providers()`: scans directory for `*.yaml`/`*.yml` files

- **`db.rs`** — Database abstraction layer
  - User CRUD operations (create, get, update, delete)
  - Provider session management (upsert, get, delete)
  - Per-user provider access control
  - Server settings storage
  - Supports SQLite, PostgreSQL, and MySQL through SQLx's `AnyPool`

- **`auth_middleware.rs`** — JWT authentication middleware
  - Token creation and validation
  - `Claims` extractor for authenticated routes
  - `AdminClaims` extractor for admin-only routes
  - 24-hour token lifetime

- **`provider_client.rs`** — HTTP client for provider APIs
  - Template variable resolution in headers, params, and body
  - Default header merging
  - JSON and form-encoded request body support

- **`error.rs`** — Centralized error handling
  - `AppError` enum with HTTP status code mapping
  - JSON error response formatting

### API Route Modules

| Module | Routes | Description |
|--------|--------|-------------|
| `api/providers.rs` | `GET /api/providers`, `GET /api/providers/:id` | Provider listing and details |
| `api/auth.rs` | `POST /api/providers/:id/auth/login`, `POST .../logout`, `GET .../check` | Provider authentication |
| `api/channels.rs` | `GET /api/providers/:id/channels`, `.../categories`, `.../stream` | Channel browsing and stream info |
| `api/proxy.rs` | `GET /api/proxy` | HLS/DASH stream proxying |
| `api/user_auth.rs` | `POST /api/auth/register`, `.../login`, `.../change-password`, `GET .../me` | OTVI user authentication |
| `api/admin.rs` | `/api/admin/users`, `/api/admin/settings` | User and system administration |

## otvi-web

The frontend is built with [Leptos](https://leptos.dev/) and compiled to WebAssembly using [Trunk](https://trunkrs.dev/).

### Key Components

- **`app.rs`** — Root component with routing and authentication context
  - Boot state machine: Loading → NeedsSetup / NeedsLogin / Ready
  - Route definitions for all pages
  - Navbar with navigation and auth controls
  - Forced password-change overlay

- **`api.rs`** — HTTP client for backend communication
  - Token storage in `LocalStorage`
  - Automatic Bearer token injection
  - Typed request/response handling

- **Pages** (`pages/` directory):
  - `home.rs` — Provider listing
  - `login.rs` — OTVI user login
  - `setup.rs` — First-time admin setup
  - `app_login.rs` — Provider authentication flows
  - `channels.rs` — Channel browsing with category filtering
  - `player.rs` — Video player (HLS.js + Shaka Player)
  - `admin.rs` — User management dashboard

### Video Playback

The frontend uses a JavaScript bridge in `index.html` for video playback:

- **HLS.js** — for HLS streams (`.m3u8`)
- **Shaka Player** — for DASH streams with DRM support (Widevine, PlayReady)
- Bridge functions: `otviInitHls()`, `otviInitDash()`, `otviDestroyPlayer()`

## Data Flow

### Authentication Flow

```
User → Frontend → POST /api/auth/login (OTVI login)
                → JWT token stored in LocalStorage
                → POST /api/providers/:id/auth/login (Provider login)
                → Session stored in database
                → Channel browsing enabled
```

### Streaming Flow

```
Frontend → GET /api/providers/:id/channels/:cid/stream
         → Server fetches stream URL from provider API
         → Returns stream URL + DRM info + proxy context token
         → Frontend initializes HLS.js or Shaka Player
         → Video requests proxied through GET /api/proxy
         → Server handles CDN auth, CORS, header injection
```
