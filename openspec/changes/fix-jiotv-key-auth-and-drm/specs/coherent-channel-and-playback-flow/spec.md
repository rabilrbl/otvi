## MODIFIED Requirements

### Requirement: Playback view must resolve channel metadata without fetching unrelated full datasets
The system MUST provide enough backend-supported information for the playback view to display channel identity metadata without re-fetching the entire provider channel lineup solely to resolve a single channel's name or logo.

#### Scenario: Player renders channel identity from targeted playback data
- **WHEN** a user opens playback for a channel
- **THEN** the playback flow obtains the channel's display metadata from targeted route state, a dedicated metadata source, or enriched stream data rather than a full channel-list fetch

## ADDED Requirements

### Requirement: Stream endpoint response must include DRM proxy URLs when DRM is active
The system SHALL include DRM-specific proxy URLs in the stream endpoint response when the upstream API indicates a DRM-protected stream. The response SHALL include:
- A proxied DASH MPD manifest URL (routed through `/api/proxy/stream/{token}`)
- A proxied DRM license URL (routed through `/api/proxy/drm/{token}`)
- The DRM system identifier (e.g., `widevine`)

#### Scenario: DRM channel stream response includes license and manifest proxy URLs
- **WHEN** a client requests the stream endpoint for a channel whose upstream API indicates `isDRM: true`
- **THEN** the response includes `drm.system` set to `widevine`, `drm.license_url` pointing to the proxy DRM endpoint, and the stream URL pointing to the proxy DASH MPD endpoint

#### Scenario: Non-DRM channel stream response has no DRM fields
- **WHEN** a client requests the stream endpoint for a channel that is not DRM-protected
- **THEN** the response does not include DRM fields (or they are null/absent), and the stream URL points to the proxy HLS manifest endpoint as before

### Requirement: Stream type must be communicated in stream endpoint response
The system SHALL include a `stream_type` field in the stream endpoint response indicating whether the stream is `hls` or `dash`, so the frontend player can select the appropriate playback strategy.

#### Scenario: HLS stream returns stream_type hls
- **WHEN** a client requests the stream endpoint for a non-DRM HLS channel
- **THEN** the response includes `stream_type: "hls"`

#### Scenario: DRM DASH stream returns stream_type dash
- **WHEN** a client requests the stream endpoint for a DRM-protected DASH channel
- **THEN** the response includes `stream_type: "dash"`
