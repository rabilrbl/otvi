## Why

JioTV channels that require DRM (Widevine via DASH/MPD) are currently unplayable because OTVI has no DRM license proxy or DASH manifest rewriting support — the `DrmResponseConfig` schema exists in the YAML config but the server has no handler to proxy license acquisition requests or rewrite MPD manifests. Additionally, `.key`/`.pkey` AES-128 key requests for HLS channels currently return 403 because the proxy does not forward the `__hdnea__` cookie correctly to the key server — the `key_exclude_resolved_cookies: true` setting in the JioTV YAML blocks the URL-extracted `__hdnea__` cookie on key requests, but the key server actually requires it (as confirmed by JioTV-Go's `RenderKeyHandler` which explicitly sets `__hdnea__` as a cookie on key requests).

## What Changes

- **Fix JioTV YAML key authentication**: Correct the `key_exclude_resolved_cookies` setting and proxy cookie/header configuration so that `.key`/`.pkey` requests receive the proper `__hdnea__` cookie and auth headers, matching JioTV-Go's `RenderKeyHandler` behavior.
- **Add generic DRM license proxy endpoint**: Implement a new `/api/proxy/drm` endpoint that proxies Widevine/FairPlay license acquisition requests to the upstream license server, applying provider-configured headers and cookies. This endpoint is generic and available to any provider that declares a `drm` block in its playback response config.
- **Add DASH/MPD manifest proxy and rewriting**: Extend the existing HLS proxy to also handle DASH MPD manifests — rewriting `<BaseURL>` and segment template URLs to route through the proxy, similar to how m3u8 rewriting works for HLS.
- **Extend `PlaybackEndpoint` config schema for DRM**: Add fields to the YAML schema so providers can declare DRM license proxy requirements (license URL template, extra headers, cookie forwarding) and DASH-specific proxy behavior.
- **Update JioTV YAML for DRM channels**: Add DRM configuration to the JioTV provider YAML so channels that the API reports as `isDRM: true` are served via DASH+Widevine with license proxying, matching JioTV-Go's `DRMKeyHandler` and `MpdHandler` implementation.
- **Extend `StreamInfo` response**: Include DRM-related proxy URLs (proxied license URL, DASH base URL info) in the stream endpoint response so the frontend player can configure Widevine decryption.

## Capabilities

### New Capabilities
- `drm-license-proxy`: Generic DRM license acquisition proxy that forwards license requests to upstream servers with provider-configured authentication (headers, cookies). Supports Widevine and is extensible to other DRM systems.
- `dash-manifest-proxy`: DASH MPD manifest fetching and URL rewriting through the existing proxy infrastructure, analogous to the existing HLS m3u8 rewriting.

### Modified Capabilities
- `secure-provider-boundaries`: The proxy must now also validate DRM license proxy requests against server-issued stream contexts, ensuring the same access control applies to license acquisition as to segment/manifest fetching.
- `coherent-channel-and-playback-flow`: The stream endpoint response contract expands to include DRM proxy URLs and DASH stream type metadata, and the frontend playback view must handle Widevine-protected DASH streams.

## Impact

- **Backend** (`crates/otvi-server`):
  - New route handler for `/api/proxy/drm` (DRM license proxy)
  - Extended `proxy_stream` handler to detect and rewrite DASH MPD content
  - `ProxyContext` extended with DRM license URL and DASH-specific fields
  - `channels::stream` handler extended to build DRM proxy URLs when `isDRM` is present
- **Core** (`crates/otvi-core`):
  - `PlaybackEndpoint` schema gains new optional fields for DRM license proxy config
  - `PlaybackResponse` schema gains fields for extracting DASH MPD URL and DRM flag from API response
  - `StreamInfo` type extended with proxied DRM license URL fields
- **Provider YAML** (`providers/jiotv-mobile.yaml`):
  - Fix `key_exclude_resolved_cookies` (should be `false` for JioTV)
  - Add DRM response extraction config (`$.isDRM`, `$.mpd.result`, `$.mpd.key`)
  - Add DRM license proxy headers matching JioTV-Go's `DRMKeyHandler`
- **Frontend** (`web/`):
  - Player component must support DASH+Widevine playback when `StreamInfo.drm` is present
- **No database changes** — all new state is ephemeral (proxy context cache)
- **No breaking API changes** — new fields are additive with `Option`/`default` semantics
