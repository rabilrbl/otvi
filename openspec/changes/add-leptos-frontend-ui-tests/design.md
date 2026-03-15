## Context

The Leptos frontend under `web/` has several critical user-visible flows (boot state gating, setup/login/password overlays, route transitions, channel and player navigation), but there is no dedicated UI test layer that validates these behaviors end-to-end from the rendered app perspective. Existing requirements already define expected channel/playback and SPA navigation behavior, but the project lacks a standardized way to assert those expectations in automated frontend tests.

The project uses Rust/Cargo for the frontend codebase, with Leptos and Trunk. The design must fit this toolchain, support deterministic execution in CI, and avoid introducing fragile assertions tied to styling details.

## Goals / Non-Goals

**Goals:**
- Define a maintainable UI testing architecture for Leptos frontend behaviors in `web/`.
- Ensure core routes and auth/boot gating states are covered by automated UI tests.
- Ensure navigation and playback route expectations from existing specs are validated by tests.
- Provide deterministic local and CI execution with clear commands and pass/fail behavior.

**Non-Goals:**
- Replacing all unit tests or backend integration tests with UI tests.
- Exhaustive visual regression testing or screenshot baseline management.
- Refactoring unrelated frontend components beyond what is needed to make UI tests reliable.

## Decisions

- Adopt a browser-level UI test harness for the Leptos app, with headless execution as the default test mode.
  - Rationale: Route transitions, overlays, and rendered page behavior are best validated through actual browser interactions instead of only component-level tests.
  - Alternative considered: pure Rust unit/component tests only. Rejected because they do not reliably validate router behavior and full rendered flows.

- Define a required coverage matrix centered on user-critical flows rather than implementation internals.
  - Required areas: boot/auth state transitions, top-level route rendering, not-found behavior, protected navigation shell behavior, channel-to-player navigation pathways.
  - Rationale: This captures regressions users experience while keeping the suite focused and maintainable.
  - Alternative considered: broad "test every component" mandate. Rejected because it creates high maintenance cost with weak signal.

- Require stable, semantic test selectors and deterministic fixtures for UI assertions.
  - Rationale: Tests must be resilient to cosmetic changes and asynchronous rendering timing.
  - Alternative considered: CSS-class or text-fragment-only selectors. Rejected due to fragility.

- Integrate UI tests into both local development and CI through a documented, single-source command path.
  - Rationale: A test strategy is only effective if it runs consistently before merges.
  - Alternative considered: optional/manual UI test runs. Rejected because critical regressions would remain undetected.

## Risks / Trade-offs

- [Risk] Browser-level tests can be slower than unit tests. -> Mitigation: keep the required suite focused on critical flows and run additional coverage incrementally.
- [Risk] Async boot checks can cause flaky timing behavior. -> Mitigation: require deterministic app state setup and explicit wait conditions for route and overlay assertions.
- [Risk] Test infrastructure complexity increases frontend maintenance overhead. -> Mitigation: standardize test helpers, fixtures, and command entry points.
- [Risk] CI environments may differ from local browser/runtime behavior. -> Mitigation: run in pinned headless configuration with consistent dependency versions.
