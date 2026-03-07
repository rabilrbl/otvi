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
- **CORS bypass** — browsers block cross-origin media requests
- **URL rewriting** in HLS playlists to route segments through the proxy

## Proxy Configuration

Proxy settings sit **directly under `playback.stream`**, alongside `request` and `response`. There is no separate top-level `playback.proxy` key.

```yaml
playback:
  stream:
    request: { ... }
    response: { ... }

    # ── Proxy settings ──────────────────────────────────────────────────────

    # Headers forwarded on every upstream proxy request (manifest, segments, keys).
    # Supports the same {{stored.*}} / {{input.*}} template variables as request specs.
    proxy_headers:
      Authorization: "Bearer {{stored.access_token}}"
      User-Agent: "MyApp/1.0"

    # Maps a URL query-parameter name from the upstream stream URL to a cookie name.
    # The proxy extracts the value from the manifest URL and sends it as the named
    # cookie on all sub-requests (segments, key files…).
    # Format: <url_param_name>: <cookie_name>
    proxy_url_cookies:
      hdnea: "__hdnea__"    # Akamai token → __hdnea__ cookie

    # Static cookie values sent verbatim on every upstream proxy request.
    # Supports {{stored.*}} template variables.
    proxy_cookies:
      ssotoken: "{{stored.sso_token}}"
      crmid:    "{{stored.crm}}"

    # When true, the raw query string from the first manifest URL that carries
    # query params is appended to every EXT-X-KEY URI before the proxy fetches
    # the key file. Use this when the key server requires the same auth token
    # that appeared in the manifest URL as a query param (not a cookie).
    append_manifest_query_to_key_uris: false

    # When true, URL-param-extracted cookies (resolved from the manifest URL via
    # proxy_url_cookies) are NOT forwarded on AES-128 key requests.
    # Use this when the key server rejects CDN auth tokens (e.g. an Akamai
    # __hdnea__ token whose ACL covers only the segment CDN path).
    # Static proxy_cookies are still forwarded regardless.
    key_exclude_resolved_cookies: false

    # Substring patterns (case-insensitive) that identify encryption-key URIs
    # inside EXT-X-KEY lines when append_manifest_query_to_key_uris is true.
    # If empty (the default), the manifest query is appended to ALL URIs found
    # in EXT-X-KEY lines. If patterns are given, only URIs containing at least
    # one pattern receive the append.
    key_uri_patterns:
      - ".pkey"        # Only append manifest query to .pkey key files
      - "/keyserver/"  # …or to any URI under /keyserver/
```

### Configuration Fields

| Field | Type | Description |
| --- | --- | --- |
| `proxy_headers` | map | Headers added to every proxied request (manifest, segments, keys). Supports template variables. |
| `proxy_url_cookies` | map | Maps URL query-param names from the manifest URL to cookie names sent on subsequent requests. |
| `proxy_cookies` | map | Static cookies sent verbatim on every proxied request. Supports template variables. |
| `append_manifest_query_to_key_uris` | boolean | When `true`, appends the manifest URL's raw query string to `EXT-X-KEY` URIs before fetching them. |
| `key_exclude_resolved_cookies` | boolean | When `true`, URL-param-extracted cookies are **not** sent on AES-128 key requests. |
| `key_uri_patterns` | string[] | Restrict `append_manifest_query_to_key_uris` to key URIs matching at least one of these substrings. |

## How the Proxy Works

### 1. Stream URL Generation

When the frontend requests a stream, the server:

1. Calls the provider API to obtain the stream URL.
2. Creates a **proxy context** containing the resolved headers, cookies, and URL-param mappings.
3. Generates an opaque **context token** (`ctx`) for that proxy context.
4. Returns the stream URL rewritten to use the `/api/proxy` endpoint.

### 2. Playlist Rewriting

For HLS streams, the proxy rewrites `.m3u8` playlists so that all segment and sub-manifest URLs route through the proxy:

```
# Original manifest
#EXTM3U
segment001.ts
segment002.ts

# Rewritten manifest
#EXTM3U
/api/proxy?url=https%3A%2F%2Fcdn.example.com%2Fsegment001.ts&ctx=abc123
/api/proxy?url=https%3A%2F%2Fcdn.example.com%2Fsegment002.ts&ctx=abc123
```

Relative URLs are converted to absolute proxy URLs. The context token is embedded in every URL so the proxy can look up the correct headers and cookies for each request.

### 3. Request Proxying

When the player requests a proxied URL:

1. The server looks up the proxy context using the `ctx` token.
2. Injects `proxy_headers` into the upstream request.
3. Attaches `proxy_cookies` as `Cookie` headers.
4. Resolves `proxy_url_cookies` — extracts named query parameters from the request URL and converts them to cookies.
5. Applies `key_exclude_resolved_cookies` / `append_manifest_query_to_key_uris` logic for AES-128 key requests.
6. Forwards the request to the upstream CDN.
7. Returns the response to the browser.

## URL Parameters to Cookies (`proxy_url_cookies`)

Some CDNs (e.g., Akamai) embed authentication tokens in the manifest URL as query parameters. The upstream server then expects those tokens as cookies on segment requests.

```yaml
proxy_url_cookies:
  hdnea: "__hdnea__"
```

