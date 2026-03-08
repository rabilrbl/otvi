---
sidebar_position: 7
title: Admin
---

# Admin API

Admin-only endpoints for user and system management. All endpoints require a JWT token with the `admin` role.

## Password Policy

All passwords set through admin endpoints must satisfy:

- Minimum **8 characters**
- At least one **uppercase** ASCII letter
- At least one **digit**

A `400 Bad Request` with a descriptive error message is returned when the policy is not met. This applies to both `POST /api/admin/users` (initial password) and `PUT /api/admin/users/:id/password` (password reset).

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
    "role": "admin",
    "must_change_password": false
  },
  {
    "id": 2,
    "username": "john",
    "role": "user",
    "must_change_password": true
  }
]
```

### User Object

| Field                  | Type    | Description                                                                           |
| ---------------------- | ------- | ------------------------------------------------------------------------------------- |
| `id`                   | number  | User ID                                                                               |
| `username`             | string  | Username                                                                              |
| `role`                 | string  | `"admin"` or `"user"`                                                                 |
| `must_change_password` | boolean | If `true`, the user must change their password before accessing provider endpoints    |

**Error Responses:**

| Status | Condition        |
| ------ | ---------------- |
| `403`  | Not an admin     |

## Create User

```
POST /api/admin/users
```

Creates a new user account with a specific role and optional provider access restrictions.

The created account automatically has `must_change_password = true` set. The user will be prompted to change their password on first login before they can access any provider or channel endpoints.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
Content-Type: application/json
```

**Request Body:**

```json
{
  "username": "newuser",
  "password": "Secure1Pass",
  "role": "user",
  "providers": ["streammax", "example"]
}
```

| Field       | Type            | Description                                                                                       |
| ----------- | --------------- | ------------------------------------------------------------------------------------------------- |
| `username`  | string          | Unique username                                                                                   |
| `password`  | string          | Initial password — must satisfy the [password policy](#password-policy)                           |
| `role`      | string          | `"admin"` or `"user"`                                                                             |
| `providers` | string[] \| null | Allowed provider IDs. `null` or omit to grant access to all providers. Empty array = all providers. |

**Response:** `200 OK`

```json
{
  "id": 3,
  "username": "newuser",
  "role": "user",
  "must_change_password": true
}
```

**Error Responses:**

| Status | Condition                                          |
| ------ | -------------------------------------------------- |
| `400`  | Username already taken, or password fails policy   |
| `403`  | Not an admin                                       |

:::note
The initial password you set here is temporary. The user will be required to change it on their first login. Communicate the temporary password to the user through a secure channel and ask them to change it immediately.
:::

## Delete User

```
DELETE /api/admin/users/:id
```

Deletes a user account and all associated provider sessions. Admins cannot delete their own account.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
```

**Path Parameters:**

| Parameter | Description        |
| --------- | ------------------ |
| `id`      | User ID to delete  |

**Response:** `200 OK`

```json
{
  "success": true
}
```

**Error Responses:**

| Status | Condition                              |
| ------ | -------------------------------------- |
| `400`  | Attempting to delete your own account  |
| `403`  | Not an admin                           |
| `404`  | User not found                         |

## Set User Provider Access

```
PUT /api/admin/users/:id/providers
```

Sets the list of providers a user is allowed to access. Users not in this list will not see restricted providers in the provider list and cannot authenticate with them.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
Content-Type: application/json
```

**Path Parameters:**

| Parameter | Description     |
| --------- | --------------- |
| `id`      | Target user ID  |

**Request Body:**

```json
{
  "providers": ["streammax", "example"]
}
```

| Field       | Type     | Description                                                                             |
| ----------- | -------- | --------------------------------------------------------------------------------------- |
| `providers` | string[] | List of allowed provider IDs. Pass an **empty array** to grant access to all providers. |

**Response:** `200 OK`

```json
{
  "success": true
}
```

**Error Responses:**

| Status | Condition     |
| ------ | ------------- |
| `403`  | Not an admin  |
| `404`  | User not found |

### Removing All Restrictions

To restore full provider access for a user, pass an empty array:

```bash
curl -X PUT http://localhost:3000/api/admin/users/2/providers \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{"providers": []}'
```

## Reset User Password

```
PUT /api/admin/users/:id/password
```

Resets a user's password and sets `must_change_password = true`. The user will see a password-change overlay on their next login and cannot access provider or channel endpoints until they set a new personal password.

**Headers:**
```
Authorization: Bearer <admin_jwt_token>
Content-Type: application/json
```

**Path Parameters:**

| Parameter | Description     |
| --------- | --------------- |
| `id`      | Target user ID  |

**Request Body:**

```json
{
  "password": "TempPass1"
}
```

| Field      | Type   | Description                                                               |
| ---------- | ------ | ------------------------------------------------------------------------- |
| `password` | string | Temporary password — must satisfy the [password policy](#password-policy) |

**Response:** `200 OK`

```json
{
  "success": true
}
```

**Error Responses:**

| Status | Condition                            |
| ------ | ------------------------------------ |
| `400`  | Password fails policy                |
| `403`  | Not an admin                         |
| `404`  | User not found                       |

:::note
The `must_change_password` flag is set automatically. You do not need to pass it explicitly. The user must call `POST /api/auth/change-password` with a new password that satisfies the policy before they can access any provider endpoints again.
:::

### Forced Password Change Flow

```
Admin calls PUT /api/admin/users/2/password
    → password stored (hashed)
    → must_change_password = true

User logs in (POST /api/auth/login)
    → JWT issued

User calls GET /api/providers
    → 403 Forbidden  (must_change_password guard)

User calls POST /api/auth/change-password
    → new password validated against policy
    → must_change_password cleared in DB

User calls GET /api/providers
    → 200 OK  (access restored)
```

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

| Field             | Type    | Description                                                                                   |
| ----------------- | ------- | --------------------------------------------------------------------------------------------- |
| `signup_disabled` | boolean | When `true`, `POST /api/auth/register` is disabled. Admins can still create users via the API. |

**Error Responses:**

| Status | Condition     |
| ------ | ------------- |
| `403`  | Not an admin  |

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

| Field             | Type    | Description                                                           |
| ----------------- | ------- | --------------------------------------------------------------------- |
| `signup_disabled` | boolean | When `true`, public user registration via `POST /api/auth/register` is disabled |

**Response:** `200 OK`

```json
{
  "success": true
}
```

**Error Responses:**

| Status | Condition     |
| ------ | ------------- |
| `403`  | Not an admin  |

:::tip
Disable public signup after creating your initial set of user accounts to prevent unauthorised registration:

```bash
curl -X PUT http://localhost:3000/api/admin/settings \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{"signup_disabled": true}'
```
:::