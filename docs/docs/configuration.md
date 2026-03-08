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

| Database   | Connection String Format                     |
| ---------- | -------------------------------------------- |
| SQLite     | `sqlite://path/to/file.db`                   |
| PostgreSQL | `postgres://user:pass@host:5432/dbname`       |
| MySQL      | `mysql://user:pass@host:3306/dbname`          |

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

### Rate Limiting

OTVI applies a two-tier IP-based rate limiter to all API routes using a
[token-bucket](https://en.wikipedia.org/wiki/Token_bucket) algorithm. Each IP
address gets its own independent bucket per tier.

| Tier    | Protected routes | Default quota |
| ------- | ---------------- | ------------- |
| Auth    | `POST /api/auth/login`, `POST /api/auth/register`, `POST /api/*/auth/login` | 5 req burst, +1 token every 10 s |
| General | All other `/api` routes | 20 req burst, +1 token every 1 s |

```bash
# Auth tier — protects login and register against brute-force attacks.
# Burst: maximum requests an IP can make before being throttled.
# Period: seconds between each token replenishment.
RATE_LIMIT_AUTH_BURST=5
RATE_LIMIT_AUTH_PERIOD_SECS=10

# General tier — broad throttling for all other API routes.
RATE_LIMIT_GENERAL_BURST=20
RATE_LIMIT_GENERAL_PERIOD_SECS=1
```

| Variable | Default | Description |
| -------- | ------- | ----------- |
| `RATE_LIMIT_AUTH_BURST` | `5` | Auth-tier token bucket burst capacity |
| `RATE_LIMIT_AUTH_PERIOD_SECS` | `10` | Auth-tier replenishment interval (seconds) |
| `RATE_LIMIT_GENERAL_BURST` | `20` | General-tier token bucket burst capacity |
| `RATE_LIMIT_GENERAL_PERIOD_SECS` | `1` | General-tier replenishment interval (seconds) |

When a client exceeds its quota the server responds with `429 Too Many Requests`
and includes a `X-RateLimit-*` header set indicating when the bucket will
refill.

:::note
Rate limiting is keyed by peer IP address. If OTVI runs behind a reverse proxy,
ensure the proxy forwards the real client IP (e.g. via `X-Forwarded-For`) and
that your network configuration is trusted, otherwise all clients will share the
same bucket.
:::

### Paths

```bash
# Directory containing provider YAML files (hot-reloaded on change)
PROVIDERS_DIR=providers

# Directory to serve as the static frontend build
STATIC_DIR=dist
```

### Logging

```bash
# Tracing filter string (RUST_LOG format).
# Examples:
#   otvi_server=debug      – verbose server logs
#   otvi_server=trace      – maximum detail
#   info                   – all crates at info level
RUST_LOG=otvi_server=info

# Log output format.
#   text  – human-readable (default)
#   json  – structured JSON, suitable for Loki, Datadog, CloudWatch, etc.
LOG_FORMAT=text
```

Setting `LOG_FORMAT=json` switches the `tracing-subscriber` formatter to JSON output. Each log line becomes a single JSON object, which log-aggregation tools can parse, index, and query natively.

**Text format (default):**
```
2024-01-15T10:23:45Z  INFO otvi_server: server listening on 0.0.0.0:3000
2024-01-15T10:23:46Z  WARN otvi_server::provider_client: unresolved placeholder {{stored.token}} in header Authorization
```

**JSON format (`LOG_FORMAT=json`):**
```json
{"timestamp":"2024-01-15T10:23:45Z","level":"INFO","target":"otvi_server","message":"server listening on 0.0.0.0:3000"}
{"timestamp":"2024-01-15T10:23:46Z","level":"WARN","target":"otvi_server::provider_client","message":"unresolved placeholder {{stored.token}} in header Authorization"}
```

### Channel Cache

```bash
# Time-to-live for cached channel and category lists, in seconds.
# Default: 86400 (24 hours). Set to 0 to disable caching.
CHANNEL_CACHE_TTL_SECS=86400
```

Channel lists and category lists fetched from upstream providers are cached
in memory to avoid hammering provider APIs on every request. The cache is
invalidated automatically on provider login and logout, so stale data is never
served after a credential change regardless of the TTL.

| Variable | Default | Description |
| -------- | ------- | ----------- |
| `CHANNEL_CACHE_TTL_SECS` | `86400` | Channel/category cache TTL in seconds |

### CORS

```bash
# Comma-separated list of allowed origins.
# Leave unset (or set to *) to allow all origins — suitable for local development only.
# CORS_ORIGINS=https://tv.example.com,https://admin.example.com
```

| Value                          | Behaviour                                                                 |
| ------------------------------ | ------------------------------------------------------------------------- |
| Unset or `*`                   | All origins allowed (permissive). A **production warning** is emitted at startup. |
| Comma-separated origin list    | Restricts `Access-Control-Allow-Origin` to the listed origins only.       |

:::warning
Never leave `CORS_ORIGINS` unset in a production deployment. Set it to the exact origin(s) your frontend is served from, for example:

```bash
CORS_ORIGINS=https://tv.example.com
```
:::

## Complete `.env.example`

```bash
# ─────────────────────────────────────────────────────────
# otvi-server  –  environment configuration
# ─────────────────────────────────────────────────────────

# ── Database ─────────────────────────────────────────────
# SQLite (default):
DATABASE_URL=sqlite://data.db
# PostgreSQL:
# DATABASE_URL=postgres://user:password@localhost:5432/otvi
# MySQL:
# DATABASE_URL=mysql://user:password@localhost:3306/otvi

# ── JWT ──────────────────────────────────────────────────
# Generate with: openssl rand -hex 32
# If unset, a random value is used — tokens are invalidated on every restart.
JWT_SECRET=change_me_to_a_long_random_string

# ── Server ───────────────────────────────────────────────
PORT=3000

# ── Paths ────────────────────────────────────────────────
PROVIDERS_DIR=providers
STATIC_DIR=dist

# ── Logging ──────────────────────────────────────────────
# Tracing filter (see https://docs.rs/tracing-subscriber)
RUST_LOG=otvi_server=info

# Log output format: "text" (default) or "json"
LOG_FORMAT=text

# ── CORS ─────────────────────────────────────────────────
# Comma-separated allowed origins. Leave unset for permissive (dev only).
# CORS_ORIGINS=https://tv.example.com,https://admin.example.com

# ── Rate limiting ─────────────────────────────────────────
# Auth tier (login / register / provider-auth) — brute-force protection.
# RATE_LIMIT_AUTH_BURST=5
# RATE_LIMIT_AUTH_PERIOD_SECS=10

# General tier (all other /api routes) — broad throttling.
# RATE_LIMIT_GENERAL_BURST=20
# RATE_LIMIT_GENERAL_PERIOD_SECS=1

# ── Channel cache ─────────────────────────────────────────
# TTL for cached channel and category lists (seconds). Default: 86400 (24 h).
# CHANNEL_CACHE_TTL_SECS=86400
```

## Server Settings

Some settings can be changed at runtime through the admin API:

| Setting           | Default | Description                                                                                   |
| ----------------- | ------- | --------------------------------------------------------------------------------------------- |
| `signup_disabled` | `false` | When `true`, new user registration is disabled (admin can still create users via the API)     |

These settings are stored in the database and can be managed via:
- The admin dashboard in the web UI
- `GET /api/admin/settings` and `PUT /api/admin/settings` API endpoints

## First-Time Setup

When the server starts with an empty database:

1. The first user to register automatically becomes an **admin**.
2. After the admin account is created, additional users can register (unless signup is disabled).
3. Admins can create users with specific roles and provider access restrictions.

:::tip
For security, disable public signup after creating your initial user accounts by setting `signup_disabled` to `true` in the admin settings.
:::

## Password Policy

All passwords (self-registration, `change-password`, admin-created accounts, admin password reset) must satisfy:

- Minimum **8 characters**
- At least one **uppercase** ASCII letter
- At least one **digit**

The same `validate_password()` function is used consistently across all endpoints, so the policy is enforced everywhere. A descriptive error message is returned when the policy is not met.

:::note
Accounts created by an admin have `must_change_password = true` set automatically. Those accounts receive a `403 Forbidden` response on all API calls (except `POST /api/auth/change-password` and `GET /api/auth/me`) until the user sets a personal password.
:::