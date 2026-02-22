---
sidebar_position: 6
title: User Management
---

# User Management API

Endpoints for OTVI user registration, login, and account management.

## Register

```
POST /api/auth/register
```

Creates a new OTVI user account. The **first user** to register automatically becomes an admin.

**Request Body:**

```json
{
  "username": "john",
  "password": "securepassword123"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `username` | string | Unique username |
| `password` | string | Account password |

**Response:** `200 OK`

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": 1,
    "username": "john",
    "role": "admin"
  }
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `400` | Username already taken, or signup is disabled |

:::note
Registration is open by default. Admins can disable signup via the [Admin Settings](./admin#update-settings) endpoint.
:::

## Login

```
POST /api/auth/login
```

Authenticates a user and returns a JWT token.

**Request Body:**

```json
{
  "username": "john",
  "password": "securepassword123"
}
```

**Response:** `200 OK`

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": 1,
    "username": "john",
    "role": "admin"
  }
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `401` | Invalid username or password |

### Token Details

- **Algorithm:** HMAC-SHA256
- **Lifetime:** 24 hours
- **Claims:** `sub` (user ID), `username`, `role`, `exp` (expiration)

## Get Current User

```
GET /api/auth/me
```

Returns information about the currently authenticated user.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response:** `200 OK`

```json
{
  "id": 1,
  "username": "john",
  "role": "admin",
  "must_change_password": false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | number | User ID |
| `username` | string | Username |
| `role` | string | `"admin"` or `"user"` |
| `must_change_password` | boolean | If `true`, user must change password before proceeding |

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `401` | Missing or invalid JWT token |

## Change Password

```
POST /api/auth/change-password
```

Changes the current user's password.

**Headers:**
```
Authorization: Bearer <jwt_token>
Content-Type: application/json
```

**Request Body:**

```json
{
  "current_password": "oldpassword",
  "new_password": "newsecurepassword"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `current_password` | string | Current password for verification |
| `new_password` | string | New password |

**Response:** `200 OK`

```json
{
  "success": true
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `400` | Current password is incorrect |
| `401` | Missing or invalid JWT token |

## Logout

```
POST /api/auth/logout
```

Logs out the current user. Since JWT is stateless, this is a no-op on the server side. The frontend removes the token from `LocalStorage`.

**Response:** `200 OK`

```json
{
  "success": true
}
```
