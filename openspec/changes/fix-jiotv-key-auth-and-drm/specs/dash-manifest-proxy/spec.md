## ADDED Requirements

### Requirement: Proxy must detect and rewrite DASH MPD manifests
The system SHALL detect DASH MPD content in proxy responses by checking the `Content-Type` header for `application/dash+xml` or the request URL extension `.mpd`. When DASH content is detected, the system SHALL rewrite `<BaseURL>` elements in the MPD XML so that segment URLs route back through the proxy, analogous to the existing HLS m3u8 URI rewriting.

#### Scenario: MPD manifest BaseURL elements are rewritten to proxy URLs
- **WHEN** the proxy fetches an upstream MPD manifest containing `<BaseURL>https://cdn.example.com/segments/</BaseURL>`
- **THEN** the returned MPD replaces the BaseURL content with a proxy-relative URL that routes segment requests through the proxy endpoint

#### Scenario: MPD without BaseURL elements is returned unmodified
- **WHEN** the proxy fetches an upstream MPD manifest that contains no `<BaseURL>` elements
- **THEN** the manifest is returned as-is without modification

#### Scenario: MPD rewriting failure falls back to unmodified manifest
- **WHEN** the proxy attempts to rewrite an MPD manifest but the rewriting logic encounters an error (e.g., malformed XML)
- **THEN** the system logs a warning and returns the original unmodified MPD content to the client

### Requirement: Proxy must handle DASH segment requests
The system SHALL proxy DASH segment requests (audio/video segments referenced by the rewritten MPD) using the same cookie and header forwarding logic as HLS segment requests. The `ProxyContext` stream_type field SHALL determine whether to apply HLS or DASH handling.

#### Scenario: DASH segment request is proxied with correct authentication
- **WHEN** a client requests a DASH segment URL through the proxy and the `ProxyContext` has `stream_type: Dash`
- **THEN** the proxy fetches the upstream segment with the context's configured cookies and headers, and returns the binary segment data to the client

### Requirement: ProxyContext must carry stream type for routing decisions
The system SHALL include a `stream_type` field in `ProxyContext` with values `Hls` or `Dash`. The proxy_stream handler SHALL use this field to determine whether to apply m3u8 rewriting (HLS) or MPD rewriting (DASH) when the response is a manifest.

#### Scenario: HLS context triggers m3u8 rewriting
- **WHEN** `proxy_stream` receives a manifest response and the `ProxyContext.stream_type` is `Hls`
- **THEN** the handler applies HLS m3u8 line-by-line rewriting

#### Scenario: DASH context triggers MPD rewriting
- **WHEN** `proxy_stream` receives a manifest response and the `ProxyContext.stream_type` is `Dash`
- **THEN** the handler applies DASH MPD BaseURL rewriting
