## MODIFIED Requirements

### Requirement: Playback view must resolve channel metadata without fetching unrelated full datasets
The system MUST provide enough backend-supported information for the playback view to display channel identity metadata without re-fetching the entire provider channel lineup solely to resolve a single channel's name or logo. When an upstream API call during channel listing or stream resolution fails with a refresh-triggering status code and the provider has a refresh config, the system SHALL transparently refresh tokens and retry the request once before returning an error to the caller.

#### Scenario: Player renders channel identity from targeted playback data
- **WHEN** a user opens playback for a channel
- **THEN** the playback flow obtains the channel's display metadata from targeted route state, a dedicated metadata source, or enriched stream data rather than a full channel-list fetch

#### Scenario: Stream resolution retries after token refresh on auth failure
- **WHEN** a stream resolution upstream call returns an HTTP status code that matches the provider's `refresh.on_status_codes`
- **THEN** the system executes the provider's refresh flow, retries the stream resolution with refreshed tokens, and returns the result of the retry to the caller

#### Scenario: Channel list fetch retries after token refresh on auth failure
- **WHEN** a channel list upstream call returns an HTTP status code that matches the provider's `refresh.on_status_codes`
- **THEN** the system executes the provider's refresh flow, retries the channel list fetch with refreshed tokens, and returns the result of the retry to the caller
