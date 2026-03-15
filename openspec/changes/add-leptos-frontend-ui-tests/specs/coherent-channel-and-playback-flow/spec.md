## MODIFIED Requirements

### Requirement: Internal navigation must preserve single-page application behavior
The frontend MUST use route-aware internal navigation for in-app transitions so navigation between home, admin, channels, login, and player views does not trigger unnecessary full-page reloads. The frontend UI test suite MUST include automated scenarios that verify this in-app route behavior for representative route transitions.

#### Scenario: In-app navigation keeps application stateful shell active
- **WHEN** a user navigates using in-app links between application routes
- **THEN** the frontend transitions routes without a full document reload and preserves application shell behavior expected from the SPA

#### Scenario: SPA navigation behavior is validated by automated frontend UI tests
- **WHEN** the frontend UI test suite runs
- **THEN** it verifies representative in-app route transitions complete through SPA routing behavior without full-document reload outcomes
