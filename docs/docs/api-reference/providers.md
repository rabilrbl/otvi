---
sidebar_position: 2
title: Providers
---

# Providers API

Endpoints for listing and viewing provider details.

## List Providers

```
GET /api/providers
```

Returns all providers accessible to the current user. If the user has a provider allow-list configured by an admin, only those providers are returned.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response:** `200 OK`

```json
[
  {
    "id": "streammax",
    "name": "StreamMax TV",
    "logo": "https://example.com/logo.png"
  },
  {
    "id": "example",
    "name": "Example TV",
    "logo": null
  }
]
```

## Get Provider Details

```
GET /api/providers/:id
```

Returns detailed information about a specific provider, including its authentication flows.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Path Parameters:**

| Parameter | Description |
|-----------|-------------|
| `id` | Provider identifier (from the YAML config) |

**Response:** `200 OK`

```json
{
  "id": "streammax",
  "name": "StreamMax TV",
  "logo": "https://example.com/logo.png",
  "auth_flows": [
    {
      "id": "email_password",
      "name": "Email & Password",
      "fields": [
        {
          "key": "email",
          "label": "Email Address",
          "field_type": "email",
          "required": true
        },
        {
          "key": "password",
          "label": "Password",
          "field_type": "password",
          "required": true
        }
      ]
    },
    {
      "id": "phone_otp",
      "name": "Phone Number",
      "fields": [
        {
          "key": "phone",
          "label": "Phone Number",
          "field_type": "tel",
          "required": true
        }
      ]
    }
  ]
}
```

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `401` | Missing or invalid JWT token |
| `403` | User does not have access to this provider |
| `404` | Provider not found |
