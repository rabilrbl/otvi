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
4. **Save** — the running server picks up the change within ~300 ms. No restart is required.

No code changes are required — everything is defined declaratively in YAML.

## Hot-Reload

The server **watches the `providers/` directory** for file-system events. Any time you create, modify, or delete a `.yaml` / `.yml` file, the provider map is atomically updated in memory:

```bash
# While the server is running, just edit a provider file:
$EDITOR providers/myprovider.yaml
# → Changes are reflected within ~300 ms, no restart needed.
```

This makes iterating on a provider config fast: capture a new endpoint, add it to the YAML, save, and the change is live immediately.

## VS Code Auto-Complete

Point the [YAML extension](https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml) at the live JSON Schema endpoint for inline validation and auto-complete while editing:

```jsonc
// .vscode/settings.json
{
  "yaml.schemas": {
    "http://localhost:3000/api/schema/provider": "providers/*.yaml"
  }
}
```

This is pre-configured in `.vscode/settings.json` in the repository. The schema is served by `GET /api/schema/provider` and is generated live from the `ProviderConfig` struct via [`schemars`](https://graham.cool/schemars/).

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
```

## Key Concepts

### Template Variables

OTVI supports dynamic request building through template variables:

| Variable        | Description                                                                 |
| --------------- | --------------------------------------------------------------------------- |
| `{{input.X}}`   | Value entered by the user in a form field                                   |
| `{{stored.X}}`  | Value extracted from a previous API response and persisted in the session   |
| `{{extract.X}}` | Value extracted in the immediately preceding authentication step            |
| `{{uuid}}`      | Auto-generated UUID v4 (useful for device IDs, request IDs)                 |
| `{{utcnow}}`    | Current UTC timestamp (`YYYYMMDDTHHmmSS`)                                   |
| `{{utcdate}}`   | Current UTC date (`YYYYMMDD`)                                               |

The template engine logs a **`WARN`-level message** for every placeholder that cannot be resolved, making misconfigured YAMLs easy to spot:

```
WARN otvi_server::provider_client: unresolved placeholder {{stored.access_token}} in header Authorization
```

### Response Extraction

Values are extracted from JSON responses using full **JSONPath** expressions, powered by [`jsonpath-rust`](https://github.com/besok/jsonpath-rust). This supports filter expressions, recursive descent, and wildcards — not just simple dot notation.

#### Simple paths

```yaml
on_success:
  extract:
    access_token: "$.data.access_token"
    user_name:    "$.data.user.display_name"
    first_item:   "$.data.items[0].id"
```

#### Filter expressions

```yaml
on_success:
  extract:
    # Extract the id of the first active item
    active_id: "$.items[?(@.active == true)].id"
    # Extract channels with more than 1000 viewers
    popular:   "$.channels[?(@.viewers > 1000)].name"
```

#### Recursive descent

```yaml
on_success:
  extract:
    # Find 'token' anywhere in the response tree
    any_token: "$..token"
    # Find all 'id' fields at any depth
    all_ids:   "$..id"
```

#### Wildcards

```yaml
on_success:
  extract:
    # All items in the array
    all_names: "$.data[*].name"
```

A simple dot-notation walker is used as a fallback for paths that are not standard JSONPath expressions, preserving compatibility with existing configs.

### Authentication Scopes

| Scope      | Description                                                      |
| ---------- | ---------------------------------------------------------------- |
| `per_user` | Each OTVI user authenticates independently with the provider     |
| `global`   | Admin authenticates once; the session is shared across all users |

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

## Complete Example

See `providers/example.yaml` in the repository for a fully-annotated example configuration covering every available option.