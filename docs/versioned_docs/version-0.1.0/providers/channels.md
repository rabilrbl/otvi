---
sidebar_position: 5
title: Channels
---

# Channel Configuration

This page covers how to configure channel browsing — listing channels, filtering by category, server-side search, and mapping provider-specific fields to OTVI's canonical schema.

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

| Field     | Description                                                              |
| --------- | ------------------------------------------------------------------------ |
| `method`  | HTTP method (`GET`, `POST`, etc.)                                        |
| `path`    | URL path (appended to `defaults.base_url`)                               |
| `headers` | Request headers (merged with `defaults.headers`)                         |
| `params`  | Query parameters — supports template variables                           |

The `{{input.category}}` and `{{input.page}}` variables are passed from the frontend when the user filters or paginates.

### Response Configuration

| Field           | Description                                                              |
| --------------- | ------------------------------------------------------------------------ |
| `items_path`    | JSONPath to the array of channel objects in the response                 |
| `logo_base_url` | Optional base URL prepended to relative logo paths                       |
| `mapping`       | Maps provider field names to canonical OTVI field names                  |

### Field Mapping

The `mapping` section translates provider-specific field names to OTVI's standard channel schema:

| OTVI Field    | Description                       | Example JSONPath                    |
| ------------- | --------------------------------- | ----------------------------------- |
| `id`          | Unique channel identifier         | `$.channel_id`, `$.id`              |
| `name`        | Channel display name              | `$.title`, `$.channel_name`         |
| `logo`        | Channel logo URL or relative path | `$.images.square`, `$.logo_url`     |
| `category`    | Channel category or genre         | `$.genre`, `$.category_name`        |
| `number`      | Channel number (LCN)              | `$.lcn`, `$.channel_number`         |
| `description` | Channel description               | `$.synopsis`, `$.description`       |

Each mapping value is a JSONPath expression evaluated against individual channel objects within the `items_path` array. Full JSONPath syntax is supported — see the [Template Engine](./template-engine#jsonpath-extraction) page for the complete reference.

### Logo URL Handling

If the provider returns relative logo paths, use `logo_base_url` to form the complete URL:

```yaml
response:
  logo_base_url: "https://cdn.example.com/logos/"
  mapping:
    logo: "$.logo_filename"
```

If the channel's `logo_filename` is `"channel1.png"`, the resulting URL becomes `https://cdn.example.com/logos/channel1.png`.

## Server-Side Search

OTVI applies a **case-insensitive substring search** on channel names server-side before returning results. Clients pass a `?search=<term>` query parameter to the channels endpoint:

```
GET /api/providers/:id/channels?search=sports
```

The search filter is applied **after** channels are fetched from the provider and **before** pagination (`limit`/`offset`) is applied. This means:

- The `total` field in the response reflects the number of channels matching the search term, not the total unfiltered count.
- Pagination controls should be based on `total`, not on a cached total.

:::note
Search is handled entirely by the OTVI server — no changes to the provider YAML are needed to enable it. The `?search=` parameter is a built-in feature of the channels API.
:::

### Combining Search and Category Filter

Search and category filtering can be combined freely:

```
GET /api/providers/:id/channels?category=sports&search=live&limit=20&offset=0
```

The evaluation order is:

1. Fetch all channels from the provider API (optionally passing category/pagination params if your YAML forwards them).
2. Apply `?category=` filter.
3. Apply `?search=` substring filter on channel names.
4. Apply `?limit=` and `?offset=` pagination.
5. Return `{ channels: [...], total: N }`.

## Pagination

The channels API supports limit/offset pagination via query parameters:

| Parameter | Description                                       |
| --------- | ------------------------------------------------- |
| `limit`   | Maximum number of channels to return              |
| `offset`  | Zero-based index of the first channel to return   |

The response always includes a `total` field with the number of channels matching the current filters (after search and category filtering), so clients can compute the total number of pages:

```
total_pages = ceil(total / limit)
```

### Example

```
# Page 1 (channels 1–20)
GET /api/providers/streammax/channels?limit=20&offset=0

# Page 2 (channels 21–40)
GET /api/providers/streammax/channels?limit=20&offset=20

# Page 3 of "Sports" results matching "live"
GET /api/providers/streammax/channels?category=sports&search=live&limit=20&offset=40
```

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

### Mixing API and Static Categories

If you want to supplement an API response with additional static categories, or fall back to static categories when the API fails, use `static_categories` alongside the API config:

```yaml
channels:
  categories:
    static:
      - id: "all"
        name: "All Channels"
```

## Filtering

The frontend automatically sends category and pagination parameters when configured. Your YAML maps these to the provider's expected query parameters:

```yaml
params:
  category: "{{input.category}}"
  page:     "{{input.page}}"
  per_page: "50"
```

- `{{input.category}}` — the selected category ID (empty string if "All")
- `{{input.page}}` — the current page number (for providers that use page-based pagination)

:::tip
For providers that use cursor-based or page-number pagination internally, you can still use OTVI's offset-based pagination on top. Fetch a full page from the provider and let OTVI slice it with `limit`/`offset`.
:::

## Channel API Endpoints

| Method | Path                                             | Description                                      |
| ------ | ------------------------------------------------ | ------------------------------------------------ |
| `GET`  | `/api/providers/:id/channels`                    | List channels — supports `category`, `search`, `limit`, `offset` |
| `GET`  | `/api/providers/:id/channels/categories`         | List categories                                  |
| `GET`  | `/api/providers/:id/channels/:cid/stream`        | Get stream URL and optional DRM info             |

See the [Channels API Reference](../api-reference/channels) for full request/response details, including the `total` field and pagination examples.