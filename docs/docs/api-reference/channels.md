---
sidebar_position: 4
title: Channels
---

# Channels API

Endpoints for browsing channels and categories.

## List Channels

```
GET /api/providers/:id/channels
```

Returns the list of channels available from the specified provider.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Path Parameters:**

| Parameter | Description |
|-----------|-------------|
| `id` | Provider identifier |

**Query Parameters:**

| Parameter | Description |
|-----------|-------------|
| `category` | Filter by category ID (optional) |

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
  ]
}
```

### Channel Object

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique channel identifier |
| `name` | string | Channel display name |
| `logo` | string \| null | Channel logo URL |
| `category` | string \| null | Channel category |
| `number` | number \| null | Channel number (LCN) |
| `description` | string \| null | Channel description |

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `400` | Provider API returned an error |
| `401` | Missing or invalid JWT token |
| `404` | Provider not found or no active session |

## List Categories

```
GET /api/providers/:id/channels/categories
```

Returns available channel categories. Categories may come from the provider's API or be statically defined in the YAML config.

**Headers:**
```
Authorization: Bearer <jwt_token>
```

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

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Category identifier |
| `name` | string | Category display name |

**Error Responses:**

| Status | Condition |
|--------|-----------|
| `401` | Missing or invalid JWT token |
| `404` | Provider not found |
