## ADDED Requirements

### Requirement: Frontend UI tests MUST cover all primary Leptos application views and route outcomes
The system MUST provide automated frontend UI tests for the Leptos app that validate rendering and user-visible behavior for all primary route outcomes: setup, login, forced password change, authenticated home, admin route access, provider channels, channel playback, and not-found handling.

#### Scenario: Primary route outcomes are covered by automated tests
- **WHEN** frontend UI tests are executed
- **THEN** the suite includes at least one passing scenario for each primary application view and route outcome defined by the app router and boot-state gates

### Requirement: Frontend UI tests MUST verify auth and boot-state gating behavior
The system MUST include automated UI tests that validate the boot check and authentication gating behavior so users only see setup, login, forced password-change, or authenticated shell states that match backend boot responses.

#### Scenario: Boot-state response drives visible gate
- **WHEN** a deterministic test setup provides each supported boot-state response
- **THEN** the rendered UI shows only the corresponding gate or authenticated shell for that response

### Requirement: Frontend UI tests MUST use deterministic setup and stable assertions
The frontend UI test framework MUST use deterministic fixtures/mocks and stable, semantic selectors or equivalent resilient assertions so tests remain reliable across cosmetic UI changes.

#### Scenario: Test reliability is maintained across non-functional style changes
- **WHEN** visual classes or non-semantic styling details change without changing behavior
- **THEN** existing UI tests continue to pass without requiring selector rewrites tied only to presentation classes

### Requirement: Frontend UI tests MUST be runnable in local and CI workflows
The project MUST define documented command(s) for running frontend UI tests in both local developer environments and CI pipelines, and CI MUST fail when required UI tests fail.

#### Scenario: CI enforces frontend UI test pass criteria
- **WHEN** a pull request or merge workflow runs frontend UI tests
- **THEN** the workflow reports failure and blocks success status if any required frontend UI test fails
