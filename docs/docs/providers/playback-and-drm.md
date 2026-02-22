---
sidebar_position: 6
title: Playback & DRM
---

# Playback & DRM

OTVI supports HLS and DASH streaming with optional DRM (Digital Rights Management) for protected content.

## Stream Configuration

```yaml
playback:
  stream:
    request:
      method: "GET"
      path: "/v2/channels/{{input.channel_id}}/stream"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      url: "$.data.manifest_url"
      type: "$.data.stream_format"
      drm:
        system: "$.data.drm.type"
        license_url: "$.data.drm.license_server_url"
        headers:
          Authorization: "Bearer {{stored.access_token}}"
          X-Custom-Data: "{{stored.user_id}}"
```

### Request

The stream request fetches the playback URL for a specific channel. The `{{input.channel_id}}` variable is automatically populated by the server when a user requests a channel stream.

### Response Fields

| Field | Description |
|-------|-------------|
| `url` | JSONPath to the stream manifest URL (`.m3u8` for HLS, `.mpd` for DASH) |
| `type` | JSONPath to the stream type — must resolve to `"hls"` or `"dash"` |
| `drm` | Optional DRM configuration (omit for clear/unencrypted streams) |

## Stream Types

### HLS (HTTP Live Streaming)

Played using [HLS.js](https://github.com/video-dev/hls.js/) in the browser.

```yaml
response:
  url: "$.stream_url"
  type: "$.format"    # Must resolve to "hls"
```

### DASH (Dynamic Adaptive Streaming over HTTP)

Played using [Shaka Player](https://github.com/shaka-project/shaka-player) in the browser.

```yaml
response:
  url: "$.stream_url"
  type: "$.format"    # Must resolve to "dash"
```

## DRM Configuration

For DRM-protected content, add the `drm` section to the stream response configuration.

### Supported DRM Systems

| System | Value | Description |
|--------|-------|-------------|
| Widevine | `widevine` | Google's DRM system (Chrome, Firefox, Android) |
| PlayReady | `playready` | Microsoft's DRM system (Edge, Windows) |

### DRM Response Fields

```yaml
drm:
  system: "$.data.drm.type"                    # JSONPath to DRM system name
  license_url: "$.data.drm.license_server_url"  # JSONPath to license server URL
  headers:                                       # Headers sent with license requests
    Authorization: "Bearer {{stored.access_token}}"
    X-Custom-Data: "{{stored.user_id}}"
```

| Field | Description |
|-------|-------------|
| `system` | JSONPath to the DRM system identifier (`widevine` or `playready`) |
| `license_url` | JSONPath to the license server URL |
| `headers` | Optional headers included in license acquisition requests |

### How DRM Works in OTVI

1. The server fetches the stream URL and DRM info from the provider API.
2. The response includes the manifest URL, DRM system, and license server URL.
3. The frontend initializes Shaka Player with the DRM configuration.
4. Shaka Player automatically requests a license from the license server.
5. DRM headers (e.g., authentication tokens) are injected into license requests.

### Example: Widevine DRM

```yaml
playback:
  stream:
    request:
      method: "GET"
      path: "/stream/{{input.channel_id}}"
      headers:
        Authorization: "Bearer {{stored.access_token}}"
    response:
      url: "$.manifest_url"
      type: "$.format"
      drm:
        system: "$.drm.system"              # Resolves to "widevine"
        license_url: "$.drm.license_url"     # e.g., "https://license.example.com/widevine"
        headers:
          Authorization: "Bearer {{stored.access_token}}"
```

## Clear Streams (No DRM)

For unencrypted HLS or DASH streams, simply omit the `drm` section:

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
```

## Frontend Player Integration

The web frontend includes JavaScript bridges for both players:

- **`otviInitHls(videoId, url)`** — Initializes HLS.js for HLS streams
- **`otviInitDash(videoId, url, drmConfigJson)`** — Initializes Shaka Player for DASH + DRM
- **`otviDestroyPlayer()`** — Cleans up the active player instance

The frontend automatically selects the appropriate player based on the `type` value returned by the stream endpoint.
