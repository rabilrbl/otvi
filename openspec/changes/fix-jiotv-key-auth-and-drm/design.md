## Context

OTVI is a data-driven TV streaming proxy: each provider is a YAML file (`ProviderConfig`) parsed into structs at startup. The server generically resolves templates, executes HTTP requests, rewrites HLS manifests, and proxies segments/keys — all driven by YAML config. There are no Rust traits per provider.

Currently:
- **HLS key auth is broken for JioTV**: Two issues were identified:
  1. The `key_exclude_resolved_cookies: true` setting in `providers/jiotv-mobile.yaml` prevented the `__hdnea__` cookie from being forwarded on `.key`/`.pkey` requests (now fixed to `false`)
  2. **Header name case mismatch**: `proxy_headers` used camelCase names (`accessToken`, `channelId`) but JioTV's key server (tv.media.jio.com) expects lowercase headers (`accesstoken`, `channelid`) as used by JioTV-Go's RenderKeyHandler and confirmed by working DRM headers
- **DRM-only channel detection fails**: The code extracted the HLS URL (`$.bitrates.auto`) before checking the DRM flag. Mandatory DRM channels that only return MPD URLs (no HLS fallback) would fail with "Stream URL not found in response" error, never reaching the DRM detection logic. Fixed by checking `is_drm` first and using MPD URL as fallback when HLS URL is missing.
- **DRM is schema-only**: `DrmResponseConfig` exists in `config.rs` with fields `system`, `license_url`, and `headers`, and `StreamInfo` has an `Option<DrmInfo>` field. But no server handler reads them — there is no DRM license proxy endpoint and no DASH MPD manifest rewriting.
- **Proxy only speaks HLS**: `proxy_stream` in `proxy.rs` only handles m3u8 rewriting and TS/key segment proxying. DASH MPD XML rewriting is not implemented.

The reference implementation is JioTV-Go, which has `DRMKeyHandler` (license proxy with HEAD-request cookie prefetch), `MpdHandler` (MPD manifest proxy with BaseURL rewriting), and `DashHandler` (DASH segment proxy).

## Goals / Non-Goals

**Goals:**
- Fix JioTV HLS key authentication so `.key`/`.pkey` requests receive the `__hdnea__` cookie and return 200
- Add a generic DRM license proxy endpoint (`/api/proxy/drm/{token}`) that any YAML-configured provider can use
- Add DASH MPD manifest fetching and URL rewriting through the existing proxy infrastructure
- Keep the data-driven architecture: all new DRM/DASH behavior is configured via YAML, not hardcoded per provider
- Extend `ProxyContext` to carry DRM license URL, DRM headers, and stream type (HLS vs DASH) so the proxy knows how to handle each request

**Non-Goals:**
- Frontend Widevine player implementation (tracked in proposal but deferred to a separate change if needed)
- FairPlay or PlayReady DRM support (Widevine only for now; schema is extensible)
- Offline/download DRM license persistence
- Multi-period MPD support (JioTV uses single-period live MPD)
- Changes to the database schema or user model

## Decisions

### 1. Fix key auth by changing YAML config, not Rust code

**Decision**: 
1. Set `key_exclude_resolved_cookies: false` in `jiotv-mobile.yaml` to forward `__hdnea__` cookie
2. Change header names in `proxy_headers` from camelCase to lowercase to match JioTV-Go and DRM headers: `accessToken` → `accesstoken`, `channelId` → `channelid`
3. Add `deviceId` header to `proxy_headers` to align with DRM header requirements

