## 1. Frontend UI Test Harness Setup

- [x] 1.1 Add and configure the Leptos-compatible frontend UI test harness and required dev dependencies under `web/`.
- [x] 1.2 Add deterministic test bootstrapping utilities for backend boot/auth responses and reusable test fixtures.
- [x] 1.3 Define and document the canonical local command to run the frontend UI test suite.

## 2. Core Route And Gate Coverage

- [x] 2.1 Implement UI tests that cover setup, login, forced password-change, and authenticated-shell boot outcomes.
- [x] 2.2 Implement UI tests that cover primary routed views: home, admin, provider channels, player, and not-found behavior.
- [x] 2.3 Implement UI tests validating protected navigation shell behavior (auth-dependent nav visibility and actions).

## 3. Navigation And Playback Contract Validation

- [x] 3.1 Implement UI tests that verify representative in-app route transitions preserve SPA navigation behavior without full-page reload outcomes.
- [x] 3.2 Implement UI tests that verify channel-to-player navigation renders expected playback context for selected channels.
- [x] 3.3 Ensure UI assertions use stable semantic selectors or equivalent resilient mechanisms instead of style-only selectors.

## 4. CI Integration, Quality Gates, And Verification

- [x] 4.1 Integrate frontend UI tests into CI so failed UI tests fail the workflow status.
- [x] 4.2 Update contributor/testing documentation with frontend UI test scope, execution steps, and troubleshooting notes.
- [x] 4.3 Run formatting, linting, and test commands (including the frontend UI suite) and resolve issues before completion.
