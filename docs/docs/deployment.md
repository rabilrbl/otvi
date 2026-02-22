---
sidebar_position: 10
title: Deployment
---

# Deployment

OTVI can be deployed using Docker (recommended) or as a standalone binary.

## Docker Deployment (Recommended)

### Using Docker Compose

The simplest way to deploy OTVI:

```bash
git clone https://github.com/rabilrbl/otvi.git
cd otvi
docker compose up --build -d
```

The application is available at **http://localhost:3000**.

### `docker-compose.yml`

```yaml
services:
  otvi:
    build: .
    ports:
      - "3000:3000"
    volumes:
      - ./providers:/app/providers:ro
    environment:
      PORT: "3000"
      PROVIDERS_DIR: "/app/providers"
      STATIC_DIR: "/app/dist"
      RUST_LOG: "otvi_server=info"
      DATABASE_URL: "sqlite:///app/data.db"
      JWT_SECRET: "change_me_to_a_long_random_string"
```

### Custom Docker Build

```bash
docker build -t otvi .
docker run -d \
  -p 3000:3000 \
  -v ./providers:/app/providers:ro \
  -e DATABASE_URL=sqlite:///app/data.db \
  -e JWT_SECRET=$(openssl rand -hex 32) \
  -e RUST_LOG=otvi_server=info \
  otvi
```

### Dockerfile Overview

The Dockerfile uses a multi-stage build:

1. **Stage 1 — Frontend Build:** Installs Rust + Trunk + wasm32 target, builds the Leptos frontend to WASM.
2. **Stage 2 — Server Build:** Compiles the Axum server binary in release mode.
3. **Stage 3 — Runtime:** Minimal Debian Bookworm image with just the binary, frontend assets, and CA certificates.

## Standalone Binary

### Build from Source

```bash
# 1. Build the frontend
cd web
trunk build --release
cd ..

# 2. Build the server
cargo build --release -p otvi-server

# 3. The binary is at target/release/otvi-server
```

### Run

```bash
# Set environment variables
export DATABASE_URL=sqlite://data.db
export JWT_SECRET=$(openssl rand -hex 32)
export PORT=3000
export PROVIDERS_DIR=./providers
export STATIC_DIR=./dist
export RUST_LOG=otvi_server=info

# Run the server
./target/release/otvi-server
```

Or use a `.env` file:

```bash
cp .env.example .env
# Edit .env with your settings
./target/release/otvi-server
```

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
- Configure with: `DATABASE_URL=sqlite://data.db`

#### PostgreSQL

- Recommended for production and multi-instance deployments
- Configure with: `DATABASE_URL=postgres://user:pass@host:5432/otvi`

#### MySQL / MariaDB

- Alternative to PostgreSQL
- Configure with: `DATABASE_URL=mysql://user:pass@host:3306/otvi`

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

### Logging

Control log verbosity with `RUST_LOG`:

```bash
# Production (minimal logging)
RUST_LOG=otvi_server=info

# Debugging
RUST_LOG=otvi_server=debug

# Maximum detail
RUST_LOG=otvi_server=trace
```

### Security Checklist

- [ ] Set a strong, persistent `JWT_SECRET`
- [ ] Disable public signup after creating initial users
- [ ] Use HTTPS (via reverse proxy)
- [ ] Mount `providers/` as read-only in Docker
- [ ] Restrict database access
- [ ] Keep Rust dependencies updated
