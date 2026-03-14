---
sidebar_position: 4
title: Authentication
---

# Authentication Flows

OTVI supports complex, multi-step authentication flows defined entirely in YAML. This page covers how to configure authentication for your provider.

## Overview

Each provider can define multiple authentication flows (e.g., email + password, phone + OTP, SSO). The frontend displays these as options for the user to choose from.

```yaml
auth:
  scope: "per_user"
  flows:
    - id: "email_password"
      name: "Email & Password"
      # ...
    - id: "phone_otp"
      name: "Phone Number"
      # ...
```

## Authentication Scopes

### `per_user` (Default)

Each OTVI user authenticates independently with the provider. Credentials and sessions are stored per-user.

```yaml
auth:
  scope: "per_user"
```

### `global`

The admin authenticates once with the provider. The session is shared across all users. Only admins can perform login/logout for global-scoped providers.

```yaml
auth:
  scope: "global"
```

## Single-Step Authentication

The simplest flow — a single API call with user credentials.

### Email & Password

```yaml
auth:
  flows:
    - id: "email_password"
      name: "Email & Password"
      inputs:
        - key: "email"
          label: "Email Address"
          type: "email"
          required: true
        - key: "password"
          label: "Password"
          type: "password"
          required: true
      steps:
        - name: "Sign In"
          request:
            method: "POST"
            path: "/v2/auth/login"
            body: |
              {
                "email": "{{input.email}}",
                "password": "{{input.password}}",
                "device_id": "{{uuid}}"
              }
          on_success:
            extract:
              access_token: "$.data.access_token"
              refresh_token: "$.data.refresh_token"
              user_name: "$.data.user.display_name"
              user_id: "$.data.user.id"
```

## Multi-Step Authentication

For flows that require multiple API calls (e.g., phone number → OTP verification), use the `prompt` mechanism to pause between steps and collect additional input.

### Phone + OTP

```yaml
auth:
  flows:
    - id: "phone_otp"
      name: "Phone Number"
      inputs:
        - key: "phone"
          label: "Phone Number"
          type: "tel"
          required: true
      steps:
        # Step 1: Send OTP
        - name: "Send Verification Code"
          request:
            method: "POST"
            path: "/v2/auth/otp/send"
            body: |
              {
                "phone_number": "{{input.phone}}",
                "country_code": "+1"
              }
          on_success:
            extract:
              request_id: "$.data.request_id"
            # Prompt the user for OTP before continuing
            prompt:
              - key: "otp"
                label: "Enter Verification Code"
                type: "text"
                required: true

        # Step 2: Verify OTP
        - name: "Verify Code"
          request:
            method: "POST"
            path: "/v2/auth/otp/verify"
            body: |
              {
                "phone_number": "{{input.phone}}",
                "code": "{{input.otp}}",
                "request_id": "{{stored.request_id}}"
              }
          on_success:
            extract:
              access_token: "$.data.access_token"
              refresh_token: "$.data.refresh_token"
```

### How Multi-Step Works

1. The frontend sends the initial inputs (e.g., phone number) to the server.
2. The server executes **Step 1** and extracts values from the response.
3. If `prompt` is defined in `on_success`, the server returns a **next step** response with the prompt fields.
4. The frontend displays the prompt fields (e.g., OTP input) to the user.
5. The user enters the additional values and submits.
6. The server executes **Step 2** with all accumulated inputs and stored values.
7. On success, the session is established.

## Input Transforms

Input values can be transformed before use in templates. Currently supported:

### `base64`

Encodes the input value as base64 before substitution.

```yaml
inputs:
  - key: "phone"
    label: "Phone Number"
    type: "tel"
    required: true
    transform: "base64"
```

This is useful when providers expect base64-encoded credentials.

## Logout

Define a logout endpoint to clean up the provider session:

```yaml
auth:
  logout:
    request:
      method: "POST"
      path: "/v2/auth/logout"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
```

When a user logs out of a provider:
1. The server calls the logout endpoint (if configured).
2. The provider session is deleted from the database.

## Token Refresh

:::warning
`auth.refresh` is not implemented by the current runtime. Provider files that include it are rejected during validation.
:::

The schema reserves the field for future work, but today you should model token rotation explicitly inside your normal auth flow and store any replacement tokens with `on_success.extract`.

Reserved example shape:

```yaml
auth:
  refresh:
    request:
      method: "POST"
      path: "/v2/auth/token/refresh"
      body: |
        {
          "refresh_token": "{{stored.refresh_token}}"
        }
    on_success:
      extract:
        access_token: "$.data.access_token"
```

## Session Storage

After successful authentication, extracted values are stored in the database:
- `access_token`, `refresh_token`, and other extracted values are persisted.
- Values are available as `{{stored.X}}` in subsequent requests.
- Sessions are scoped based on the `auth.scope` setting.

## Provider Login API

The server exposes the following endpoints for provider authentication:

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/providers/:id/auth/login` | Start or continue authentication |
| `GET` | `/api/providers/:id/auth/check` | Check if session is valid |
| `POST` | `/api/providers/:id/auth/logout` | Clear provider session |

See the [API Reference](../api-reference/authentication) for details on request/response formats.
