## ADDED Requirements

### Requirement: Supported provider YAML behavior must be explicitly validated
The system MUST validate provider configuration against runtime-supported behavior at load time. Configuration fields that are not supported by the runtime MUST either be implemented as specified or rejected with actionable validation errors instead of being silently ignored.

#### Scenario: Unsupported provider field is rejected clearly
- **WHEN** a provider YAML file includes a field or behavior that is documented or present in schema but not supported by runtime execution
- **THEN** provider loading reports a clear validation error identifying the unsupported field and why it is rejected

#### Scenario: Supported provider field behaves consistently with documentation
- **WHEN** a provider YAML file uses a documented supported field
- **THEN** the runtime behavior matches the documented semantics for that field

### Requirement: API documentation must match implemented request and response shapes
The system MUST keep API reference documentation aligned with implemented request requirements, response payloads, and route behavior for provider auth, user management, admin actions, streaming, and channel browsing.

#### Scenario: Provider auth docs reflect actual payload contract
- **WHEN** a maintainer or integrator reads the provider authentication API reference
- **THEN** the documented request fields, intermediate-step behavior, and response field names match the implemented API behavior

#### Scenario: Admin and user docs reflect actual identifier and payload shapes
- **WHEN** a maintainer or integrator reads user-management or admin API documentation
- **THEN** the documented field names, identifier formats, and response bodies match the implemented API behavior

### Requirement: Frontend and architecture documentation must describe real runtime behavior
The system MUST document actual frontend routes, overlays, query-state handling, and data-flow behavior as implemented after this change, rather than describing stale or aspirational flows.

#### Scenario: Frontend guide reflects actual route and overlay model
- **WHEN** a maintainer reads the frontend guide
- **THEN** the guide accurately describes which experiences are routes, which are overlays, and how authentication and password-change flows are presented

#### Scenario: Architecture docs reflect the actual channel and playback flow
- **WHEN** a maintainer reads the architecture or introduction documentation
- **THEN** the described channel search, playback metadata, and proxy behavior match the implemented backend and frontend flow

### Requirement: Regression coverage must protect aligned contracts
The system MUST add or update automated tests so security boundaries, channel/playback contract behavior, and documented request/response shapes are protected against regression.

#### Scenario: Security and contract regressions are covered by tests
- **WHEN** future changes alter proxying, provider authorization, channel query behavior, or provider contract validation
- **THEN** automated tests fail if the change violates the agreed runtime contract
