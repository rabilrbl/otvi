## ADDED Requirements

### Requirement: Provider sessions with refresh config SHALL automatically refresh tokens on upstream auth failure
When a provider YAML defines an `auth.refresh` block and an upstream API call returns an HTTP status code listed in `refresh.on_status_codes` (default: `[401]`), the system SHALL execute the refresh flow (HTTP request + token extraction), persist the updated tokens, and retry the original upstream request exactly once with the refreshed credentials.

#### Scenario: Upstream returns 401 and refresh succeeds
- **WHEN** an upstream provider API call (channel list or stream resolution) returns HTTP 401 and the provider has an `auth.refresh` config
- **THEN** the system executes the refresh request, extracts new token values via the configured JSONPath mappings, persists them to the provider session, and retries the original upstream call with updated tokens

#### Scenario: Upstream returns 401 and refresh fails
- **WHEN** an upstream provider API call returns HTTP 401 and the subsequent refresh request fails (network error, non-success status, or extraction yields no values)
- **THEN** the system returns the original upstream error to the caller without further retry and logs a warning

#### Scenario: Upstream returns a non-refresh status code
- **WHEN** an upstream provider API call returns an HTTP status code that is not in the provider's `refresh.on_status_codes` list
- **THEN** the system returns the upstream response as-is without attempting a refresh

#### Scenario: Retry after refresh fails with same error
- **WHEN** the retry attempt after a successful refresh also returns a status code in `on_status_codes`
- **THEN** the system returns the retry's error response without triggering another refresh (no infinite loop)

### Requirement: Refresh config on_status_codes SHALL be configurable per provider
The `RefreshConfig` schema SHALL include an `on_status_codes` field (list of HTTP status codes, default `[401]`) that determines which upstream response codes trigger automatic refresh. Providers MAY override this default.

#### Scenario: Provider overrides on_status_codes
- **WHEN** a provider YAML sets `refresh.on_status_codes: [401, 403]`
- **THEN** both HTTP 401 and HTTP 403 upstream responses trigger automatic token refresh

#### Scenario: Provider uses default on_status_codes
- **WHEN** a provider YAML defines `auth.refresh` without specifying `on_status_codes`
- **THEN** only HTTP 401 upstream responses trigger automatic token refresh

### Requirement: Concurrent refresh attempts for the same session SHALL be serialized
When multiple concurrent upstream requests for the same provider session all encounter a refresh-triggering status code, the system SHALL execute the refresh flow at most once. Subsequent waiters SHALL use the refreshed tokens without re-executing the refresh.

#### Scenario: Two concurrent requests both get 401
- **WHEN** two concurrent upstream requests for the same provider+user session both return HTTP 401
- **THEN** only one refresh request is sent to the upstream refresh endpoint, and both original requests are retried with the same refreshed tokens

#### Scenario: Concurrent requests for different sessions refresh independently
- **WHEN** two concurrent upstream requests for different user sessions of the same provider both return HTTP 401
- **THEN** each session executes its own independent refresh flow

### Requirement: Refresh SHALL merge extracted values into existing stored values
When extracting tokens from a refresh response, the system SHALL merge only the extracted key-value pairs into the existing stored session values. Keys not present in the refresh response's `on_success.extract` mapping SHALL remain unchanged in the stored session.

#### Scenario: Refresh updates access_token but preserves refresh_token
- **WHEN** a refresh response extracts `access_token: "$.authToken"` and the stored session also contains `refresh_token`, `device_id`, and `crm`
- **THEN** only `access_token` is updated; `refresh_token`, `device_id`, and `crm` retain their prior values

### Requirement: Providers with auth.refresh config SHALL load successfully
The server SHALL accept and load provider YAML files that define an `auth.refresh` block. The validation guard that previously rejected such providers SHALL be removed.

#### Scenario: Provider with refresh config loads at startup
- **WHEN** the server starts with a provider YAML that includes an `auth.refresh` block
- **THEN** the provider is loaded into the active provider map without error

#### Scenario: Provider with refresh config loads during hot-reload
- **WHEN** a provider YAML with an `auth.refresh` block is added or modified while the server is running
- **THEN** the hot-reload mechanism loads the provider successfully

### Requirement: Stale proxy contexts SHALL be invalidated after token refresh
After a successful token refresh, the system SHALL invalidate proxy context entries whose resolved headers contain tokens from the refreshed session, so that subsequent stream requests create fresh proxy contexts with updated credentials.

#### Scenario: Active stream proxy context is invalidated after refresh
- **WHEN** a token refresh succeeds for a provider session that has active proxy context entries
- **THEN** those proxy context entries are removed from the cache, and subsequent proxy requests for the same stream require a new stream resolution (which creates a fresh proxy context with updated tokens)
