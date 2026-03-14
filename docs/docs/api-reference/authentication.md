---
sidebar_position: 3
title: Authentication
---

# Provider Authentication API

These endpoints authenticate an OTVI user with a configured TV provider.

## Login to Provider

```text
POST /api/providers/:id/auth/login
```

Starts or continues a provider authentication flow.

### Headers

```text
Authorization: Bearer <jwt_token>
Content-Type: application/json
```

### Request body

`step` is required on every request, including the first one.

```json
{
  "flow_id": "email_password",
  "step": 0,
  "inputs": {
    "email": "user@example.com",
    "password": "secretpassword"
  }
}
```

| Field | Type | Description |
| --- | --- | --- |
| `flow_id` | string | Provider flow identifier |
| `step` | number | Zero-based step index |
| `inputs` | object | Collected field values for the current flow |
| `session_id` | string or null | Present in the shared type for compatibility; currently unused by the server |

### Successful single-step response

```json
{
  "success": true,
  "session_id": null,
  "next_step": null,
  "user_name": "alice",
  "error": null
}
```

### Successful intermediate-step response

Intermediate steps return `success: false` and provide `next_step` details when more user input is required.

```json
{
  "success": false,
  "session_id": null,
  "next_step": {
    "step_index": 1,
    "step_name": "Verify OTP",
    "fields": [
      {
        "key": "otp",
        "label": "Enter Verification Code",
        "field_type": "text",
        "required": true
      }
    ]
  },
  "user_name": null,
  "error": null
}
```

### Error responses

| Status | Condition |
| --- | --- |
| `400` | Invalid step index or malformed request |
| `401` | Missing or invalid JWT token |
| `403` | Provider access denied, password change required, or non-admin attempting a global-provider credential action |
| `404` | Provider or flow not found |

## Check Provider Session

```text
GET /api/providers/:id/auth/check
```

Returns whether the current OTVI user has a stored session for the provider.

```json
{
  "valid": true
}
```

or

```json
{
  "valid": false
}
```

## Logout from Provider

```text
POST /api/providers/:id/auth/logout
```

Clears the stored provider session. If a provider-specific logout request is configured, the server attempts that request before deleting local session state.

### Response

```json
{
  "success": true
}
```

### Error responses

| Status | Condition |
| --- | --- |
| `401` | Missing or invalid JWT token |
| `403` | Provider access denied, password change required, or non-admin attempting a global-provider credential action |
| `404` | Provider not found |
