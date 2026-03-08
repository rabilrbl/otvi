---
sidebar_position: 2
title: Getting Started
---

# Getting Started

This guide walks you through setting up OTVI from scratch — from installing prerequisites to running your first provider.

## Prerequisites

### Required

- **Rust** stable toolchain (1.83 or later)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **wasm32-unknown-unknown** target (for the frontend)
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

- **Trunk** CLI (WASM bundler for Leptos)
  ```bash
  cargo install trunk
  ```

### Optional

- **Docker** & **Docker Compose** — for containerized deployment
- **mitmproxy**, **Charles Proxy**, or **HTTP Toolkit** — for capturing provider API traffic

## Quick Start (Development)

### 1. Clone the Repository

```bash
git clone https://github.com/rabilrbl/otvi.git
cd otvi
```

### 2. Configure Environment

```bash
cp .env.example .env
```

Edit `.env` as needed. The defaults work out of the box with SQLite:

```bash
DATABASE_URL=sqlite://data.db
JWT_SECRET=change_me_to_a_long_random_string
PORT=3000
PROVIDERS_DIR=providers
STATIC_DIR=dist
RUST_LOG=otvi_server=info
LOG_FORMAT=text
# CORS_ORIGINS=https://tv.example.com   # leave unset in development
```

### 3. Build the Frontend

```bash
cd web
trunk build
cd ..
```

This compiles the Leptos frontend to WASM and outputs the build artifacts to the `dist/` directory.

### 4. Run the Server

```bash
cargo run -p otvi-server
```

The server starts at **http://localhost:3000**.

### 5. Initial Setup

1. Open **http://localhost:3000** in your browser.
2. You will be prompted to create the **first admin account**.
3. After registration, log in with your admin credentials.
4. Navigate to a provider and authenticate with its credentials.
5. Browse channels and start streaming!

## Quick Start (Docker)

### Production

```bash
git clone https://github.com/rabilrbl/otvi.git
cd otvi
docker compose up --build
```

The application is available at **http://localhost:3000**.

### Development (with hot-reload)

```bash
docker compose -f docker-compose.dev.yml up --build
```

The dev compose file:
- Bind-mounts `./providers` so YAML changes are picked up without rebuilding the image.
- Uses `./data/` for the SQLite database.
- Enables `DEBUG`-level logging (`RUST_LOG=otvi_server=debug`).
- Does **not** set `restart: always`, so containers stop when you `Ctrl-C`.

## Hot-Reload Provider Configs

The server **watches the `providers/` directory** for changes. Any time you create,
modify, or delete a `.yaml` / `.yml` file the provider map is atomically swapped in
memory — **no restart is required**. Changes are reflected within ~300 ms.

```bash
# While the server is running, just edit a file and save:
$EDITOR providers/myprovider.yaml
# → The running server picks up the change automatically.
```

## Adding a Provider

1. Copy `providers/example.yaml` to a new file (e.g., `providers/myprovider.yaml`).
2. Edit the YAML file with the API endpoints captured from your provider's app.
3. **Save** — the running server picks up the change within ~300 ms (no restart needed).

See the [Provider Guide](./providers/overview) for a complete walkthrough.

## Password Policy

All passwords must satisfy:

- Minimum **8 characters**
- At least one **uppercase** ASCII letter
- At least one **digit**

This applies to self-registration, the `change-password` endpoint, and admin-created or
admin-reset passwords. A clear error is returned when the policy is not met.

Accounts created by an admin are flagged `must_change_password = true`. Those accounts
are **blocked from all API calls** (returning `403 Forbidden`) until the user changes
their password via `POST /api/auth/change-password`.

## Environment Variables

| Variable        | Default              | Description                                                                                     |
| --------------- | -------------------- | ----------------------------------------------------------------------------------------------- |
| `DATABASE_URL`  | `sqlite://data.db`   | Database connection string — supports `sqlite://`, `postgres://`, `mysql://`                    |
| `JWT_SECRET`    | *(random)*           | Secret for signing JWTs. **Always set a persistent value in production.**                       |
| `PORT`          | `3000`               | Port the server listens on                                                                      |
| `PROVIDERS_DIR` | `providers`          | Directory scanned for `*.yaml` / `*.yml` provider configs (hot-reloaded on change)             |
| `STATIC_DIR`    | `dist`               | Directory served as the static frontend build                                                   |
| `RUST_LOG`      | `otvi_server=info`   | Log filter ([`tracing` format](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/)) |
| `LOG_FORMAT`    | `text`               | Set to `json` for structured JSON logs (Loki, Datadog, CloudWatch, etc.)                        |
| `CORS_ORIGINS`  | *(permissive)*       | Comma-separated allowed origins, e.g. `https://tv.example.com`. Unset = allow all (dev only)    |

:::warning
Leaving `CORS_ORIGINS` unset is fine for local development but **should not be used in
production**. The server emits a warning at startup when CORS is permissive.
:::

## Development Workflow

### Watch Mode (Frontend)

For rapid frontend development, use Trunk's watch mode:

```bash
cd web
trunk serve --proxy-backend=http://localhost:3000/api
```

### YAML Auto-Complete in VS Code

Point the [YAML extension](https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml)
at the live schema endpoint for inline validation and auto-complete while editing
provider files:

```jsonc
// .vscode/settings.json
{
  "yaml.schemas": {
    "http://localhost:3000/api/schema/provider": "providers/*.yaml"
  }
}
```

### Running Tests

```bash
# Run all workspace tests
cargo test --workspace --all-features

# Run formatting check
cargo fmt --all -- --check

# Run linter
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Health & Readiness Probes

Two lightweight endpoints are available for orchestrators and reverse proxies:

| Endpoint  | Description                                      |
| --------- | ------------------------------------------------ |
| `GET /healthz` | Liveness probe — always returns `200 OK` instantly |
| `GET /readyz`  | Readiness probe — checks database connectivity before returning `200 OK` |

```bash
curl http://localhost:3000/healthz   # → 200 OK
curl http://localhost:3000/readyz    # → 200 OK (or 503 if DB unavailable)
```

## Next Steps

- [Architecture](./architecture) — understand the system design
- [Configuration](./configuration) — environment variables and settings
- [Provider Guide](./providers/overview) — create your own provider configs
- [API Reference](./api-reference/overview) — REST endpoint documentation
- [Deployment](./deployment) — production deployment with Docker
