---
sidebar_position: 5
title: Channels
---

# Channel Configuration

This page covers how to configure channel browsing — listing channels, filtering by category, and mapping provider-specific fields to OTVI's canonical schema.

## Channel List

The channel list endpoint fetches available channels from the provider's API.

```yaml
channels:
  list:
    request:
      method: "GET"
      path: "/v2/channels"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
      params:
        category: "{{input.category}}"
        page: "{{input.page}}"
        per_page: "50"
    response:
      items_path: "$.data.channels"
      logo_base_url: "https://cdn.example.com/logos/"
      mapping:
        id: "$.channel_id"
        name: "$.title"
        logo: "$.images.square"
        category: "$.genre"
        number: "$.lcn"
        description: "$.synopsis"
```

### Request Configuration

| Field | Description |
|-------|-------------|
| `method` | HTTP method (`GET`, `POST`, etc.) |
| `path` | URL path (appended to `defaults.base_url`) |
| `headers` | Request headers (merged with `defaults.headers`) |
| `params` | Query parameters — supports template variables |

The `{{input.category}}` and `{{input.page}}` variables are passed from the frontend when the user filters or paginates.

### Response Configuration

| Field | Description |
|-------|-------------|
| `items_path` | JSONPath to the array of channel objects in the response |
| `logo_base_url` | Optional base URL prepended to relative logo paths |
| `mapping` | Maps provider field names to canonical OTVI field names |

### Field Mapping

The `mapping` section translates provider-specific field names to OTVI's standard channel schema:

| OTVI Field | Description | Example JSONPath |
|-----------|-------------|-----------------|
| `id` | Unique channel identifier | `$.channel_id`, `$.id` |
| `name` | Channel display name | `$.title`, `$.channel_name` |
| `logo` | Channel logo URL or relative path | `$.images.square`, `$.logo_url` |
| `category` | Channel category or genre | `$.genre`, `$.category_name` |
| `number` | Channel number (LCN) | `$.lcn`, `$.channel_number` |
| `description` | Channel description | `$.synopsis`, `$.description` |

Each mapping value is a JSONPath expression evaluated against individual channel objects within the `items_path` array.

### Logo URL Handling

If the provider returns relative logo paths, use `logo_base_url` to form the complete URL:

```yaml
response:
  logo_base_url: "https://cdn.example.com/logos/"
  mapping:
    logo: "$.logo_filename"
```

If the channel's `logo_filename` is `"channel1.png"`, the resulting URL becomes `https://cdn.example.com/logos/channel1.png`.

## Categories

Categories allow users to filter channels. They can be fetched from an API or defined statically.

### API-Based Categories

```yaml
channels:
  categories:
    request:
      method: "GET"
      path: "/v2/channels/categories"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      items_path: "$.data"
      mapping:
        id: "$.id"
        name: "$.name"
```

### Static Categories

For providers that don't have a categories API, define them directly in YAML:

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
      - id: "kids"
        name: "Kids"
      - id: "music"
        name: "Music"
```

## Filtering

The frontend automatically sends category and pagination parameters when configured:

- `{{input.category}}` — selected category ID (empty string if "All")
- `{{input.page}}` — current page number

Your YAML can map these to the provider's expected query parameters:

```yaml
params:
  category: "{{input.category}}"
  page: "{{input.page}}"
  per_page: "50"
```

## Channel API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/providers/:id/channels` | List channels (with optional `?category=` filter) |
| `GET` | `/api/providers/:id/channels/categories` | List categories |

See the [API Reference](../api-reference/channels) for request/response details.
