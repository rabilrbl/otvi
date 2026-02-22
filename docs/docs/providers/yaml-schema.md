---
sidebar_position: 2
title: YAML Schema
---

# Provider YAML Schema

This page documents the complete YAML schema used to define a provider configuration.

## Top-Level Structure

```yaml
provider:       # Provider identity (required)
defaults:       # Default HTTP settings (required)
auth:           # Authentication configuration (required)
channels:       # Channel browsing configuration (required)
playback:       # Stream playback configuration (required)
```

## `provider` — Identity

```yaml
provider:
  name: "StreamMax TV"         # Display name (required)
  id: "streammax"              # Unique identifier (required)
  logo: "https://..."          # Logo URL (optional)
```

## `defaults` — HTTP Defaults

Default settings applied to every outbound HTTP request.

```yaml
defaults:
  base_url: "https://api.example.com"   # Base URL for all requests (required)
  headers:                                # Default headers (optional)
    User-Agent: "MyApp/1.0"
    Accept: "application/json"
    Content-Type: "application/json"
    X-Custom-Header: "value"
```

:::tip
Copy the `User-Agent` and custom headers from the provider app's actual traffic captures.
:::

## `auth` — Authentication

### Scope

```yaml
auth:
  scope: "per_user"     # "per_user" (default) or "global"
```

| Scope | Behavior |
|-------|----------|
| `per_user` | Each OTVI user logs in to the provider independently |
| `global` | Admin logs in once; the session is shared with all users |

### Flows

An array of authentication flows. Each provider can have multiple flows (e.g., email + password, phone + OTP).

```yaml
auth:
  flows:
    - id: "email_password"         # Unique flow identifier
      name: "Email & Password"     # Display name
      inputs:                       # Fields shown to the user
        - key: "email"
          label: "Email Address"
          type: "email"             # text, email, password, tel
          required: true
        - key: "password"
          label: "Password"
          type: "password"
          required: true
      steps:                        # Authentication steps (executed sequentially)
        - name: "Sign In"
          request: { ... }
          on_success: { ... }
```

### Input Fields

| Property | Type | Description |
|----------|------|-------------|
| `key` | string | Field identifier used in templates as `{{input.key}}` |
| `label` | string | Display label for the form field |
| `type` | string | Input type: `text`, `email`, `password`, `tel` |
| `required` | boolean | Whether the field must be filled |
| `transform` | string | Optional transform applied to the value (e.g., `base64`) |

### Steps

Each step represents an API call in the authentication flow.

```yaml
steps:
  - name: "Step Display Name"
    request:
      method: "POST"                    # HTTP method
      path: "/v2/auth/login"            # URL path (appended to base_url)
      headers:                          # Step-specific headers (merged with defaults)
        X-Step-Header: "value"
      params:                           # Query parameters
        key: "value"
      body: |                           # Request body (supports templates)
        {
          "email": "{{input.email}}",
          "password": "{{input.password}}",
          "device_id": "{{uuid}}"
        }
      body_type: "json"                 # "json" (default) or "form"
    on_success:
      extract:                          # Values to extract from response
        access_token: "$.data.token"
        user_name: "$.data.user.name"
      prompt:                           # Additional fields to show (multi-step)
        - key: "otp"
          label: "Enter OTP"
          type: "text"
          required: true
```

### Logout

```yaml
auth:
  logout:
    request:
      method: "POST"
      path: "/auth/logout"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
```

### Token Refresh

```yaml
auth:
  refresh:
    request:
      method: "POST"
      path: "/auth/refresh"
      body: |
        { "refresh_token": "{{stored.refresh_token}}" }
    on_success:
      extract:
        access_token: "$.data.access_token"
```

## `channels` — Channel Browsing

### Channel List

```yaml
channels:
  list:
    request:
      method: "GET"
      path: "/channels"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
      params:
        category: "{{input.category}}"
        page: "{{input.page}}"
        per_page: "50"
    response:
      items_path: "$.data.channels"      # JSONPath to the channels array
      logo_base_url: "https://..."       # Prepended to relative logo URLs
      mapping:                            # Field mapping to canonical schema
        id: "$.channel_id"
        name: "$.title"
        logo: "$.images.square"
        category: "$.genre"
        number: "$.lcn"
        description: "$.synopsis"
```

#### Response Mapping Fields

| Field | Description |
|-------|-------------|
| `id` | Channel unique identifier |
| `name` | Channel display name |
| `logo` | Channel logo URL |
| `category` | Channel category/genre |
| `number` | Channel number (LCN) |
| `description` | Channel description |

### Categories

Categories can be fetched from an API or defined statically.

#### API-Based Categories

```yaml
channels:
  categories:
    request:
      method: "GET"
      path: "/channels/categories"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      items_path: "$.data"
      mapping:
        id: "$.id"
        name: "$.name"
```

#### Static Categories

```yaml
channels:
  categories:
    static:
      - id: "entertainment"
        name: "Entertainment"
      - id: "movies"
        name: "Movies"
      - id: "sports"
        name: "Sports"
      - id: "news"
        name: "News"
```

## `playback` — Stream Playback

### Stream Endpoint

```yaml
playback:
  stream:
    request:
      method: "GET"
      path: "/channels/{{input.channel_id}}/stream"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      url: "$.data.manifest_url"         # JSONPath to stream URL
      type: "$.data.stream_format"       # JSONPath to type ("hls" or "dash")
      drm:                                # DRM configuration (optional)
        system: "$.data.drm.type"        # "widevine" or "playready"
        license_url: "$.data.drm.license_server_url"
        headers:                          # Headers sent with license requests
          Authorization: "Bearer {{stored.access_token}}"
```

### Stream Proxy

```yaml
playback:
  proxy:
    headers:                              # Headers added to proxied requests
      Authorization: "Bearer {{stored.access_token}}"
    cookies:                              # Static cookies
      auth: "{{stored.access_token}}"
    url_params_to_cookies:                # Query params → cookie mapping
      - param: "hdnts"
        cookie: "hdnts"
    skip_url_param_cookies_for_keys: true # Don't send param cookies for key requests
```

## Complete Example

See `providers/example.yaml` in the repository for a fully-annotated example configuration.
