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

OTVI user authentication page. Accepts username and password. New users can register from this page (unless signup is disabled by an admin).

### Provider Login (`/login/:provider_id`)

Shows the provider's available authentication flows. Users select a flow and fill in the required fields. For multi-step flows (e.g., phone + OTP), additional prompt fields appear between steps automatically.

### Channels (`/providers/:provider_id/channels`)

Channel browsing page featuring:

- **Channel grid** — cards showing name, logo, and channel number.
- **Search box** — a text input with a clear (×) button above the grid. Typing sends a `?search=<term>` query to the server, which performs a case-insensitive substring match on channel names before pagination is applied. The grid updates in real-time as the user types.
- **Category filter** — a dropdown that filters channels by category. The selected category is stored in the URL as `?cat=<id>`, making filtered views bookmarkable and restoring state when the user navigates back.
- **Skeleton loading states** — while the channel list is loading, an 18-card placeholder grid is shown so the layout does not shift when data arrives.
- **URL-persisted filters** — both `?cat=` and `?search=` are reflected in the browser URL, so sharing or bookmarking a filtered view works correctly.

### Player (`/providers/:provider_id/play/:channel_id`)

Video player page featuring:

- **Full-width video element** — adapts to the viewport.
- **Automatic player selection** — HLS.js for HLS streams, Shaka Player for DASH + DRM.
- **Channel info card** — displayed below the video, showing the resolved **channel name** and **logo**. The page fetches the provider's channel list on mount to resolve these details. A spinner skeleton is shown in the info card while the details load.
- **DRM licence handling** — configured headers are injected into licence acquisition requests automatically.
- **Loading overlay** — a spinning loader is displayed over the video element while the stream initialises.

### Change Password (`/change-password`)

Shown automatically as a full-screen overlay when `must_change_password` is `true` on the current user's account (i.e., the account was created or had its password reset by an admin). The user must set a new password that satisfies the password policy before they can access any other part of the application.

Users can also navigate here voluntarily to change their password at any time.

### Admin (`/admin`)

Admin dashboard (visible only to users with the `admin` role) with:

- User list with role indicators.
- Create new user form with role and provider-access fields.
- Per-user provider access management.
- Password reset functionality (sets `must_change_password = true` on the target user).
- Server settings toggle (disable/enable public signup).

### 404 (`*`)

A friendly not-found page rendered for any unmatched route.

## Authentication Flow

```
App Startup
    │
    ├── GET /api/auth/me
    │     │
    │     ├── 404 (no users) → /setup
    │     │
    │     ├── 401 (not logged in) → /login
    │     │
    │     ├── 200 + must_change_password = true → ChangePassword overlay
    │     │
    │     └── 200 → Ready (show app)
    │
    └── JWT stored in LocalStorage (key: "otvi_jwt")
```

The `AuthCtx` context is shared across all components and provides:
- Current user info (id, username, role, `must_change_password`)
- Admin status check
- Login/logout functions

## Channel Search & Filtering

### Search

```
User types in search box
    → frontend sends GET /api/providers/:id/channels?search=<term>
    → server applies case-insensitive substring filter on channel names
    → filtered + paginated results returned
    → channel grid re-renders with results
```

The `?search=` query is evaluated **server-side before pagination**, so `total`
in the response reflects the number of matching channels, not the total
unfiltered count.

### Category Filter

```
User selects a category
    → URL updated to ?cat=<category_id>
    → frontend sends GET /api/providers/:id/channels?category=<id>
    → server filters channels by category
    → URL change is pushed to browser history (bookmarkable / back-button aware)
```

Both `?search=` and `?cat=` can be combined in the same request.

## Skeleton Loading States

The UI uses skeleton placeholders instead of blank screens while data loads:

| Page     | Skeleton behaviour                                           |
| -------- | ------------------------------------------------------------ |
| Channels | 18-card placeholder grid shown while the channel list loads  |
| Player   | Spinner overlay on the video element while the stream starts |
| Player info card | Placeholder name/logo shown while channel details resolve |

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
    → Shaka Player initialises with DRM config
    → Licence requests include configured headers
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
trunk serve --proxy-backend=http://localhost:3000/api
```

This starts a development server with hot reload. The backend must be running separately for API calls to succeed.

### Tailwind CSS

Styles are defined in `input.css` and compiled by Tailwind CSS v4 during the Trunk build process. The generated `style.css` is included in `index.html`.

## File Structure

```
web/
├── Trunk.toml
├── index.html           # HTML entry + HLS.js / Shaka Player bridge functions
├── input.css            # Tailwind CSS source
├── style.css            # Generated (do not edit manually)
└── src/
    ├── app.rs           # Root component, routing, AuthCtx, must_change_password overlay
    ├── api.rs           # Backend HTTP client (token storage, typed API calls)
    └── pages/
        ├── home.rs              # Provider listing
        ├── login.rs             # OTVI user login / registration
        ├── setup.rs             # First-run admin setup wizard
        ├── app_login.rs         # Provider authentication flows (multi-step)
        ├── channels.rs          # Channel grid (search, URL-persisted category, skeletons)
        ├── player.rs            # Video player (resolved name/logo, loading skeleton)
        ├── admin.rs             # User management dashboard
        ├── change_password.rs   # Forced + voluntary password change
        └── not_found.rs         # 404 page
```
