---
sidebar_position: 7
title: Stream Proxy
---

# Stream Proxy

OTVI includes a built-in stream proxy that handles CORS restrictions, CDN authentication, and header injection for HLS/DASH streams.

## Why a Proxy?

Many TV providers use CDN authentication mechanisms (e.g., Akamai signed URLs, cookie-based tokens) that require:

- **Custom headers** on every segment request
- **Cookies** derived from URL parameters
- **CORS bypass** since browsers block cross-origin media requests
- **URL rewriting** in HLS playlists to route through the proxy

## Proxy Configuration

```yaml
playback:
  proxy:
    headers:
      Authorization: "Bearer {{stored.access_token}}"
      X-Custom-Header: "value"
    cookies:
      auth_token: "{{stored.access_token}}"
    url_params_to_cookies:
      - param: "hdnts"
        cookie: "hdnts"
    skip_url_param_cookies_for_keys: true
```

### Configuration Fields

| Field | Description |
|-------|-------------|
| `headers` | Headers added to every proxied request |
| `cookies` | Static cookies added to proxied requests |
| `url_params_to_cookies` | Maps URL query parameters to cookies |
| `skip_url_param_cookies_for_keys` | When `true`, URL-param cookies are not sent for encryption key requests |

## How the Proxy Works

### 1. Stream URL Generation

When the frontend requests a stream, the server:
1. Fetches the stream URL from the provider API.
2. Creates a **proxy context** containing headers, cookies, and URL-param mappings.
3. Generates an opaque **context token** for the proxy context.
4. Returns the stream URL rewritten to use the proxy endpoint.

### 2. Playlist Rewriting

For HLS streams, the proxy rewrites `.m3u8` playlists:
- Relative URLs (e.g., `segment001.ts`) are converted to absolute proxy URLs.
- Each segment URL includes the proxy context token.

```
# Original manifest
segment001.ts
segment002.ts

# Rewritten manifest
/api/proxy?url=https://cdn.example.com/segment001.ts&ctx=abc123
/api/proxy?url=https://cdn.example.com/segment002.ts&ctx=abc123
```

### 3. Request Proxying

When the frontend requests a proxied URL:
1. The server looks up the proxy context using the token.
2. Injects configured headers and cookies.
3. Converts URL parameters to cookies (if configured).
4. Forwards the request to the upstream CDN.
5. Returns the response to the client.

## URL Parameters to Cookies

Some CDNs (e.g., Akamai) pass authentication tokens as URL query parameters, but the upstream server expects them as cookies.

```yaml
url_params_to_cookies:
  - param: "hdnts"
    cookie: "hdnts"
```

This configuration:
1. Extracts the `hdnts` query parameter from the request URL.
2. Sets it as a `hdnts` cookie on the proxied request.
3. Removes the parameter from the URL.

### Key Request Handling

Encryption key requests (`.key` files) may need different cookie handling:

```yaml
skip_url_param_cookies_for_keys: true
```

When set to `true`, URL-param cookies are **not** sent for key file requests. This is needed when the key server does not accept CDN cookies.

## Proxy Endpoint

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/proxy?url=<stream_url>&ctx=<token>` | Proxy a stream request |

### Query Parameters

| Parameter | Description |
|-----------|-------------|
| `url` | The upstream URL to fetch |
| `ctx` | Proxy context token (contains headers, cookies, etc.) |

## Example: Akamai CDN

A provider using Akamai CDN with signed URLs:

```yaml
playback:
  stream:
    request:
      method: "GET"
      path: "/channels/{{input.channel_id}}/play"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      url: "$.play_url"
      type: "$.type"

  proxy:
    headers:
      User-Agent: "MyApp/1.0"
    url_params_to_cookies:
      - param: "hdnts"
        cookie: "hdnts"
      - param: "hdntl"
        cookie: "hdntl"
    skip_url_param_cookies_for_keys: true
```

This setup:
1. Fetches the stream URL (which includes Akamai `hdnts` tokens as query params).
2. On each segment request, extracts `hdnts` and `hdntl` from the URL and converts them to cookies.
3. For encryption key requests, skips the URL-param cookies.
