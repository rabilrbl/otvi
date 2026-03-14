## Context

OTVI currently succeeds at basic end-to-end behavior, but several core boundaries are blurred: proxying trusts caller-supplied URLs too broadly, provider authorization is enforced inconsistently across endpoints, channel and playback behavior is split across backend and frontend, and the documented provider contract no longer matches runtime behavior in multiple places. Because the project is still in internal testing, we can correct these issues with broad refactors now rather than preserving accidental behavior for compatibility.

This change spans backend security, provider integration rules, frontend routing and data flow, and project documentation. It also needs to preserve the project's current stack and broad product direction: Axum server, Leptos frontend, YAML-driven providers, and SQLx-backed user/session management.

## Goals / Non-Goals

**Goals:**
- Enforce provider security boundaries centrally, especially for stream proxying, provider access checks, cookie/session isolation, and exposed error details.
- Make channel browsing and playback behavior coherent across backend and frontend so search, filtering, pagination, metadata resolution, and navigation use one consistent contract.
- Align supported YAML, API, and frontend behavior with what the runtime actually implements, with explicit validation or removal of unsupported features.
- Break down the highest-risk modules into clearer service boundaries without changing the platform stack.
- Establish regression coverage for the newly enforced security and contract behavior.

**Non-Goals:**
- Replacing Axum, Leptos, SQLx, or the YAML-driven provider model.
- Designing a new product surface or major UI redesign beyond changes required to support coherent flows.
- Adding new provider capabilities unrelated to hardening, performance, or contract alignment.
- Preserving undocumented or unsafe behavior solely for backward compatibility during internal testing.

## Decisions

### 1. Proxy access will be converted from open fetching to server-issued constrained access

The stream proxy will only serve requests that can be tied to a server-issued context token created by the stream endpoint, rather than accepting arbitrary caller-supplied upstream URLs without trusted context.

Rationale:
- This removes the current SSRF/open-proxy shape.
- It keeps proxy behavior aligned with the existing architecture, where the stream endpoint already constructs proxied URLs.
- It allows the server to carry resolved headers, cookie forwarding rules, and manifest-derived state without trusting the browser to define them.

Alternatives considered:
- Add a host allow-list while still accepting arbitrary proxy URLs: better than today, but still leaves too much caller control and validation complexity.
- Require authentication on `/api/proxy` only: insufficient, because authenticated arbitrary upstream fetching is still unsafe.

### 2. Provider authorization will be enforced through shared backend policy, not per-handler drift

All provider-bound routes will use a shared authorization path that checks provider existence, auth scope, and per-user provider access consistently before auth, channel, category, or stream operations proceed.

Rationale:
- Current drift exists because handlers own too much policy logic.
- Centralized policy reduces missed checks and makes integration tests more meaningful.

Alternatives considered:
- Patch each handler independently: fast, but likely to drift again.
- Move everything into middleware keyed only by route parameters: less explicit and harder to reuse in tests than a dedicated service/helper layer.

### 3. Outbound provider state will be isolated from the global shared HTTP client

The system will stop relying on a globally shared cookie store for provider requests. Shared client configuration may remain for connection reuse and defaults, but cookie/session state must be isolated per logical provider context.

Rationale:
- Shared cookie state across users or providers is a correctness and security risk.
- Session state already has an explicit database and proxy-context model; the HTTP client should not silently become another state store.

Alternatives considered:
- Keep the shared cookie jar and try to clear it aggressively: fragile and easy to get wrong.
- Create one fully separate client per request with all configuration duplicated: safer, but unnecessarily expensive if common transport settings can still be reused cleanly.

### 4. Channel search and playback metadata resolution will have one primary source of truth

Channel search, filtering, pagination, and playback metadata resolution will be defined by the backend contract and consumed directly by the frontend, instead of combining server-side filtering with additional client-side filtering and list re-fetches.

Rationale:
- The backend already owns provider fetches, caching, and pagination semantics.
- The frontend should reflect query state and render results, not reinterpret the dataset independently.
- This avoids fetching the full channel list simply to render player metadata.

Alternatives considered:
- Move all filtering fully client-side: simpler UI code in the short term, but scales poorly and contradicts the API shape.
- Keep hybrid behavior: preserves drift and inconsistent totals/bookmarking behavior.

### 5. Runtime-supported provider contract will be made explicit and validated at load time

Fields that are documented or present in schema but not implemented in runtime behavior will be either implemented as part of this change or rejected/removed so the platform has an explicit supported contract.

Rationale:
- Schema-only features create false confidence for provider authors.
- Load-time validation is the earliest and least ambiguous place to reject unsupported behavior.

Alternatives considered:
- Leave unsupported fields documented as "future": still encourages broken configs.
- Silently ignore unsupported fields: current behavior and a source of confusion.

### 6. Documentation will be rewritten from runtime truth, not from intended architecture

High-drift docs will be updated only after the target runtime behavior is decided, and docs will describe actual routes, actual response shapes, and actual provider configuration support.

Rationale:
- The current duplication problem came from multiple pages narrating desired behavior rather than verified behavior.
- Rewriting from runtime truth improves maintainability and onboarding.

Alternatives considered:
- Patch examples only: not enough, because several conceptual sections are wrong.

## Risks / Trade-offs

- [Stricter proxy constraints may break existing internal test providers] -> Mitigation: update stream URL generation and add focused integration coverage for proxied manifests, segments, and key requests before rollout.
- [Centralized provider authorization may surface currently hidden access bugs] -> Mitigation: add route-by-route authorization tests and treat newly failing cases as intended corrections.
- [Removing hybrid client/server channel behavior may change UX details] -> Mitigation: keep URL-backed query state explicit and align frontend behavior with documented backend semantics.
- [Load-time validation may reject previously tolerated provider YAML] -> Mitigation: document unsupported fields, add clear validation messages, and update bundled/example providers in the same change.
- [Splitting large modules increases short-term churn] -> Mitigation: refactor around explicit boundaries and retain test coverage at each step rather than attempting one large rewrite without checkpoints.

## Migration Plan

- Implement security and contract checks behind the existing internal testing workflow first, with regression tests covering proxying, provider authorization, and channel/playback flows.
- Update bundled providers, test fixtures, and documentation in the same change so the repository remains self-consistent at every merge point.
- Roll out as a single internal-only compatibility break: unsafe proxy URLs, unsupported YAML fields, and stale frontend assumptions may stop working, but that is acceptable in this testing phase.
- If rollback is required, revert the change set as a unit; partial rollback would reintroduce contract drift between backend, frontend, and docs.

## Open Questions

- Should playback metadata come from a dedicated lightweight endpoint or from enriched stream responses when channel identity is already known?
- Which currently documented schema-only fields, if any, are valuable enough to implement now instead of deprecating?
- How much frontend route guarding should be enforced at the router layer versus per-page data loaders once auth handling is centralized?
