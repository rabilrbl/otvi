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

Each `{{uuid}}` occurrence produces a **different** UUID, so you can generate multiple distinct IDs in a single request.

### `{{utcnow}}` — UTC Timestamp

Returns the current UTC timestamp in `YYYYMMDDTHHmmSS` format.

```yaml
body: |
  {
    "timestamp": "{{utcnow}}"
  }
```

### `{{utcdate}}` — UTC Date

Returns the current UTC date in `YYYYMMDD` format.

```yaml
params:
  date: "{{utcdate}}"
```

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

## Template Resolution

### How Resolution Works

The template engine replaces every `{{variable}}` placeholder in a string with the corresponding value from the current context. There are three resolution functions, each with different behaviour for unresolved placeholders:

| Function | Unresolved placeholder behaviour |
| --- | --- |
| `resolve()` | Returns `ResolveResult { rendered, unresolved }` — the rendered string plus a list of placeholder names that could not be substituted |
| `resolve_warn()` | Calls `resolve()` and emits a `tracing::warn!` for every unresolved key. Used by `provider_client.rs` for all outbound requests. |
| `resolve_lossy()` | Silently removes unresolved placeholders (replaces them with an empty string). Legacy behaviour for callers that do not need warnings. |

In production, `resolve_warn()` is used for every outbound API request. Any placeholder that could not be substituted — for example because a previous step failed to extract a value — is logged at `WARN` level:

```
WARN otvi_server::provider_client: unresolved placeholder {{stored.access_token}} in header Authorization
WARN otvi_server::provider_client: unresolved placeholder {{stored.refresh_token}} in body
```

These warnings make misconfigured YAML files easy to spot without crashing the request.

### Resolution Order

When a template is rendered, variables are resolved in the following order:

1. **Built-in variables** (`{{uuid}}`, `{{utcnow}}`, `{{utcdate}}`)
2. **Input variables** (`{{input.X}}`) — from the user's form submission
3. **Extract variables** (`{{extract.X}}`) — from the immediately preceding step
4. **Stored variables** (`{{stored.X}}`) — from persistent session storage

### Usage Locations

Templates can be used in the following YAML fields:

| Location | Example |
| --- | --- |
| Request path | `path: "/channels/{{input.channel_id}}/stream"` |
| Request headers | `Authorization: "Bearer {{stored.access_token}}"` |
| Query parameters | `category: "{{input.category}}"` |
| Request body | `"email": "{{input.email}}"` |
| Proxy headers | `Cookie: "token={{stored.access_token}}"` |
| Proxy cookies | `ssotoken: "{{stored.sso_token}}"` |

## JSONPath Extraction

The `extract` section uses **full JSONPath** expressions to pull values from JSON responses, powered by [`jsonpath-rust`](https://github.com/besok/jsonpath-rust). This supports the complete JSONPath specification — filter expressions, recursive descent, and wildcards — not just simple dot notation.

A simple dot-notation walker is used as a fallback for paths that are not valid JSONPath, preserving compatibility with basic `$.key.subkey` patterns.

### Basic Paths

Navigate into nested objects and arrays using dot notation and index brackets:

```yaml
on_success:
  extract:
    # Simple nested path
    access_token:  "$.data.access_token"

    # Deeper nesting
    user_name:     "$.data.user.display_name"
    avatar_url:    "$.data.user.profile.avatar.url"

    # Array index
    first_item_id: "$.data.items[0].id"
    second_item:   "$.results[1].value"
```

### Filter Expressions

Use `[?(@.field operator value)]` to select array elements that match a condition:

```yaml
on_success:
  extract:
    # ID of the first active item
    active_id:    "$.items[?(@.active == true)].id"

    # Channels with more than 1000 viewers
    popular_name: "$.channels[?(@.viewers > 1000)].name"

    # Item matching a specific string
    admin_token:  "$.users[?(@.role == 'admin')].token"

    # Numeric comparison
    hd_channel:   "$.channels[?(@.resolution >= 1080)].id"
```

Supported filter operators: `==`, `!=`, `<`, `<=`, `>`, `>=`.

### Recursive Descent (`$..`)

Use `$..key` to find a field at any depth in the JSON tree:

```yaml
on_success:
  extract:
    # Find 'token' anywhere in the response, regardless of nesting
    any_token:   "$..token"

    # Find all 'id' fields at any depth
    any_id:      "$..id"

    # Recursive + child — all 'name' fields inside any 'user' object
    user_names:  "$..user.name"
```

This is useful when the provider's API nests values differently across versions or endpoints.

### Wildcards

Use `[*]` to select all elements of an array or all values of an object:

```yaml
on_success:
  extract:
    # All names from an array of objects
    all_names:  "$.data[*].name"

    # All top-level values
    all_values: "$.*"
```

### Combined Expressions

JSONPath expressions can be chained:

```yaml
on_success:
  extract:
    # Recursive descent + filter
    active_user_id: "$..users[?(@.active == true)].id"

    # Wildcard + field
    all_item_ids:   "$.data[*].items[*].id"
```

### Extraction Examples by Use Case

#### Login response — simple token extraction

```yaml
on_success:
  extract:
    access_token:  "$.data.access_token"
    refresh_token: "$.data.refresh_token"
    user_id:       "$.data.user.id"
    user_name:     "$.data.user.display_name"
```

#### Multi-step — OTP flow

```yaml
# Step 1: send OTP
on_success:
  extract:
    request_id: "$.data.request_id"   # stored, used in step 2

# Step 2: verify OTP
request:
  body: |
    {
      "request_id": "{{stored.request_id}}",
      "code": "{{input.otp}}"
    }
on_success:
  extract:
    access_token: "$.data.access_token"
```

#### Find value in an unknown location

```yaml
on_success:
  extract:
    # The API sometimes nests the token differently — find it anywhere
    token: "$..access_token"
```

#### Pick the first matching element from a list

```yaml
on_success:
  extract:
    # The ID of the first active subscription
    subscription_id: "$.subscriptions[?(@.status == 'active')].id"
```
