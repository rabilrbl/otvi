---
sidebar_position: 3
title: Authentication
---

# Authentication API

Endpoints for authenticating with TV providers.

## Login to Provider

```
POST /api/providers/:id/auth/login
```

Starts or continues a provider authentication flow. Handles both single-step and multi-step flows.

**Headers:**
```
Authorization: Bearer <jwt_token>
Content-Type: application/json
```

**Path Parameters:**

| Parameter | Description |
|-----------|-------------|
| `id` | Provider identifier |

### Initial Login Request

**Request Body:**

```json
{
  "flow_id": "email_password",
  "inputs": {
    "email": "user@example.com",
    "password": "secretpassword"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `flow_id` | string | Authentication flow identifier |
| `inputs` | object | Key-value pairs matching the flow's input fields |

### Success Response (Single-Step)

**Response:** `200 OK`

```json
{
  "success": true
}
```

### Success Response (Multi-Step — Next Step Required)

**Response:** `200 OK`

```json
{
  "success": true,
  "next_step": {
    "step_index": 1,
    "fields": [
      {
        "key": "otp",
        "label": "Enter Verification Code",
        "field_type": "text",
        "required": true
      }
    ]
  }
}
```

### Continue Multi-Step Login

When a next step is returned, send another request with additional inputs:

**Request Body:**

```json
{
  "flow_id": "phone_otp",
  "step": 1,
  "inputs": {
    "phone": "+1234567890",
    "otp": "123456"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `step` | number | The step index from the `next_step` response |
| `inputs` | object | All inputs including both initial and new values |

### Error Responses

| Status | Condition |
|--------|-----------|
| `400` | Invalid flow ID, missing inputs, or provider API error |
| `401` | Missing or invalid JWT token |
| `403` | Global-scoped provider and user is not admin |
| `404` | Provider not found |

## Check Provider Session

```
GET /api/providers/:id/auth/check
```

Checks if the user has a valid session with the specified provider.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response (authenticated):** `200 OK`

```json
{
  "authenticated": true
}
```

**Response (not authenticated):** `200 OK`

```json
{
  "authenticated": false
}
```

## Logout from Provider

```
POST /api/providers/:id/auth/logout
```

Logs out from a provider, clearing the stored session.

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

If the provider has a configured logout endpoint, the server calls it before clearing the local session.

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `401` | Missing or invalid JWT token |
| `403` | Global-scoped provider and user is not admin |
| `404` | Provider not found |
