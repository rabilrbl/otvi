---
sidebar_position: 2
title: Getting Started
---

# Getting Started

This guide walks you through setting up OTVI from scratch — from installing prerequisites to running your first provider.

## Prerequisites

### Required

- **Rust** stable toolchain (1.75 or later)
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

```bash
git clone https://github.com/rabilrbl/otvi.git
cd otvi
docker compose up --build
```

The application is available at **http://localhost:3000**.

The Docker setup:
- Builds the WASM frontend in a separate stage
- Compiles the server binary in release mode
- Creates a minimal runtime image based on Debian Bookworm
- Mounts `./providers` as a read-only volume

## Adding a Provider

1. Copy `providers/example.yaml` to a new file (e.g., `providers/myprovider.yaml`).
2. Edit the YAML file with the API endpoints captured from your provider's app.
3. Restart the server to load the new provider.

See the [Provider Guide](./providers/overview) for a complete walkthrough.

## Development Workflow

### Watch Mode (Frontend)

For rapid frontend development, use Trunk's watch mode:

```bash
cd web
trunk serve --proxy-backend=http://localhost:3000/api
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

## Next Steps

- [Architecture](./architecture) — understand the system design
- [Configuration](./configuration) — environment variables and settings
- [Provider Guide](./providers/overview) — create your own provider configs
- [API Reference](./api-reference/overview) — REST endpoint documentation
- [Deployment](./deployment) — production deployment with Docker
