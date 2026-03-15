---
sidebar_position: 10
title: Deployment
---

# Deployment

OTVI can be deployed from GHCR images (recommended), Docker source builds, or standalone binaries.

## Docker Deployment (Recommended)

### Prebuilt GHCR Images

Use the published GitHub Container Registry images when you want a pinned release instead of building locally.

These images are now multi-architecture, supporting linux/amd64, linux/arm64, and linux/arm/v7 platforms, allowing them to run on a wide range of hardware including x86_64 servers, Apple Silicon Macs, and Raspberry Pi devices.

- Full app with embedded frontend: `ghcr.io/rabilrbl/otvi:v0`
- API-only image: `ghcr.io/rabilrbl/otvi-server:v0`

The registry publishes these tags:

- `dev`
- `main`
- `vX`
- `vX.Y`
- `vX.Y.Z`

No `latest` tag is published.

Example production compose using the current stable major line:

```yaml
services:
  otvi:
    image: ghcr.io/rabilrbl/otvi:v0
    ports:
      - "3000:3000"
    volumes:
      - ./providers:/app/providers:ro
      - ./data:/app/data
    environment:
      PORT: "3000"
      PROVIDERS_DIR: "/app/providers"
      DATABASE_URL: "sqlite:///app/data/data.db"
      JWT_SECRET: "change_me_to_a_long_random_string"
      CORS_ORIGINS: "https://tv.example.com"
```

### Using Docker Compose

The simplest way to deploy OTVI in production:

```bash
git clone https://github.com/rabilrbl/otvi.git
cd otvi
docker compose up --build -d
```

The application is available at **http://localhost:3000**.

### `docker-compose.yml` (Production)

```yaml
services:
  otvi:
    build: .
    ports:
      - "3000:3000"
    volumes:
      - ./providers:/app/providers:ro
      - ./data:/app/data
    environment:
      PORT: "3000"
      PROVIDERS_DIR: "/app/providers"
      STATIC_DIR: "/app/dist"
      RUST_LOG: "otvi_server=info"
      LOG_FORMAT: "text"
      DATABASE_URL: "sqlite:///app/data/data.db"
      JWT_SECRET: "change_me_to_a_long_random_string"
      CORS_ORIGINS: "https://tv.example.com"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/healthz"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
    restart: unless-stopped
```

### `docker-compose.dev.yml` (Development)

A dedicated dev compose file is provided for local development. It enables hot-reload and verbose logging without auto-restart:

```bash
docker compose -f docker-compose.dev.yml up --build
```

Key differences from the production compose:

| Feature | Production | Development |
|---------|-----------|-------------|
| Provider mount | Read-only (`:ro`) | Read-write (bind mount — changes hot-reloaded) |
| Database path | `/app/data/data.db` | `./data/data.db` (local) |
| Log level | `info` | `debug` |
| Log format | `text` | `text` |
| CORS | Restricted | Permissive (unset) |
| Restart policy | `unless-stopped` | None (stops on `Ctrl-C`) |

```yaml
# docker-compose.dev.yml
services:
  otvi:
    build: .
    ports:
      - "3000:3000"
    volumes:
      - ./providers:/app/providers   # read-write for hot-reload
      - ./data:/app/data
    environment:
      PORT: "3000"
      PROVIDERS_DIR: "/app/providers"
      STATIC_DIR: "/app/dist"
      RUST_LOG: "otvi_server=debug"
      LOG_FORMAT: "text"
      DATABASE_URL: "sqlite:///app/data/data.db"
      JWT_SECRET: "dev_secret_not_for_production"
      # CORS_ORIGINS not set → permissive (dev only)
```

### Custom Docker Build

```bash
docker build -t otvi .
docker run -d \
  -p 3000:3000 \
  -v ./providers:/app/providers:ro \
  -v ./data:/app/data \
  -e DATABASE_URL=sqlite:///app/data/data.db \
  -e JWT_SECRET=$(openssl rand -hex 32) \
  -e CORS_ORIGINS=https://tv.example.com \
  -e LOG_FORMAT=json \
  -e RUST_LOG=otvi_server=info \
  otvi
```

