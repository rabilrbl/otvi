---
sidebar_position: 1
title: Overview
---

# Provider Guide

Providers are the heart of OTVI. Each provider is defined in a single YAML file that describes how to communicate with a TV provider's API — including authentication, channel listing, and stream playback.

## How It Works

1. **Capture traffic** from the provider's mobile or Android TV app using tools like mitmproxy, Charles Proxy, or HTTP Toolkit.
2. **Create a YAML file** based on the captured API endpoints, headers, and body structures.
3. **Place the file** in the `providers/` directory.
4. **Restart** the server to load the new provider.

No code changes are required — everything is defined declaratively in YAML.

## Provider YAML Structure

A provider YAML file has the following top-level sections:

```yaml
# Provider identity
provider:
  name: "My TV Provider"
  id: "myprovider"
  logo: "https://example.com/logo.png"

# Default HTTP settings for all requests
defaults:
  base_url: "https://api.example.com"
  headers:
    User-Agent: "MyApp/1.0"

# Authentication flows
auth:
  scope: "per_user"        # or "global"
  flows:
    - id: "email_password"
      name: "Email & Password"
      inputs: [...]
      steps: [...]
  logout: ...
  refresh: ...

# Channel browsing
channels:
  list: ...
  categories: ...

# Stream playback
playback:
  stream: ...
  proxy: ...
```

## Key Concepts

### Template Variables

OTVI supports dynamic request building through template variables:

| Variable | Description |
|----------|-------------|
| `{{input.X}}` | Value entered by the user in a form field |
| `{{stored.X}}` | Value extracted from a previous API response and persisted in the session |
| `{{extract.X}}` | Value extracted in the previous authentication step |
| `{{uuid}}` | Auto-generated UUID (useful for device IDs) |
| `{{utcnow}}` | Current UTC timestamp |
| `{{utcdate}}` | Current UTC date |

### Response Extraction

Values are extracted from JSON responses using JSONPath-like dot notation:

```yaml
on_success:
  extract:
    access_token: "$.data.access_token"
    user_name: "$.data.user.display_name"
```

`$.data.access_token` navigates into `{"data": {"access_token": "..."}}`.

### Authentication Scopes

| Scope | Description |
|-------|-------------|
| `per_user` | Each user authenticates independently with the provider |
| `global` | Admin authenticates once, session shared across all users |

## Getting Started

1. Start with the [YAML Schema](./yaml-schema) reference to understand the full configuration format.
2. Learn about the [Template Engine](./template-engine) for dynamic request building.
3. See the [Authentication](./authentication) guide for multi-step auth flows.
4. Configure [Channels](./channels) and [Playback](./playback-and-drm) endpoints.
5. Set up [Stream Proxying](./stream-proxy) for CDN authentication handling.

## Example

Here's a minimal provider configuration:

```yaml
provider:
  name: "Example TV"
  id: "example"
  logo: "https://example.com/logo.png"

defaults:
  base_url: "https://api.example.com"
  headers:
    User-Agent: "ExampleTV/1.0"
    Accept: "application/json"
    Content-Type: "application/json"

auth:
  flows:
    - id: "login"
      name: "Email & Password"
      inputs:
        - key: "email"
          label: "Email"
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
            path: "/auth/login"
            body: |
              {
                "email": "{{input.email}}",
                "password": "{{input.password}}"
              }
          on_success:
            extract:
              access_token: "$.token"

channels:
  list:
    request:
      method: "GET"
      path: "/channels"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      items_path: "$.channels"
      mapping:
        id: "$.id"
        name: "$.name"
        logo: "$.logo"
        category: "$.category"

playback:
  stream:
    request:
      method: "GET"
      path: "/channels/{{input.channel_id}}/stream"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      url: "$.stream_url"
      type: "$.stream_type"
```
