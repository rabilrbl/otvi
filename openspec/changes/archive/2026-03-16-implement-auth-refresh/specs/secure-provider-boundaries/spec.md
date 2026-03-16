## MODIFIED Requirements

### Requirement: Provider session state must not leak across users or providers
The system MUST isolate outbound provider session state so cookies, headers, and credential-derived context for one user or provider cannot be reused implicitly by another user or provider through shared HTTP client state. Token refresh operations MUST respect the same session isolation: a per-user refresh SHALL only update that user's stored tokens, and a global-scope refresh SHALL use the shared session without affecting per-user sessions.

#### Scenario: Session state from one user does not affect another user's provider request
- **WHEN** one user authenticates with a per-user provider and another user accesses the same provider without authenticating
- **THEN** the second user's request does not inherit cookies or session state from the first user

#### Scenario: Session state from one provider does not affect another provider
- **WHEN** one provider interaction stores cookies or request state for an upstream domain
- **THEN** requests for a different provider do not reuse that stored state unless it was explicitly created for that provider context

#### Scenario: Per-user refresh does not update another user's session
- **WHEN** a token refresh executes for user A's session with a per-user provider
- **THEN** user B's stored tokens for the same provider remain unchanged

#### Scenario: Global-scope refresh updates the shared session
- **WHEN** a token refresh executes for a global-scope provider
- **THEN** the refresh updates the single shared session used by all users, and subsequent requests from any user use the refreshed tokens
