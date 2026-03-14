---
sidebar_position: 11
title: Admin Guide
---

# Admin Guide

This guide covers OTVI administration — user management, provider access control, and server settings.

## First-Time Setup

When the server starts with an empty database:

1. Navigate to **http://localhost:3000** (or your deployment URL).
2. The setup page appears, prompting you to create the first user.
3. The **first user is automatically an admin**.
4. Log in with the admin credentials.

## User Management

### Password Policy

All passwords — including those set by admins — must satisfy:

- Minimum **8 characters**
- At least one **uppercase** ASCII letter
- At least one **digit**

This policy is enforced consistently across `POST /api/auth/register`,
`POST /api/auth/change-password`, `POST /api/admin/users`, and
`PUT /api/admin/users/:id/password`. A clear error message is returned when
the policy is not met.

### Creating Users

Admins can create new users via the **Admin Dashboard** or the API.

#### Via Web UI

1. Navigate to the Admin page (accessible from the navbar).
2. Fill in the "Create User" form with username, password, and role.
3. Optionally restrict the user to specific providers.

#### Via API

```bash
curl -X POST http://localhost:3000/api/admin/users \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "newuser",
    "password": "Secure1Pass",
    "role": "user",
    "providers": ["streammax"]
  }'
```

:::note
Passwords created via `POST /api/admin/users` are subject to the same password
policy. A `400 Bad Request` is returned if the password does not meet the
requirements.
:::

When an admin creates a user, the account is automatically flagged with
`must_change_password = true`. See [Forced Password Change](#forced-password-change)
below.

### User Roles

| Role    | Permissions                                                              |
| ------- | ------------------------------------------------------------------------ |
| `admin` | Full access: manage users, settings, all providers, global auth          |
| `user`  | Access to allowed providers; per-user provider auth                      |

### Deleting Users

Admins can delete any user except themselves:

```bash
curl -X DELETE http://localhost:3000/api/admin/users/2 \
  -H "Authorization: Bearer <admin_token>"
```

### Password Reset

Reset a user's password and force them to change it on next login:

```bash
curl -X PUT http://localhost:3000/api/admin/users/2/password \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{"password": "TempPass1"}'
```

The user's `must_change_password` flag is set to `true`. They will see the
password-change overlay on their next login.

:::note
The temporary password must still satisfy the password policy (min 8 chars,
≥1 uppercase, ≥1 digit). Choose something the user can communicate securely
to the recipient and ask them to change it immediately.
:::

### Forced Password Change

Whenever a user account is created or has its password reset by an admin, the
`must_change_password` flag is set to `true`. Until the user changes their
password via `POST /api/auth/change-password`:

- `GET /api/providers` and `GET /api/providers/:id` return **`403 Forbidden`**.
- All other provider and channel endpoints are similarly blocked.
- `GET /api/auth/me` and `POST /api/auth/change-password` remain accessible so
  the user can identify themselves and complete the change.

The web UI enforces this automatically — a full-screen password-change overlay
is shown before the user can navigate anywhere else.

Once the password is changed the flag is cleared and all endpoints become
accessible immediately.

## Provider Access Control

By default, all users can access all providers. Admins can restrict access per user.

### Setting Provider Access

```bash
curl -X PUT http://localhost:3000/api/admin/users/2/providers \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{"providers": ["streammax", "example"]}'
```

This restricts user ID 2 to only the `streammax` and `example` providers. They
will not see other providers in their provider list.

### Removing Restrictions

To give a user access to all providers, pass an empty array:

```bash
curl -X PUT http://localhost:3000/api/admin/users/2/providers \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{"providers": []}'
```

## Server Settings

### Disabling Public Signup

To prevent new users from self-registering:

#### Via Web UI

1. Go to the Admin Dashboard.
2. Toggle the "Disable Signup" setting.

#### Via API

```bash
curl -X PUT http://localhost:3000/api/admin/settings \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{"signup_disabled": true}'
```

When signup is disabled:
- The `POST /api/auth/register` endpoint returns a `400` error.
- Admins can still create users via `POST /api/admin/users`.

### Viewing Settings

```bash
curl http://localhost:3000/api/admin/settings \
  -H "Authorization: Bearer <admin_token>"
```

Response:
```json
{
  "signup_disabled": false
}
```

## Provider Authentication Scopes

### Per-User Scope (Default)

Each user authenticates with the provider independently. Users manage their
own provider sessions.

### Global Scope

Only admins can log in/out of global-scoped providers. The session is shared
across all users. Use this for providers where a single account is shared
among all OTVI users.

To configure a provider as global, set the scope in the provider YAML:

```yaml
auth:
  scope: "global"
```

## Monitoring

### Health & Readiness Probes

Two lightweight unauthenticated endpoints are available for monitoring:

| Endpoint       | Description                                                                          |
| -------------- | ------------------------------------------------------------------------------------ |
| `GET /healthz` | **Liveness probe** — returns `200 OK` immediately. Use to detect a crashed process.  |
| `GET /readyz`  | **Readiness probe** — checks database connectivity. Returns `503` if DB is down.     |

```bash
# Liveness — instant 200 OK
curl http://localhost:3000/healthz

# Readiness — checks DB, then returns 200 OK (or 503)
curl http://localhost:3000/readyz
```

Both the `Dockerfile` and `docker-compose.yml` configure a `HEALTHCHECK`
directive pointing at `/healthz` automatically.

### Log Levels

Control verbosity with the `RUST_LOG` environment variable:

```bash
# Standard production logging
RUST_LOG=otvi_server=info

# Debug logging (includes request details)
RUST_LOG=otvi_server=debug

# Trace logging (maximum detail)
RUST_LOG=otvi_server=trace
```

### Structured JSON Logging

For log-aggregation stacks (Grafana Loki, Datadog, AWS CloudWatch), enable
structured JSON output:

```bash
LOG_FORMAT=json
RUST_LOG=otvi_server=info
```

Each log line becomes a single parseable JSON object:

```json
{"timestamp":"2024-01-15T10:23:45Z","level":"INFO","target":"otvi_server","message":"server listening on 0.0.0.0:3000"}
{"timestamp":"2024-01-15T10:23:46Z","level":"WARN","target":"otvi_server::provider_client","message":"unresolved placeholder {{stored.token}} in header Authorization"}
```

The `WARN`-level messages about unresolved template placeholders are
particularly useful for diagnosing misconfigured provider YAML files.

### Provider JSON Schema

OTVI serves a live JSON Schema for provider YAML files:

```
GET /api/schema/provider
```

You can use this in automated validation pipelines or CI to catch YAML
mistakes before deployment:

```bash
# Validate a provider file against the live schema
curl -s http://localhost:3000/api/schema/provider > /tmp/provider-schema.json
npx ajv validate -s /tmp/provider-schema.json -d providers/myprovider.yaml
```
