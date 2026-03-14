---
sidebar_position: 6
title: User Management
---

# User Management API

These endpoints manage OTVI application users, independent of provider-specific sessions.

## Password Policy

All accepted passwords must contain:

- at least 8 characters
- at least one uppercase ASCII letter
- at least one digit

The same policy is enforced for registration, password change, admin-created users, and admin password resets.

## Register

```text
POST /api/auth/register
```

Creates a new user and returns a JWT plus the created user object.

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": "d8b5d2a9-...",
    "username": "john",
    "role": "admin",
    "providers": [],
    "must_change_password": false
  }
}
```

- `id` is a UUID string
- `providers: []` means unrestricted access to all loaded providers
- the first registered user becomes `admin`

## Login

```text
POST /api/auth/login
```

Authenticates a user and returns:

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": "d8b5d2a9-...",
    "username": "john",
    "role": "admin",
    "providers": [],
    "must_change_password": false
  }
}
```

`must_change_password` is embedded in the returned user state so the frontend can immediately show the forced password-change overlay when needed.

## Get Current User

```text
GET /api/auth/me
```

Returns the current user object.

```json
{
  "id": "d8b5d2a9-...",
  "username": "john",
  "role": "admin",
  "providers": [],
  "must_change_password": false
}
```

This endpoint is exempt from the `must_change_password` guard so the frontend can discover that state during boot.

## Change Password

```text
POST /api/auth/change-password
```

Request body:

```json
{
  "current_password": "OldPass1",
  "new_password": "NewSecure2Pass"
}
```

Successful response returns a fresh JWT and refreshed user payload:

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": "d8b5d2a9-...",
    "username": "john",
    "role": "user",
    "providers": [],
    "must_change_password": false
  }
}
```

When `must_change_password` was set, a successful password change clears the flag immediately.

## Logout

```text
POST /api/auth/logout
```

The server returns success and the frontend is expected to clear `otvi_jwt` locally.

```json
{
  "success": true
}
```
