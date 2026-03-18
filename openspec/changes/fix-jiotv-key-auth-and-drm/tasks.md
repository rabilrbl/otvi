## 1. Fix JioTV HLS Key Authentication

- [x] 1.1 In `providers/jiotv-mobile.yaml`, set `key_exclude_resolved_cookies: false` and verify `resolved_cookie_names` includes `__hdnea__`
- [x] 1.2 Fix header name case mismatches in `proxy_headers` to match DRM headers (lowercase `accesstoken` and `channelid` instead of camelCase, matching JioTV-Go behavior)
- [x] 1.3 Add `deviceId` header to `proxy_headers` to align with DRM headers (JioTV key server may require it)
- [ ] 1.4 Verify the fix by running the project and confirming `.key`/`.pkey` requests for a JioTV HLS channel return 200 instead of 403

## 2. Extend Core Config Schema for DRM

- [x] 2.1 In `crates/otvi-core/src/config.rs`, add DRM response extraction fields to `DrmResponseConfig`: `is_drm` (JSON path), `mpd_url` (JSON path), `cookies` (Vec<String>), `prefetch_url` (Option<String>)
- [x] 2.2 Existing `DrmResponseConfig` already had `system`, `license_url`, and `headers` fields
- [x] 2.3 In `crates/otvi-core/src/types.rs`, `StreamType` enum and `stream_type` field on `StreamInfo` already existed; added `Default` derive with `Hls` as default
- [x] 2.4 `DrmInfo` in `types.rs` already has `system`, `license_url`, and `headers` — sufficient for proxied license URL
- [x] 2.5 `cargo check` on `otvi-core` passes

## 3. Extend ProxyContext for DRM and DASH

- [x] 3.1 In `crates/otvi-server/src/state.rs`, added fields to `ProxyContext`: `stream_type`, `drm_license_url`, `drm_license_headers`, `drm_license_cookies`, `drm_prefetch_url`
- [x] 3.2 Updated `ProxyContext` construction in `channels.rs` stream handler to populate DRM fields when `is_drm` is truthy, with literal-vs-JSON-path handling for `system` and `prefetch_url`
- [x] 3.3 Stream endpoint response (`StreamInfo`) includes proxied DRM license URL (`/api/proxy/drm/{token}`) and correct `stream_type` when DRM is active
- [x] 3.4 `cargo check` on `otvi-server` passes

## 4. Implement DRM License Proxy Endpoint

- [x] 4.1 Added `proxy_drm` POST handler in `proxy.rs` for `/api/proxy/drm/{token}` — looks up `ProxyContext`, validates request, forwards binary body to upstream license URL
- [x] 4.2 Applies DRM-specific headers from `ProxyContext.drm_license_headers` and falls back to proxy headers; applies cookies from `drm_license_cookies` resolved against `static_cookies`
- [x] 4.3 Implements optional HEAD prefetch to `drm_prefetch_url` (fire-and-forget with warning on failure)
- [x] 4.4 Route registered at `/api/proxy/drm/{token}` in `lib.rs` with OpenAPI path
- [x] 4.5 Error handling: 404 for invalid token, 400 for empty body or missing DRM config, 502 for upstream failures, pass-through of upstream status codes

## 5. Implement DASH MPD Manifest Proxy and Rewriting

- [x] 5.1 Extended `proxy_stream` to detect MPD content by content-type (`dash+xml`, `application/xml` + `.mpd` in URL) and URL extension (`.mpd`, `.mpd?`)
- [x] 5.2 Implemented `rewrite_mpd` function with `<BaseURL>` string replacement — resolves relative URLs, routes through proxy with ctx token
- [x] 5.3 Fallback logic: on rewriting error, logs warning and returns unmodified MPD content
- [x] 5.4 DASH segment requests use existing `proxy_stream` cookie/header forwarding from `ProxyContext`; MPD rewriting discovers new hosts and updates `allowed_hosts`

## 6. Update JioTV Provider YAML for DRM

- [x] 6.1 Added `drm:` block in `providers/jiotv-mobile.yaml` with `is_drm: "$.isDRM"`, `mpd_url: "$.mpd.result"`, `system: "widevine"`, `license_url: "$.mpd.key"`
- [x] 6.2 Added DRM license headers: `accesstoken`, `appName`, `crmid`, `deviceId`, `devicetype`, `os`, `osVersion`, `ssotoken`, `subscriberId`, `uniqueId`, `userId`, `usergroup`, `versionCode`, `x-platform`
- [x] 6.3 Added `cookies` list and `prefetch_url: "$.mpd.result"` configuration
- [x] 6.4 YAML parses correctly — `cargo check` passes, all tests pass

## 7. Verification and Cleanup

- [x] 7.1 `cargo fmt` — clean (all formatting issues fixed)
- [x] 7.2 `cargo clippy` — clean (0 warnings after collapsible-if fixes)
- [x] 7.3 `cargo test` — 144 unit tests pass, 57 integration tests pass, 1 doctest passes, 0 failures
- [ ] 7.4 Manual testing with live JioTV service (requires JioTV credentials and network access)
