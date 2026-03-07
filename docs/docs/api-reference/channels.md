---
sidebar_position: 4
title: Channels
---

# Channels API

Endpoints for browsing channels, listing categories, and obtaining stream URLs.

## List Channels

```
GET /api/providers/:id/channels
```

Returns channels available from the specified provider. Results can be filtered by category and/or a search term, and support limit/offset pagination.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Path Parameters:**

| Parameter | Description      |
| --------- | ---------------- |
| `id`      | Provider identifier |

**Query Parameters:**

| Parameter  | Type   | Required | Description                                                                              |
| ---------- | ------ | -------- | ---------------------------------------------------------------------------------------- |
| `category` | string | No       | Filter by category ID. Omit or pass an empty string to return all categories.            |
| `search`   | string | No       | Case-insensitive substring match on channel names, evaluated **server-side before pagination**. |
| `limit`    | number | No       | Maximum number of channels to return. Omit for the provider's default page size.         |
| `offset`   | number | No       | Zero-based index of the first channel to return (default: `0`).                          |

:::tip
`search` is applied before `limit`/`offset`, so the `total` field in the response
always reflects the number of channels that matched the search (and category) filter,
not the full unfiltered count. Use `total` to render pagination controls correctly.
:::

**Example Requests:**

```bash
# All channels (no filters)
GET /api/providers/streammax/channels

# First page of "Sports" channels
GET /api/providers/streammax/channels?category=sports&limit=20&offset=0

# Search for "news" across all categories, second page
GET /api/providers/streammax/channels?search=news&limit=20&offset=20

# Combine search and category filter
GET /api/providers/streammax/channels?category=news&search=24&limit=10&offset=0
```

**Response:** `200 OK`

```json
{
  "channels": [
    {
      "id": "ch001",
      "name": "News 24/7",
      "logo": "https://cdn.example.com/logos/news247.png",
      "category": "news",
      "number": 1,
      "description": "24-hour news channel"
    },
    {
      "id": "ch002",
      "name": "Sports Live",
      "logo": "https://cdn.example.com/logos/sportslive.png",
      "category": "sports",
      "number": 2,
      "description": "Live sports coverage"
    }
  ],
  "total": 142
}
```

### Response Fields

| Field      | Type            | Description                                                                                          |
| ---------- | --------------- | ---------------------------------------------------------------------------------------------------- |
| `channels` | Channel[]       | Array of channel objects for the current page                                                        |
| `total`    | number          | Total number of channels matching the applied filters (use this to compute pagination page count)    |

### Channel Object

| Field         | Type            | Description                          |
| ------------- | --------------- | ------------------------------------ |
| `id`          | string          | Unique channel identifier            |
| `name`        | string          | Channel display name                 |
| `logo`        | string \| null  | Channel logo URL                     |
| `category`    | string \| null  | Channel category identifier          |
| `number`      | number \| null  | Channel number (LCN)                 |
| `description` | string \| null  | Channel description                  |

**Error Responses:**

| Status | Condition                                                   |
| ------ | ----------------------------------------------------------- |
| `400`  | Provider API returned an error                              |
| `401`  | Missing or invalid JWT token                               |
| `403`  | User has `must_change_password = true` (password change required before accessing this endpoint) |
| `404`  | Provider not found or no active session                     |

## Pagination

Use `limit` and `offset` together with the `total` field to build pagination controls:

```
Total channels matching filter: total = 142
Page size:                       limit  = 20
Current page (0-based):          page   = 3

offset = page × limit = 60

GET /api/providers/streammax/channels?limit=20&offset=60
```

**Example pagination loop:**

```js
const limit  = 20;
let   offset = 0;

while (true) {
  const { channels, total } = await fetchChannels({ limit, offset });
  renderPage(channels);

  offset += limit;
  if (offset >= total) break;
}
```

## List Categories

```
GET /api/providers/:id/channels/categories
```

Returns available channel categories. Categories may be fetched from the provider's API or defined statically in the provider YAML.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Path Parameters:**

| Parameter | Description         |
| --------- | ------------------- |
| `id`      | Provider identifier |

**Response:** `200 OK`

```json
{
  "categories": [
    {
      "id": "entertainment",
      "name": "Entertainment"
    },
    {
      "id": "movies",
      "name": "Movies"
    },
    {
      "id": "sports",
      "name": "Sports"
    },
    {
      "id": "news",
      "name": "News"
    }
  ]
}
```

### Category Object

| Field  | Type   | Description              |
| ------ | ------ | ------------------------ |
| `id`   | string | Category identifier      |
| `name` | string | Category display name    |

**Error Responses:**

| Status | Condition                                                   |
| ------ | ----------------------------------------------------------- |
| `401`  | Missing or invalid JWT token                               |
| `403`  | User has `must_change_password = true`                      |
| `404`  | Provider not found                                          |