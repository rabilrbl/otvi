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

| Crate           | Purpose                                                                       |
| --------------- | ----------------------------------------------------------------------------- |
| **otvi-core**   | Shared types: YAML config schema, API request/response types, template engine |
| **otvi-server** | Axum REST API, loads provider YAMLs, proxies API calls, serves frontend       |
| **otvi-web**    | Leptos CSR frontend compiled to WASM (trunk)                                  |

## Quick Start

### Prerequisites

- Rust stable (1.75+)
- `trunk` CLI for the WASM frontend: `cargo install trunk`
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`

### Build & Run (development)

```bash
# 1. Build the frontend
cd web && trunk build && cd ..

# 2. Run the server
cargo run -p otvi-server
# → http://localhost:3000
```

### Docker

```bash
docker compose up --build
# → http://localhost:3000
```

## Creating a Provider Config

Each provider is a single YAML file placed in the `providers/` directory.

### Step-by-step

1. **Capture traffic** from the provider's mobile / Android TV app using
   mitmproxy, Charles Proxy, or HTTP Toolkit.
2. **Copy** `providers/example.yaml` and rename it.
3. **Fill in** the API endpoints, headers, and body templates from your
   traffic captures.
4. **Restart** the server (or rebuild the Docker image).

### Template Variables

| Variable        | Description                                                               |
| --------------- | ------------------------------------------------------------------------- |
| `{{input.X}}`   | Value entered by the user in a form field                                 |
| `{{stored.X}}`  | Value extracted from a previous API response and persisted in the session |
| `{{extract.X}}` | Value extracted in the previous auth step                                 |
| `{{uuid}}`      | Auto-generated UUID (useful for device IDs)                               |

### Response Extraction

Values are extracted from JSON responses using dot-notation paths:

```yaml
on_success:
  extract:
    access_token: "$.data.access_token"
    user_name: "$.data.user.display_name"
```

`$.data.access_token` navigates into `{"data": {"access_token": "…"}}`.

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

## REST API Reference

| Method | Path                                      | Description                                  |
| ------ | ----------------------------------------- | -------------------------------------------- |
| GET    | `/api/providers`                          | List all loaded providers                    |
| GET    | `/api/providers/:id`                      | Provider details + auth flows                |
| POST   | `/api/providers/:id/auth/login`           | Login (handles multi-step)                   |
| POST   | `/api/providers/:id/auth/logout`          | Logout (requires `X-Session-Token`)          |
| GET    | `/api/providers/:id/channels`             | Browse channels (requires `X-Session-Token`) |
| GET    | `/api/providers/:id/channels/categories`  | List categories                              |
| GET    | `/api/providers/:id/channels/:cid/stream` | Get stream URL + DRM info                    |

## Project Structure

```
otvi/
├── Cargo.toml                  # Workspace
├── Dockerfile                  # Multi-stage build
├── docker-compose.yml
├── providers/
│   └── example.yaml            # Example provider config
├── crates/
│   ├── otvi-core/              # Shared types & template engine
│   └── otvi-server/            # Axum REST API server
└── web/                        # Leptos WASM frontend
    ├── Trunk.toml
    ├── index.html
    ├── style.css
    └── src/
```

## License

CC BY-NC-SA 4.0. See [LICENSE](LICENSE) for details.