### Dockerfile Overview

The Dockerfile uses a three-stage build:

1. **Stage 1 — Frontend Build:** Installs Rust + Trunk + the `wasm32-unknown-unknown` target, then builds the Leptos frontend to WASM.
2. **Stage 2 — Server Build:** Compiles the Axum server binary in release mode using the optimised `[profile.release]` settings (LTO, symbol stripping, `panic = abort`).
3. **Stage 3 — Runtime:** Minimal Debian Bookworm image containing only the binary, frontend assets, and CA certificates.

The image includes a built-in `HEALTHCHECK` directive pointing at `/healthz` so container orchestrators can monitor liveness automatically.

### API-only Image

If you only want the backend APIs and do not need the bundled frontend, use the API-only image:

```bash
docker run -d \
  -p 3000:3000 \
  -v ./providers:/app/providers:ro \
  -v ./data:/app/data \
  -e DATABASE_URL=sqlite:///app/data/data.db \
  -e JWT_SECRET=$(openssl rand -hex 32) \
  ghcr.io/rabilrbl/otvi-server:v0
```

#### Release Profile

The server binary is built with:

```toml
[profile.release]
lto           = "thin"    # link-time optimisation → smaller binary
codegen-units = 1         # maximum per-unit optimisation
strip         = "symbols" # remove debug symbols
panic         = "abort"   # eliminate unwinding code
```

This typically reduces the binary size by 20–40% compared to default release settings.

## Standalone Binary

Release assets publish two tarballs for each `vX.Y.Z` tag:

- bundled `otvi` release artifact with the frontend embedded into the binary and the executable named `otvi`
- `otvi-server` release artifact for API-only use

### Build from Source

```bash
# 1. Build the frontend
cd web
trunk build --release
cd ..

# 2. Build the server (uses [profile.release] from Cargo.toml)
cargo build --release -p otvi-server

# 3. The binary is at target/release/otvi-server
```

### Run

Bundled binary (frontend embedded):

```bash
export DATABASE_URL=sqlite://data.db
export JWT_SECRET=$(openssl rand -hex 32)
export PORT=3000
export PROVIDERS_DIR=./providers
export RUST_LOG=otvi_server=info
export LOG_FORMAT=text
export CORS_ORIGINS=https://tv.example.com

./otvi
```

API-only binary (serves frontend files from disk):

```bash
export DATABASE_URL=sqlite://data.db
export JWT_SECRET=$(openssl rand -hex 32)
export PORT=3000
export PROVIDERS_DIR=./providers
export STATIC_DIR=./dist
export RUST_LOG=otvi_server=info
export LOG_FORMAT=text
export CORS_ORIGINS=https://tv.example.com

./otvi-server
```

Or use a `.env` file:

```bash
cp .env.example .env
# Edit .env with your settings
./target/release/otvi-server
```

## Health & Readiness Probes

Two lightweight endpoints are available for orchestrators, load balancers, and reverse proxies:

| Endpoint | Auth | Description |
|----------|------|-------------|
| `GET /healthz` | None | **Liveness probe** — returns `200 OK` immediately. Use this to detect a crashed/hung process. |
| `GET /readyz` | None | **Readiness probe** — checks database connectivity before responding. Returns `503` if the DB is unavailable. |

### Kubernetes Example

```yaml
livenessProbe:
  httpGet:
    path: /healthz
    port: 3000
  initialDelaySeconds: 5
  periodSeconds: 15

readinessProbe:
  httpGet:
    path: /readyz
    port: 3000
  initialDelaySeconds: 10
  periodSeconds: 10
```

### Docker Compose Health Check

Both `docker-compose.yml` and the `Dockerfile` declare a health check targeting `/healthz`:

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:3000/healthz"]
  interval: 30s
  timeout: 5s
  retries: 3
  start_period: 10s
