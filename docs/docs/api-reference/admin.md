---
sidebar_position: 7
title: Admin
---

# Admin API

Admin-only endpoints for user and system management. All endpoints require a JWT token with the `admin` role.

## List Users

```
GET /api/admin/users
```

Returns all registered users.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
```

**Response:** `200 OK`

```json
[
  {
    "id": 1,
    "username": "admin",
    "role": "admin"
  },
  {
    "id": 2,
    "username": "john",
    "role": "user"
  }
]
```

## Create User

```
POST /api/admin/users
```

Creates a new user account with a specific role and optional provider access restrictions.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
Content-Type: application/json
```

**Request Body:**

```json
{
  "username": "newuser",
  "password": "securepassword",
  "role": "user",
  "providers": ["streammax", "example"]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `username` | string | Unique username |
| `password` | string | Initial password |
| `role` | string | `"admin"` or `"user"` |
| `providers` | string[] \| null | Allowed provider IDs (null = all providers) |

**Response:** `200 OK`

```json
{
  "id": 3,
  "username": "newuser",
  "role": "user"
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `400` | Username already taken |
| `403` | Not an admin |

## Delete User

```
DELETE /api/admin/users/:id
```

Deletes a user account. Admins cannot delete their own account.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
```

**Path Parameters:**

| Parameter | Description |
|-----------|-------------|
| `id` | User ID to delete |

**Response:** `200 OK`

```json
{
  "success": true
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `400` | Attempting to delete own account |
| `403` | Not an admin |
| `404` | User not found |

## Set User Provider Access

```
PUT /api/admin/users/:id/providers
```

Sets the list of providers a user is allowed to access.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
Content-Type: application/json
```

**Request Body:**

```json
{
  "providers": ["streammax", "example"]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `providers` | string[] | List of allowed provider IDs (empty array = all providers) |

**Response:** `200 OK`

```json
{
  "success": true
}
```

## Reset User Password

```
PUT /api/admin/users/:id/password
```

Resets a user's password and forces them to change it on next login.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
Content-Type: application/json
```

**Request Body:**

```json
{
  "password": "temporarypassword"
}
```

**Response:** `200 OK`

```json
{
  "success": true
}
```

The user's `must_change_password` flag is set to `true`. They will be prompted to change their password on next login.

## Get Settings

```
GET /api/admin/settings
```

Returns current server settings.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
```

**Response:** `200 OK`

```json
{
  "signup_disabled": false
}
```

## Update Settings

```
PUT /api/admin/settings
```

Updates server settings.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
Content-Type: application/json
```

**Request Body:**

```json
{
  "signup_disabled": true
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signup_disabled` | boolean | When `true`, public user registration is disabled |

**Response:** `200 OK`

```json
{
  "success": true
}
```
