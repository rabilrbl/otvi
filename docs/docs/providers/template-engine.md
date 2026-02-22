---
sidebar_position: 3
title: Template Engine
---

# Template Engine

OTVI includes a powerful template engine that enables dynamic request building. Template variables can be used in request URLs, headers, query parameters, and body content.

## Template Syntax

Templates use double curly braces: `{{variable_name}}`.

```yaml
body: |
  {
    "email": "{{input.email}}",
    "device_id": "{{uuid}}",
    "token": "{{stored.access_token}}"
  }
```

## Variable Types

### `{{input.X}}` — User Input

Values entered by the user in form fields. The `X` corresponds to the `key` defined in the flow's `inputs` or a prompt's fields.

```yaml
inputs:
  - key: "email"         # Referenced as {{input.email}}
    label: "Email"
    type: "email"
    required: true
  - key: "password"      # Referenced as {{input.password}}
    label: "Password"
    type: "password"
    required: true
```

### `{{stored.X}}` — Session Storage

Values extracted from previous API responses and persisted in the session. These survive across requests and are available for the entire session lifetime.

```yaml
# Extracted in step 1
on_success:
  extract:
    access_token: "$.data.token"    # Stored as "access_token"

# Used in subsequent requests
headers:
  Authorization: "Bearer {{stored.access_token}}"
```

### `{{extract.X}}` — Previous Step Values

Values extracted in the **immediately preceding** authentication step. Useful in multi-step flows where intermediate values need to be passed between steps.

```yaml
steps:
  - name: "Send OTP"
    request: ...
    on_success:
      extract:
        request_id: "$.data.request_id"    # Available as {{extract.request_id}}

  - name: "Verify OTP"
    request:
      method: "POST"
      path: "/auth/verify"
      body: |
        {
          "request_id": "{{extract.request_id}}",
          "code": "{{input.otp}}"
        }
```

:::note
`{{extract.X}}` values are only available in the step immediately following the extraction. For values that need to persist across multiple steps, use `{{stored.X}}` instead.
:::

### `{{uuid}}` — Auto-Generated UUID

Generates a new UUID v4 each time the template is rendered. Useful for device IDs, request IDs, and other unique identifiers.

```yaml
body: |
  {
    "device_id": "{{uuid}}",
    "session_id": "{{uuid}}"
  }
```

### `{{utcnow}}` — UTC Timestamp

Returns the current UTC timestamp.

### `{{utcdate}}` — UTC Date

Returns the current UTC date.

## Input Transforms

Input values can be transformed before being inserted into templates. Transforms are specified in the input field definition.

### Base64 Encoding

```yaml
inputs:
  - key: "phone"
    label: "Phone Number"
    type: "tel"
    required: true
    transform: "base64"     # Value is base64-encoded before use
```

When the user enters `+1234567890`, the template variable `{{input.phone}}` resolves to the base64-encoded version of that string.

## JSONPath Extraction

The `extract` section uses JSONPath-like dot notation to pull values from JSON responses.

### Syntax

Paths start with `$` (root) and use dots to navigate:

```
$.data.token          → {"data": {"token": "abc"}}     → "abc"
$.data.user.name      → {"data": {"user": {"name": "Jo"}}} → "Jo"
$.data.items[0].id    → {"data": {"items": [{"id": 1}]}}  → 1
```

### Examples

```yaml
on_success:
  extract:
    # Simple path
    access_token: "$.data.access_token"

    # Nested path
    user_name: "$.data.user.display_name"

    # Deep nesting
    avatar_url: "$.data.user.profile.avatar.url"
```

## Template Resolution Order

When a template is rendered, variables are resolved in the following order:

1. **Built-in variables** (`{{uuid}}`, `{{utcnow}}`, `{{utcdate}}`)
2. **Input variables** (`{{input.X}}`) — from user form submission
3. **Extract variables** (`{{extract.X}}`) — from previous step
4. **Stored variables** (`{{stored.X}}`) — from session storage

## Usage Locations

Templates can be used in the following YAML fields:

| Location | Example |
|----------|---------|
| Request path | `path: "/channels/{{input.channel_id}}/stream"` |
| Request headers | `Authorization: "Bearer {{stored.access_token}}"` |
| Query parameters | `category: "{{input.category}}"` |
| Request body | `"email": "{{input.email}}"` |
| Proxy headers | `Cookie: "token={{stored.access_token}}"` |
