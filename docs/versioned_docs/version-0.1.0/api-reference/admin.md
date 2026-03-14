---
sidebar_position: 7
title: Admin
---

# Admin API

Admin endpoints manage users, provider access, and server settings. All routes require an authenticated admin JWT.

## List Users

```text
GET /api/admin/users
```

Returns `UserInfo[]`.

```json
[
  {
    "id": "3a6d...",
    "username": "admin",
    "role": "admin",
    "providers": [],
    "must_change_password": false
  },
  {
    "id": "d8b5...",
    "username": "viewer",
    "role": "user",
    "providers": ["streammax"],
    "must_change_password": true
  }
]
```

## Create User

```text
POST /api/admin/users
```

```json
{
  "username": "viewer",
  "password": "TempPass1",
  "role": "user",
  "providers": ["streammax", "example"]
}
```

- `providers: []` means unrestricted access to all providers
- newly created users receive `must_change_password: true`

Response:

```json
{
  "id": "d8b5...",
  "username": "viewer",
  "role": "user",
  "providers": ["streammax", "example"],
  "must_change_password": true
}
```

## Delete User

```text
DELETE /api/admin/users/:id
```

Deletes the user plus associated provider sessions. Admins cannot delete their own account.

```json
{
  "success": true
}
```

## Set User Provider Access

```text
PUT /api/admin/users/:id/providers
```

```json
{
  "providers": ["streammax", "example"]
}
```

- pass an empty array to restore unrestricted provider access
- restricted users are blocked not only from `/api/providers`, but also from provider auth, channels, categories, and stream routes

```json
{
  "success": true
}
```

## Reset User Password

```text
PUT /api/admin/users/:id/password
```

```json
{
  "new_password": "TempPass1"
}
```

Successful response:

```json
{
  "success": true
}
```

The reset sets `must_change_password = true`, so the user must call `POST /api/auth/change-password` before using the rest of the API.

## Server Settings

### Get settings

```text
GET /api/admin/settings
```

```json
{
  "signup_disabled": false
}
```

### Update settings

```text
PUT /api/admin/settings
```

```json
{
  "signup_disabled": true
}
```

```json
{
  "success": true
}
```
