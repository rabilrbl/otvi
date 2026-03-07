---
sidebar_position: 6
title: User Management
---

# User Management API

Endpoints for OTVI user registration, login, and account management.

## Password Policy

All endpoints that accept a password enforce the same policy:

- Minimum **8 characters**
- At least one **uppercase** ASCII letter
- At least one **digit**

A `400 Bad Request` with a descriptive error message is returned when the policy is not satisfied. This applies to registration, change-password, and admin-created or admin-reset passwords alike.

## Register

```
POST /api/auth/register
```

Creates a new OTVI user account. The **first user** to register automatically becomes an admin.

**Request Body:**

```json
{
  "username": "john",
  "password": "Secure1Pass"
}
```

| Field      | Type   | Description                                                   |
| ---------- | ------ | ------------------------------------------------------------- |
| `username` | string | Unique username                                               |
| `password` | string | Account password — must satisfy the [password policy](#password-policy) |

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

| Status | Condition                                                     |
| ------ | ------------------------------------------------------------- |
| `400`  | Username already taken, password fails policy, or signup is disabled |

:::note
Registration is open by default. Admins can disable signup via the [Admin Settings](./admin#update-settings) endpoint. When disabled, this endpoint returns `400` with `"signup is disabled"`.
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
  "password": "Secure1Pass"
}
```

**Response:** `200 OK`

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": 1,
    "username": "john",
    "role": "admin",
    "must_change_password": false
  }
}
```

**Error Responses:**

| Status | Condition                          |
| ------ | ---------------------------------- |
| `401`  | Invalid username or password       |

### Token Details

- **Algorithm:** HMAC-SHA256
- **Lifetime:** 24 hours
- **Claims:** `sub` (user ID), `username`, `role`, `exp` (expiration timestamp)

:::tip
Check the `must_change_password` field in the login response. If `true`, redirect the user to the change-password page immediately — most other API endpoints will return `403 Forbidden` until the password is changed.
:::

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

| Field                  | Type    | Description                                                                             |
| ---------------------- | ------- | --------------------------------------------------------------------------------------- |
| `id`                   | number  | User ID                                                                                 |
| `username`             | string  | Username                                                                                |
| `role`                 | string  | `"admin"` or `"user"`                                                                   |
| `must_change_password` | boolean | If `true`, the user must change their password before accessing provider endpoints      |

**Error Responses:**

| Status | Condition                          |
| ------ | ---------------------------------- |
| `401`  | Missing or invalid JWT token       |

:::note
This endpoint is **exempt** from the `must_change_password` guard and always returns the current user info regardless of the flag value. The frontend uses this on startup to detect whether to show the forced password-change overlay.
:::

## Change Password

```
POST /api/auth/change-password
```

Changes the current user's password. If the account has `must_change_password = true` (set by an admin), successfully changing the password clears that flag and restores full access to all endpoints.

**Headers:**
```
Authorization: Bearer <jwt_token>
Content-Type: application/json
```

**Request Body:**

```json
{
  "current_password": "OldPass1",
  "new_password": "NewSecure2Pass"
}
```

| Field              | Type   | Description                                                                           |
| ------------------ | ------ | ------------------------------------------------------------------------------------- |
| `current_password` | string | The user's current password (required for verification)                               |
| `new_password`     | string | The new password — must satisfy the [password policy](#password-policy)               |

**Response:** `200 OK`

```json
{
  "success": true
}
```

**Error Responses:**

| Status | Condition                                                   |
| ------ | ----------------------------------------------------------- |
| `400`  | Current password is incorrect, or new password fails policy |
| `401`  | Missing or invalid JWT token                                |

:::note
This endpoint is **exempt** from the `must_change_password` guard. Users with the flag set can — and must — call this endpoint to regain access to the rest of the API.
:::

### Forced Password Change Flow

When an admin creates a user or resets a password, the account is locked behind a forced password-change requirement:

```
1. Admin creates user  →  must_change_password = true
2. User logs in        →  receives JWT
3. User calls GET /api/providers  →  403 Forbidden
4. User calls POST /api/auth/change-password (new_password satisfies policy)
5. must_change_password cleared  →  all endpoints accessible
```

## Logout

```
POST /api/auth/logout
```

Logs out the current user. Since JWT is stateless, this is a no-op on the server — the frontend removes the token from `LocalStorage`.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response:** `200 OK`

```json
{
  "success": true
}
```

**Error Responses:**

| Status | Condition                    |
| ------ | ---------------------------- |
| `401`  | Missing or invalid JWT token |