**Rationale**: 
- The existing cookie forwarding machinery already supports forwarding resolved cookies — the YAML just had it disabled
- **Critical discovery**: JioTV's key server at tv.media.jio.com expects lowercase header names, not camelCase. This was confirmed by comparing with the working DRM headers which use lowercase and explicitly mirror JioTV-Go's implementation
- HTTP header names are case-insensitive per RFC 7230, but some CDN/proxy implementations (including JioTV's Akamai setup) are case-sensitive in practice
- No Rust code change needed — purely configuration fix

**Alternatives considered**:
- Add a separate `key_cookies` YAML field → Unnecessary complexity; the existing mechanism handles it
- Use HTTP/2 pseudo-headers → Not applicable; JioTV uses HTTP/1.1

### 1a. Check DRM flag before extracting stream URL

**Decision**: Reorder the stream endpoint response processing to check the `is_drm` flag *before* attempting to extract the HLS stream URL. When a channel is DRM-only (no HLS fallback), use the MPD URL from `drm.mpd_url` as the primary stream URL and force the stream type to DASH.

**Rationale**:
- Some JioTV channels are **mandatory DRM** — they only return an MPD URL in the response, with no `$.bitrates.auto` HLS fallback
- The original code extracted `$.bitrates.auto` first (lines 453-462), which would fail with "Stream URL not found in response" for DRM-only channels, stopping execution before the DRM detection logic could run
- By checking `is_drm` early and falling back to MPD URL when HLS URL is missing, we handle both:
  1. **DRM-only channels**: Use MPD URL from the start
  2. **Hybrid channels**: Have both HLS and MPD, prefer MPD for DRM
  3. **HLS-only channels**: Use HLS URL as before

**Implementation**:
```rust
// Check is_drm first
let is_drm = drm_cfg
    .and_then(|cfg| cfg.is_drm.as_ref())
    .and_then(|path| extract_json_path(&response, path))
    .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
    .unwrap_or(false);

// Extract stream URL with MPD fallback for DRM-only channels
let stream_url = extract_json_path(&response, &stream_endpoint.response.url)
    .or_else(|| {
        if is_drm {
            drm_cfg
                .and_then(|cfg| cfg.mpd_url.as_ref())
                .and_then(|mpd_path| extract_json_path(&response, mpd_path))
        } else {
            None
        }
    })
    .ok_or_else(|| AppError::Internal("Stream URL not found in response".into()))?;
```

**Alternatives considered**:
- Make HLS URL optional in schema → Would require broader schema changes and affect all providers
- Add separate `drm_only` flag to config → Unnecessary; `is_drm` + missing HLS URL already indicates this
- Keep original order and special-case the error → Less clean than fixing the root cause

### 2. DRM license proxy as a new Axum route handler, not a proxy_stream mode

**Decision**: Add a dedicated `/api/proxy/drm/{token}` POST endpoint as a new handler function in `proxy.rs`, separate from the existing `proxy_stream` GET handler.

**Rationale**: License acquisition is a POST request with a binary body (Widevine challenge), fundamentally different from GET-based segment/manifest proxying. Mixing it into `proxy_stream` would complicate the existing handler with conditional logic. A separate handler is cleaner and matches JioTV-Go's architecture (`DRMKeyHandler` is separate from `RenderHandler`).

**Alternatives considered**:
- Extend `proxy_stream` with a query param like `?mode=drm` → Conflates GET manifest/segment proxying with POST license requests; harder to reason about security.

### 3. DASH MPD rewriting inside proxy_stream via content-type detection

**Decision**: In the existing `proxy_stream` handler, after fetching the upstream response, detect DASH content by checking `Content-Type` for `application/dash+xml` or the URL extension `.mpd`. When detected, parse the MPD XML body and rewrite `<BaseURL>` elements to point back through the proxy, analogous to how m3u8 line-by-line rewriting works for HLS.

**Rationale**: MPD manifests are fetched via GET just like m3u8 manifests, and the proxy already has the infrastructure for fetching, rewriting, and re-serving manifests. Adding a content-type branch keeps the architecture consistent.

**Alternatives considered**:
- Separate `/api/proxy/mpd/{token}` endpoint → Would duplicate the fetch/cookie/header logic already in `proxy_stream`. Since MPD is a GET manifest just like m3u8, reusing the same handler is cleaner.

### 4. Extend ProxyContext with stream_type and DRM fields

**Decision**: Add to `ProxyContext`:
- `stream_type: StreamType` (enum: `Hls`, `Dash`) to control rewriting behavior
- `drm_license_url: Option<String>` — the upstream license server URL template
- `drm_license_headers: Option<HashMap<String, String>>` — extra headers for license requests
- `drm_license_cookies: Option<Vec<String>>` — cookie names to forward on license requests

**Rationale**: `ProxyContext` is the ephemeral per-stream state stored in the moka cache. It already carries HLS-specific fields (base_url, key patterns, cookies). DRM fields follow the same pattern. The `stream_type` field lets `proxy_stream` branch between m3u8 and MPD rewriting without guessing.

### 5. Extend PlaybackEndpoint YAML schema for DRM config

**Decision**: Add optional fields to `PlaybackEndpoint` (in `config.rs`):
- `drm_response` block: JSON paths to extract `is_drm`, `mpd_url`, and `license_key_url` from the upstream playback API response
- `drm_license_headers`: template-resolved headers to send on license proxy requests
- `drm_license_cookies`: cookie names to forward on license proxy requests

**Rationale**: This keeps the data-driven architecture — the YAML declares how to extract DRM info from the API response and what auth to apply, the server generically executes it. No provider-specific Rust code.

### 6. MPD rewriting strategy: string-based BaseURL replacement

**Decision**: Use simple string/regex replacement on MPD XML to rewrite `<BaseURL>` values, rather than full XML DOM parsing.

**Rationale**: JioTV-Go uses `strings.Replace` for MPD rewriting (`getDrmMpd` function). MPD BaseURL elements are simple and predictable. Full XML parsing (e.g., with `quick-xml`) adds complexity and potential formatting changes. String replacement is sufficient and matches the reference implementation.

**Alternatives considered**:
- Full XML DOM parse with `quick-xml` → Overkill for replacing BaseURL text content; risks reformatting the XML and breaking player compatibility.

### 7. License proxy HEAD-request cookie prefetch

**Decision**: Before proxying the license POST request, the DRM license handler performs a HEAD request to the channel's original stream URL to refresh authentication cookies (matching JioTV-Go's `DRMKeyHandler` which does `http.Head(channelURL)`).

