## ADDED Requirements

### Requirement: DRM license proxy endpoint must forward license acquisition requests to upstream server
The system SHALL expose a POST endpoint at `/api/proxy/drm/{token}` that accepts a binary request body (Widevine license challenge), looks up the `ProxyContext` by token from the proxy context cache, and forwards the request body to the upstream license URL stored in that context. The response body from the upstream license server SHALL be returned verbatim to the client with the same content type.

#### Scenario: Valid license request is proxied to upstream
- **WHEN** a client sends a POST request to `/api/proxy/drm/{token}` with a valid context token and a binary Widevine challenge body
- **THEN** the system forwards the body to the upstream license URL from the context, applies the configured DRM headers and cookies, and returns the upstream response body and status code to the client

#### Scenario: License request with invalid or expired token is rejected
- **WHEN** a client sends a POST request to `/api/proxy/drm/{token}` with a token that does not exist in the proxy context cache
- **THEN** the system returns a 403 or 404 error and does not make any upstream request

#### Scenario: License request with missing body is rejected
- **WHEN** a client sends a POST request to `/api/proxy/drm/{token}` with an empty body
- **THEN** the system returns a 400 error

### Requirement: DRM license proxy must apply provider-configured authentication
The system SHALL apply headers and cookies from the `ProxyContext` DRM configuration when forwarding license requests to the upstream server. The headers and cookies SHALL be those resolved at stream context creation time from the provider YAML config's `drm_license_headers` and `drm_license_cookies` fields.

#### Scenario: Configured DRM headers are sent on license request
- **WHEN** the `ProxyContext` contains DRM license headers (e.g., `crmid`, `deviceId`, `osVersion`, `usergroup`)
- **THEN** the upstream license request includes all configured headers with their resolved values

#### Scenario: Configured DRM cookies are forwarded on license request
- **WHEN** the `ProxyContext` contains DRM license cookie names and matching cookies exist in the resolved cookie store
- **THEN** the upstream license request includes those cookies in the `Cookie` header

### Requirement: DRM license proxy must perform optional prefetch request for cookie refresh
The system SHALL support an optional `drm_prefetch_url` field in `ProxyContext`. When set, the DRM license proxy handler SHALL perform a HEAD request to that URL before forwarding the license request, to refresh authentication cookies required by the upstream license server.

#### Scenario: Prefetch URL triggers HEAD request before license proxy
- **WHEN** a DRM license proxy request is made and the `ProxyContext` has a non-empty `drm_prefetch_url`
- **THEN** the system performs a HEAD request to the prefetch URL before forwarding the license body to the upstream license server

#### Scenario: No prefetch when field is unset
- **WHEN** a DRM license proxy request is made and the `ProxyContext` has no `drm_prefetch_url`
- **THEN** the system skips the HEAD prefetch and forwards the license body directly

### Requirement: DRM license proxy configuration must be declarable in provider YAML
The system SHALL support the following optional fields in the `PlaybackEndpoint` YAML schema for DRM license proxy configuration:
- `drm_response.is_drm`: JSON path to extract a boolean DRM flag from the upstream API response
- `drm_response.mpd_url`: JSON path to extract the DASH MPD URL from the upstream API response
- `drm_response.license_key_url`: JSON path to extract the DRM license server URL from the upstream API response
- `drm_license_headers`: Map of header name to template-resolvable value strings for license requests
- `drm_license_cookies`: List of cookie names to forward on license requests
- `drm_prefetch_url`: Optional template-resolvable URL for the HEAD prefetch

#### Scenario: Provider YAML with DRM block produces DRM-aware stream context
- **WHEN** a provider YAML declares `drm_response` fields and the upstream API response indicates `isDRM: true`
- **THEN** the stream endpoint creates a `ProxyContext` with `stream_type: Dash`, the extracted MPD URL, license URL, and configured DRM headers/cookies

#### Scenario: Provider YAML without DRM block produces standard HLS context
- **WHEN** a provider YAML does not declare `drm_response` fields or the upstream response does not indicate DRM
- **THEN** the stream endpoint creates a standard HLS `ProxyContext` with no DRM fields set
