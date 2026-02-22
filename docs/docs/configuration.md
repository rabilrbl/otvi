---
sidebar_position: 4
title: Configuration
---

# Configuration

OTVI is configured through environment variables. Copy `.env.example` to `.env` and adjust as needed.

## Environment Variables

### Database

```bash
# SQLite (default – file created automatically on first run)
DATABASE_URL=sqlite://data.db

# PostgreSQL
DATABASE_URL=postgres://user:password@localhost:5432/otvi

# MySQL / MariaDB
DATABASE_URL=mysql://user:password@localhost:3306/otvi
```

OTVI uses [SQLx](https://github.com/launchbadge/sqlx) with `AnyPool`, meaning you can switch databases at runtime simply by changing the connection string. The appropriate migrations are applied automatically at startup.

| Database | Connection String Format |
|----------|------------------------|
| SQLite | `sqlite://path/to/file.db` |
| PostgreSQL | `postgres://user:pass@host:5432/dbname` |
| MySQL | `mysql://user:pass@host:3306/dbname` |

### JWT Authentication

```bash
# Secret used to sign & verify JWT tokens.
# Generate a strong random value:
#   openssl rand -hex 32
# If unset, a random UUID is used (tokens are invalidated on restart).
JWT_SECRET=change_me_to_a_long_random_string
```

:::warning
If `JWT_SECRET` is not set, a random value is generated on each server start. This means **all existing JWT tokens become invalid** after a restart. Always set a persistent secret in production.
:::

### Server

```bash
# Port the server listens on (default: 3000)
PORT=3000
```

### Paths

```bash
# Directory containing provider YAML files
PROVIDERS_DIR=providers

# Directory to serve as the static frontend build
STATIC_DIR=dist
```

### Logging

```bash
# Tracing filter string (RUST_LOG format)
# Examples:
#   otvi_server=debug      – verbose server logs
#   otvi_server=trace      – maximum detail
#   info                   – all crates at info level
RUST_LOG=otvi_server=info
```

## Complete `.env.example`

```bash
# ─────────────────────────────────────────────────────────
# otvi-server  –  environment configuration
# ─────────────────────────────────────────────────────────

# ── Database ─────────────────────────────────────────────
DATABASE_URL=sqlite://data.db

# ── JWT ──────────────────────────────────────────────────
JWT_SECRET=change_me_to_a_long_random_string

# ── Server ───────────────────────────────────────────────
PORT=3000

# ── Paths ────────────────────────────────────────────────
PROVIDERS_DIR=providers
STATIC_DIR=dist

# ── Logging ──────────────────────────────────────────────
RUST_LOG=otvi_server=info
```

## Server Settings

Some settings can be changed at runtime through the admin API:

| Setting | Default | Description |
|---------|---------|-------------|
| `signup_disabled` | `false` | When `true`, new user registration is disabled (admin can still create users) |

These settings are stored in the database and can be managed via:
- The admin dashboard in the web UI
- `GET /api/admin/settings` and `PUT /api/admin/settings` API endpoints

## First-Time Setup

When the server starts with an empty database:

1. The first user to register automatically becomes an **admin**.
2. After the admin account is created, additional users can register (unless signup is disabled).
3. Admins can create users with specific roles and provider access restrictions.

:::tip
For security, disable public signup after creating initial user accounts by setting `signup_disabled` to `true` in the admin settings.
:::
