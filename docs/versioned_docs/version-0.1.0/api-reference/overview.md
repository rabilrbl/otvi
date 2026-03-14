---
sidebar_position: 1
title: Overview
---

# API Reference

OTVI exposes a REST API for provider interaction, user management, and administration. All responses are JSON.

## Base URL

```
http://localhost:3000
```

API routes are prefixed with `/api`. Infrastructure routes (`/healthz`, `/readyz`) have no prefix.

## Authentication

Most endpoints require a JWT token in the `Authorization` header:

```
Authorization: Bearer <jwt_token>
```

Obtain a token by logging in via `POST /api/auth/login`.

## Infrastructure Endpoints

These endpoints require no authentication and are suitable for health monitoring, orchestrators, and YAML tooling.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/healthz` | **Liveness probe** — returns `200 OK` instantly |
| `GET` | `/readyz` | **Readiness probe** — checks database connectivity before responding |
| `GET` | `/api/schema/provider` | **JSON Schema** for provider YAML files (for VS Code / CI validation) |
| `GET` | `/api/docs/` | **Swagger UI** — interactive API explorer |
| `GET` | `/api/docs/openapi.json` | **OpenAPI document** — raw JSON spec consumed by the Swagger UI |

### `/healthz`

Returns `200 OK` with no body as long as the process is running. Use this as a liveness probe in Docker, Kubernetes, or any process supervisor.

```bash
curl http://localhost:3000/healthz
# HTTP 200
```

### `/readyz`

Returns `200 OK` once the database is reachable. Returns `503 Service Unavailable` if the database connection fails. Use this as a readiness probe to delay traffic until the server is fully initialised.

```bash
curl http://localhost:3000/readyz
# HTTP 200  (or 503 if DB is unavailable)
```

### `/api/docs/`

Opens the Swagger UI — an interactive browser-based API explorer generated from the OpenAPI document. Use it to browse all endpoints, inspect request/response schemas, and try calls directly from the browser.

```
http://localhost:3000/api/docs/
```

The raw OpenAPI JSON document is available at `/api/docs/openapi.json` and can be imported into tools such as Postman or Insomnia.

### `/api/schema/provider`

Returns a live [JSON Schema](https://json-schema.org/) generated from the `ProviderConfig` struct via [`schemars`](https://graham.cool/schemars/). Use it to enable inline validation and auto-complete in VS Code:

```jsonc
// .vscode/settings.json
{
  "yaml.schemas": {
    "http://localhost:3000/api/schema/provider": "providers/*.yaml"
  }
}
```

## Endpoint Summary

### Providers

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/providers` | JWT | List all accessible providers |
| `GET` | `/api/providers/:id` | JWT | Get provider details and auth flows |

:::note
Both provider endpoints enforce the `must_change_password` guard. If the authenticated user has `must_change_password = true`, these endpoints return `403 Forbidden` until the user changes their password via `POST /api/auth/change-password`.
:::

### Provider Authentication

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/api/providers/:id/auth/login` | JWT | Login to a provider (single- or multi-step) |
| `GET` | `/api/providers/:id/auth/check` | JWT | Check whether a provider session is active |
| `POST` | `/api/providers/:id/auth/logout` | JWT | Logout from a provider |

### Channels

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/providers/:id/channels` | JWT | List channels (supports search and pagination) |
| `GET` | `/api/providers/:id/channels/categories` | JWT | List channel categories |
| `GET` | `/api/providers/:id/channels/:cid/stream` | JWT | Get stream URL and optional DRM info |

#### Channel List Query Parameters

| Parameter  | Type   | Description                                                              |
| ---------- | ------ | ------------------------------------------------------------------------ |
| `category` | string | Filter by category ID                                                    |
| `search`   | string | Case-insensitive substring search on channel names (server-side)         |
| `limit`    | number | Maximum number of channels to return                                     |
| `offset`   | number | Zero-based offset for pagination                                         |

`search` is evaluated **before** pagination, so the `total` field in the response reflects the number of matching channels, not the total unfiltered count.

### Streaming

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/proxy` | None | Proxy a stream request (manifest, segment, or key file) |

### User Authentication

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/api/auth/register` | None | Register a new user (first user becomes admin) |
| `POST` | `/api/auth/login` | None | Login and receive a JWT token |
| `GET` | `/api/auth/me` | JWT | Get current user info (including `must_change_password`) |
| `POST` | `/api/auth/change-password` | JWT | Change password; clears the `must_change_password` flag |
| `POST` | `/api/auth/logout` | JWT | Logout (no-op — client drops the token) |

### Admin

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/admin/users` | Admin | List all users |
| `POST` | `/api/admin/users` | Admin | Create a user (sets `must_change_password = true`) |
| `DELETE` | `/api/admin/users/:id` | Admin | Delete a user |
| `PUT` | `/api/admin/users/:id/providers` | Admin | Set a user's provider allow-list |
| `PUT` | `/api/admin/users/:id/password` | Admin | Reset a user's password (sets `must_change_password = true`) |
| `GET` | `/api/admin/settings` | Admin | Get server settings |
| `PUT` | `/api/admin/settings` | Admin | Update server settings |

## Password Policy

All endpoints that accept a password (`/api/auth/register`, `/api/auth/change-password`, `POST /api/admin/users`, `PUT /api/admin/users/:id/password`) enforce the same policy:

- Minimum **8 characters**
- At least one **uppercase** ASCII letter
- At least one **digit**

A `400 Bad Request` with a descriptive error message is returned when the policy is not satisfied.

## `must_change_password` Enforcement

When an admin creates a user or resets a user's password, `must_change_password` is set to `true` on the account. Until the user changes their password:

- `GET /api/providers` → `403 Forbidden`
- `GET /api/providers/:id` → `403 Forbidden`
- All provider, channel, and stream endpoints → `403 Forbidden`

The following endpoints remain accessible regardless:

- `GET /api/auth/me` — so the frontend can detect the flag
- `POST /api/auth/change-password` — so the user can clear the flag

Once the user changes their password the flag is cleared and all endpoints become accessible immediately.

## Error Responses

All errors return JSON with a consistent format:

```json
{
  "error": "Error description message"
}
```

### HTTP Status Codes

| Code | Description |
|------|-------------|
| `400` | Bad Request — invalid input, failed password policy, or provider API error |
| `401` | Unauthorized — missing or invalid JWT token |
| `403` | Forbidden — insufficient permissions, or `must_change_password` is set |
| `404` | Not Found — resource does not exist |
| `500` | Internal Server Error |
| `503` | Service Unavailable — database not reachable (readiness probe only) |
