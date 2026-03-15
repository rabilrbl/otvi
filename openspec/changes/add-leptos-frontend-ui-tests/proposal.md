## Why

The Leptos frontend currently has no UI-focused automated tests, so regressions in routing, auth-gated flows, and page rendering can ship unnoticed. Adding consistent frontend UI test coverage now will reduce breakages as the UI and provider integrations evolve.

## What Changes

- Add a frontend UI testing capability for the Leptos app with a standard test harness and project-wide conventions.
- Define required UI test coverage for core routes and user-visible states (boot/auth overlays, navigation shell, page-level rendering, and route fallbacks).
- Add CI- and local-friendly test execution requirements for frontend UI tests.
- Document baseline expectations for maintainable UI tests (stable selectors/assertions and deterministic setup).

## Capabilities

### New Capabilities
- `leptos-frontend-ui-tests`: Defines required automated UI test coverage, structure, and execution behavior for the Leptos frontend.

### Modified Capabilities
- `coherent-channel-and-playback-flow`: Clarifies that SPA navigation and channel/playback route behavior must be verified by frontend UI tests.

## Impact

- Affected code: `web/` frontend codebase, including app/router flows and page components.
- Tooling/dependencies: frontend test harness and supporting dev/test dependencies for Leptos UI tests.
- Delivery process: CI pipeline and local developer workflow gain required frontend UI test execution.