**Rationale**: JioTV's CDN rotates cookies; the HEAD request ensures fresh cookies are available for the license server. This is stored as an optional `drm_prefetch_url` in `ProxyContext` — providers that don't need it simply leave it unset.

## Risks / Trade-offs

**[Risk] String-based MPD rewriting may break on complex MPD structures** → Mitigation: Scope to single-period live MPDs (JioTV's format). Add a fallback that returns the unmodified MPD if rewriting fails, with a warning log. Can upgrade to DOM parsing later if needed.

**[Risk] DRM license proxy could be abused as an open relay** → Mitigation: License proxy requires a valid `ProxyContext` token from the moka cache (same as segment proxy). The upstream license URL is server-determined from YAML config, not caller-supplied. Same security model as existing proxy.

**[Risk] HEAD prefetch adds latency to every license request** → Mitigation: Only performed when `drm_prefetch_url` is set in the provider config. The HEAD request is lightweight. Could add cookie caching with short TTL later if latency is a concern.

**[Risk] Cookie forwarding change could leak cookies to unintended key servers** → Mitigation: The `key_url_pattern` in YAML still controls which URLs are treated as key requests. Cookies are only forwarded to URLs matching the pattern. The fix only removes the blanket exclusion; per-URL pattern matching is still enforced.

## Open Questions

- Should the MPD rewriting also handle `<SegmentTemplate>` media/init URLs, or is `<BaseURL>` sufficient for JioTV's MPD format? (Need to inspect actual JioTV MPD response to confirm.)
- Should the DRM license proxy response include CORS headers for browser-based players, or is the frontend same-origin sufficient?