```

## Environment Variables

| Variable                 | Default              | Description                                                                                       |
| ------------------------ | -------------------- | ------------------------------------------------------------------------------------------------- |
| `DATABASE_URL`           | `sqlite://data.db`   | Database connection string — supports `sqlite://`, `postgres://`, `mysql://`                      |
| `JWT_SECRET`             | *(random)*           | Secret for signing JWTs. **Always set a persistent value in production.**                         |
| `PORT`                   | `3000`               | Port the server listens on                                                                        |
| `PROVIDERS_DIR`          | `providers`          | Directory scanned for `*.yaml` / `*.yml` provider configs (hot-reloaded on change)               |
| `STATIC_DIR`             | `dist`               | Directory served as the static frontend build                                                     |
| `RUST_LOG`               | `otvi_server=info`   | Log filter ([`tracing` format](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/))   |
| `LOG_FORMAT`             | `text`               | `text` for human-readable logs, `json` for structured output (Loki, Datadog, CloudWatch, etc.)   |
| `CORS_ORIGINS`           | *(permissive)*       | Comma-separated allowed origins, e.g. `https://tv.example.com`. Unset = allow all (dev only).    |
| `CHANNEL_CACHE_TTL_SECS` | `86400` (24 h)       | TTL for the server-side channel and category list cache. Entries are also invalidated immediately on provider login / logout, so reducing this is rarely necessary. |

## Production Considerations

### JWT Secret

Always set a persistent `JWT_SECRET` in production:

```bash
# Generate a strong secret
openssl rand -hex 32
```

If `JWT_SECRET` is not set, a random value is generated on each restart, invalidating all existing tokens.

### Database

#### SQLite (Default)

- Good for single-instance deployments
- File-based, no external dependencies
- Mount `./data` as a persistent volume in Docker
- Configure with: `DATABASE_URL=sqlite:///app/data/data.db`

#### PostgreSQL

- Recommended for production and multi-instance deployments
- Configure with: `DATABASE_URL=postgres://user:pass@host:5432/otvi`

#### MySQL / MariaDB

- Alternative to PostgreSQL
- Configure with: `DATABASE_URL=mysql://user:pass@host:3306/otvi`

### Structured Logging

For production environments with a log-aggregation stack (Grafana Loki, Datadog, AWS CloudWatch), switch to JSON output:

```bash
LOG_FORMAT=json
RUST_LOG=otvi_server=info
```

Each log line becomes a single parseable JSON object, enabling structured querying and alerting.

### Reverse Proxy

When running behind a reverse proxy (nginx, Caddy, Traefik):

#### Nginx Example

```nginx
server {
    listen 80;
    server_name otvi.example.com;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

#### Caddy Example

```
otvi.example.com {
    reverse_proxy localhost:3000
}
```

When running behind a TLS-terminating reverse proxy, set `CORS_ORIGINS` to the public HTTPS origin:

```bash
CORS_ORIGINS=https://otvi.example.com
```

### CORS Configuration

`CORS_ORIGINS` controls which browser origins are allowed to make cross-origin requests to the API:

```bash
# Single origin
CORS_ORIGINS=https://tv.example.com

# Multiple origins
CORS_ORIGINS=https://tv.example.com,https://admin.example.com
```

:::warning
Leaving `CORS_ORIGINS` unset permits all origins and is only safe for local development. The server logs a warning at startup when running in permissive mode.
:::

### Log Levels

Control log verbosity with `RUST_LOG`:

```bash
# Production (minimal logging)
RUST_LOG=otvi_server=info

# Debugging
RUST_LOG=otvi_server=debug

# Maximum detail
RUST_LOG=otvi_server=trace
```

## Security Checklist

- [ ] Set a strong, persistent `JWT_SECRET` (e.g., `openssl rand -hex 32`)
- [ ] Set `CORS_ORIGINS` to your frontend's exact origin(s)
- [ ] Disable public signup after creating initial users (`signup_disabled: true`)
- [ ] Use HTTPS (via reverse proxy with TLS termination)
- [ ] Mount `providers/` as read-only in Docker (`:ro`)
- [ ] Use a persistent volume for the database file
- [ ] Restrict database network access
- [ ] Set `LOG_FORMAT=json` and ship logs to a centralised aggregator
- [ ] Configure health-check probes in your orchestrator
- [ ] Rotate `JWT_SECRET` periodically (note: rotation invalidates all active sessions)
- [ ] Keep Rust dependencies updated (`cargo update`)
