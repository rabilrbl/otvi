---
sidebar_position: 1
title: Overview
---

# API Reference

OTVI exposes a REST API for provider interaction, user management, and administration. All responses are JSON.

## Base URL

```
http://localhost:3000/api
```

## Authentication

Most endpoints require a JWT token in the `Authorization` header:

```
Authorization: Bearer <jwt_token>
```

Obtain a token by logging in via `POST /api/auth/login`.

## Endpoint Summary

### Providers

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/providers` | JWT | List all accessible providers |
| `GET` | `/api/providers/:id` | JWT | Get provider details |

### Provider Authentication

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/api/providers/:id/auth/login` | JWT | Login to a provider |
| `GET` | `/api/providers/:id/auth/check` | JWT | Check provider session |
| `POST` | `/api/providers/:id/auth/logout` | JWT | Logout from a provider |

### Channels

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/providers/:id/channels` | JWT | List channels |
| `GET` | `/api/providers/:id/channels/categories` | JWT | List categories |
| `GET` | `/api/providers/:id/channels/:cid/stream` | JWT | Get stream URL |

### Streaming

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/proxy` | None | Proxy stream request |

### User Authentication

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/api/auth/register` | None | Register a new user |
| `POST` | `/api/auth/login` | None | Login (get JWT token) |
| `GET` | `/api/auth/me` | JWT | Get current user info |
| `POST` | `/api/auth/change-password` | JWT | Change password |
| `POST` | `/api/auth/logout` | JWT | Logout (no-op for stateless JWT) |

### Admin

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/api/admin/users` | Admin | List all users |
| `POST` | `/api/admin/users` | Admin | Create a user |
| `DELETE` | `/api/admin/users/:id` | Admin | Delete a user |
| `PUT` | `/api/admin/users/:id/providers` | Admin | Set user's provider access |
| `PUT` | `/api/admin/users/:id/password` | Admin | Reset user password |
| `GET` | `/api/admin/settings` | Admin | Get server settings |
| `PUT` | `/api/admin/settings` | Admin | Update server settings |

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
| `400` | Bad Request — invalid input or parameters |
| `401` | Unauthorized — missing or invalid JWT token |
| `403` | Forbidden — insufficient permissions |
| `404` | Not Found — resource does not exist |
| `500` | Internal Server Error |
