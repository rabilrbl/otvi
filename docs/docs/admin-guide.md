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
    "password": "initialpassword",
    "role": "user",
    "providers": ["streammax"]
  }'
```

### User Roles

| Role | Permissions |
|------|-------------|
| `admin` | Full access: manage users, settings, all providers, global auth |
| `user` | Access allowed providers, per-user provider auth |

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
  -d '{"password": "temporarypassword"}'
```

The user will see a password-change overlay on their next login.

## Provider Access Control

By default, all users can access all providers. Admins can restrict access per user.

### Setting Provider Access

```bash
curl -X PUT http://localhost:3000/api/admin/users/2/providers \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{"providers": ["streammax", "example"]}'
```

This restricts user ID 2 to only the `streammax` and `example` providers. They will not see other providers in their provider list.

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

Each user authenticates with the provider independently. Users manage their own provider sessions.

### Global Scope

Only admins can log in/out of global-scoped providers. The session is shared across all users. Use this for providers where a single account is shared among all OTVI users.

To configure a provider as global, set the scope in the provider YAML:

```yaml
auth:
  scope: "global"
```

## Monitoring

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

### Health Check

Use the `/api/auth/me` endpoint as a basic health check (returns `401` if the server is running but no token is provided):

```bash
curl -o /dev/null -s -w "%{http_code}" http://localhost:3000/api/auth/me
# Returns 401 if server is healthy
```
