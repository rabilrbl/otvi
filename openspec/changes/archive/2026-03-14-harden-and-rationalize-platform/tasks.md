## 1. Secure provider boundaries

- [x] 1.1 Refactor `crates/otvi-server/src/api/proxy.rs` and related stream-resolution code so proxy requests require server-issued context and cannot be used as an open fetch endpoint.
- [x] 1.2 Introduce shared provider authorization checks used by auth, channel, category, and stream handlers, covering per-user allow-lists and global-provider admin restrictions.
- [x] 1.3 Remove or isolate shared outbound cookie state in provider HTTP execution so provider session data cannot leak across users or providers.
- [x] 1.4 Sanitize backend error responses and routine logging paths so raw upstream bodies, internal exception strings, headers, and cookies are not exposed.

## 2. Rationalize channel and playback flow

- [x] 2.1 Make backend channel query behavior the single contract for search, category filtering, pagination, and totals, and remove conflicting duplicate filtering logic.
- [x] 2.2 Update `web/src/pages/channels.rs` and related API helpers so search and filter state are URL-driven, bookmarkable, and rendered directly from backend results.
- [x] 2.3 Rework playback metadata resolution so `web/src/pages/player.rs` no longer fetches the full channel list just to resolve one channel's display data.
- [x] 2.4 Replace plain internal anchors with route-aware navigation in the frontend shell and pages to preserve SPA behavior.

## 3. Align runtime contracts and validation

- [x] 3.1 Audit provider schema fields against runtime support and either implement or reject unsupported documented behavior during provider load.
- [x] 3.2 Unify response-path handling and provider contract interpretation so channel/category extraction and related config behavior match documented semantics.
- [x] 3.3 Update example providers, fixtures, and request/response types as needed to reflect the supported contract after validation changes.

## 4. Rewrite docs from runtime truth

- [x] 4.1 Rewrite high-drift frontend and architecture docs to reflect actual routes, overlays, query-state behavior, playback metadata flow, and navigation.
- [x] 4.2 Rewrite affected API reference and provider-authoring docs so payload shapes, field names, and supported YAML behavior match implementation.

## 5. Verify and harden with tests

- [x] 5.1 Add or update backend tests for proxy rejection/allow paths, provider authorization enforcement, and provider-config validation failures.
- [x] 5.2 Add or update tests for channel query semantics and playback metadata behavior so frontend/backend contract drift is caught.
- [x] 5.3 Run formatting, clippy, unit tests, and integration tests required for the touched code, then fix any regressions before merge.