Given a manifest URL like:

```
https://cdn.example.com/live/ch1.m3u8?hdnea=exp=1234~acl=/live/*~hmac=abc
```

The proxy:

1. Extracts the value of the `hdnea` query parameter.
2. Sends it as the `__hdnea__` cookie on all subsequent segment and key requests.

### Key Request Handling

Encryption key requests (`.key` files, `EXT-X-KEY` URIs) may need different cookie handling. If the key server is on a separate domain that does **not** accept CDN tokens, set:

```yaml
key_exclude_resolved_cookies: true
```

When set to `true`, URL-param-derived cookies (from `proxy_url_cookies`) are stripped from key requests. Statically configured `proxy_cookies` are still sent regardless.

## Appending Manifest Query to Key URIs (`append_manifest_query_to_key_uris`)

Some providers protect both segments and key files with the same query-string token, but the key URI inside the manifest does not include the token. Enable this option to append it automatically:

```yaml
append_manifest_query_to_key_uris: true
```

Given a manifest URL with query string `?token=abc123` and an `EXT-X-KEY` line:

```
#EXT-X-KEY:METHOD=AES-128,URI="https://keys.example.com/ch1.key"
```

The proxy rewrites the key URI to:

```
https://keys.example.com/ch1.key?token=abc123
```

### Targeting Specific Key URIs (`key_uri_patterns`)

If the manifest contains a mix of key URI types and you only want to append the query to a subset, use `key_uri_patterns`:

```yaml
append_manifest_query_to_key_uris: true
key_uri_patterns:
  - ".pkey"
  - "/keyserver/"
```

Only key URIs containing `.pkey` or `/keyserver/` will have the manifest query appended. Other key URIs are proxied as-is.

## Proxy Behaviour Matrix

| Scenario | Recommended settings |
| --- | --- |
| CDN authenticates every request via headers | `proxy_headers` only |
| CDN uses Akamai `hdnea` cookie (token in manifest URL query) | `proxy_url_cookies: { hdnea: "__hdnea__" }` |
| Key server is on a separate domain that rejects the CDN token | `key_exclude_resolved_cookies: true` |
| Key server requires the raw manifest query string as URL params | `append_manifest_query_to_key_uris: true` |
| Only some key URIs (e.g. `.pkey`) need the manifest query | `append_manifest_query_to_key_uris: true` + `key_uri_patterns: [".pkey"]` |
| Static session cookie required on all requests | `proxy_cookies: { name: "{{stored.value}}" }` |

## Proxy Endpoint

| Method | Path | Auth | Description |
| --- | --- | --- | --- |
| `GET` | `/api/proxy?url=<encoded_url>&ctx=<token>` | None | Proxy a stream request (manifest, segment, or key file) |

### Query Parameters

| Parameter | Description |
| --- | --- |
| `url` | URL-encoded upstream URL to fetch |
| `ctx` | Proxy context token (generated by the stream endpoint, contains headers and cookies) |

**No authentication is required** — the `ctx` token encapsulates all the necessary context. The token is opaque and scoped to the stream session.

## Examples

### Example 1: Bearer Token via Header

A provider that requires a `Bearer` token on every CDN request:

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

    proxy_headers:
      Authorization: "Bearer {{stored.access_token}}"
      User-Agent: "MyApp/1.0"
```

### Example 2: Akamai CDN with Signed URL

A provider using Akamai HDN token authentication:

```yaml
playback:
  stream:
    request:
      method: "GET"
      path: "/channels/{{input.channel_id}}/stream"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      url: "$.stream_url"
      type: "$.format"

    proxy_headers:
      User-Agent: "MyApp/1.0"
    proxy_url_cookies:
      hdnea: "__hdnea__"    # Akamai token from manifest URL → cookie
    key_exclude_resolved_cookies: true  # Key server doesn't accept the CDN cookie
```

This setup:
1. Fetches the stream URL (which includes an Akamai `hdnea` token as a query param).
2. On each segment request, extracts `hdnea` from the URL and sends it as the `__hdnea__` cookie.
3. For AES-128 key requests, strips the `__hdnea__` cookie (the key server is on a different domain).

### Example 3: Static Session Cookie + Manifest Query on Keys

A provider that uses a static SSO cookie and requires the manifest query string on key requests:

```yaml
playback:
  stream:
    request:
      method: "GET"
      path: "/channels/{{input.channel_id}}/stream"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      url: "$.manifest_url"
      type: "$.type"

    proxy_cookies:
      ssotoken: "{{stored.sso_token}}"
    append_manifest_query_to_key_uris: true
    key_uri_patterns:
      - ".pkey"
```

### Example 4: Combined — Headers + URL Cookies + Static Cookies

```yaml
playback:
  stream:
    request:
      method: "GET"
      path: "/stream/{{input.channel_id}}"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      url: "$.url"
      type: "$.type"

    proxy_headers:
      User-Agent: "MyApp/2.0"
      X-Device-ID: "{{stored.device_id}}"
    proxy_url_cookies:
      hdnts: "hdnts"
      hdntl: "hdntl"
    proxy_cookies:
      crm_id: "{{stored.crm_id}}"
    key_exclude_resolved_cookies: true
```
