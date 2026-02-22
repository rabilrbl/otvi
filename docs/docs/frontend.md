---
sidebar_position: 9
title: Frontend
---

# Frontend Guide

OTVI's frontend is built with [Leptos](https://leptos.dev/) — a Rust web framework that compiles to WebAssembly. It uses [Tailwind CSS](https://tailwindcss.com/) for styling.

## Build Process

The frontend is compiled using [Trunk](https://trunkrs.dev/):

```bash
cd web
trunk build           # Production build → outputs to ../dist/
trunk serve           # Development server with hot reload
```

### Build Configuration

The `Trunk.toml` file configures the build:
- **Output directory:** `../dist` (served by the backend as static files)
- **Watch paths:** `src/`, `index.html`, `input.css`
- **Pre-build hook:** Runs Tailwind CSS v4 to generate styles

## Pages

### Home (`/`)

Displays all providers accessible to the current user. Each provider card shows the name, logo, and a link to authenticate or browse channels.

### Setup (`/setup`)

Shown on first run when no users exist. Prompts for creating the first admin account.

### Login (`/login`)

OTVI user authentication page. Accepts username and password.

### Provider Login (`/login/:provider_id`)

Shows the provider's available authentication flows. Users select a flow and fill in the required fields. For multi-step flows, additional prompts appear between steps.

### Channels (`/providers/:provider_id/channels`)

Channel browsing page with:
- Channel grid showing name, logo, and number
- Category filter dropdown
- Click-to-play navigation

### Player (`/providers/:provider_id/play/:channel_id`)

Video player page featuring:
- Full-width video element
- Automatic player selection (HLS.js or Shaka Player)
- DRM license handling for protected content

### Admin (`/admin`)

Admin dashboard with:
- User list with role indicators
- Create new user form
- Per-user provider access management
- Password reset functionality
- Server settings (signup toggle)

## Authentication Flow

```
App Startup
    │
    ├── GET /api/auth/me
    │     │
    │     ├── 401 → NeedsSetup (no users) or NeedsLogin
    │     │
    │     ├── 200 + must_change_password → ChangePassword overlay
    │     │
    │     └── 200 → Ready (show app)
    │
    └── JWT stored in LocalStorage (key: "otvi_jwt")
```

The `AuthCtx` context is shared across all components and provides:
- Current user info
- Admin status check
- Login/logout functions

## Video Playback

### HLS Streams

```
Frontend receives stream_type = "hls"
    → Calls window.otviInitHls(videoId, url)
    → HLS.js attaches to <video> element
    → Segments loaded via /api/proxy
```

### DASH + DRM Streams

```
Frontend receives stream_type = "dash" + drm config
    → Calls window.otviInitDash(videoId, url, drmConfigJson)
    → Shaka Player initializes with DRM config
    → License requests include configured headers
    → Content decrypted and played
```

### Player Cleanup

When navigating away from the player page, `otviDestroyPlayer()` is called to properly clean up HLS.js or Shaka Player instances and prevent memory leaks.

## Token Management

- JWT tokens are stored in `LocalStorage` under the key `otvi_jwt`.
- Every API request automatically includes the `Authorization: Bearer <token>` header.
- On `401` responses, the user is redirected to the login page.
- Tokens expire after 24 hours.

## Development

### Prerequisites

```bash
# Install Rust WASM target
rustup target add wasm32-unknown-unknown

# Install Trunk
cargo install trunk
```

### Dev Server

```bash
cd web
trunk serve
```

This starts a development server with hot reload. Note: you still need the backend running separately for API calls.

### Tailwind CSS

Styles are defined in `input.css` and compiled by Tailwind CSS v4 during the Trunk build process. The generated `style.css` is included in `index.html`.
