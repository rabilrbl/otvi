## Why

OTVI is functionally promising, but its current implementation has drifted across runtime behavior, frontend assumptions, provider contracts, and documentation. Internal testing gives us a safe window to harden security boundaries, remove inefficient flows, and make the platform's actual behavior consistent before wider usage turns today's shortcuts into long-term constraints.

## What Changes

- Lock down provider-facing security boundaries, especially stream proxying, provider access enforcement, cookie/session isolation, and error/log redaction.
- Rationalize channel, playback, and frontend data flows so search/filter behavior, navigation, and metadata loading happen in one well-defined layer instead of being duplicated across server and client.
- Tighten provider contract handling so documented YAML and API behavior matches what the runtime actually supports, including validation of unsupported or schema-only features.
- Restructure high-complexity modules into clearer service and UI boundaries to reduce handler/page sprawl and make future changes safer.
- Rewrite the most misleading documentation so maintainer and provider-authoring guidance reflects the implemented system rather than stale or aspirational behavior.

## Capabilities

### New Capabilities
- `secure-provider-boundaries`: Constrain proxying, credential propagation, provider authorization, and error exposure so provider integrations cannot bypass core security expectations.
- `coherent-channel-and-playback-flow`: Define a single, consistent contract for channel search, filtering, pagination, playback metadata resolution, and navigation behavior across backend and frontend.
- `aligned-runtime-contracts`: Define and document the supported provider YAML, API, and frontend behavior so runtime, schema, tests, and docs share the same source of truth.

### Modified Capabilities

- None.

## Impact

- Affected backend code: `crates/otvi-server/src/api/proxy.rs`, `crates/otvi-server/src/api/channels.rs`, `crates/otvi-server/src/api/auth.rs`, `crates/otvi-server/src/state.rs`, `crates/otvi-server/src/provider_client.rs`, `crates/otvi-server/src/error.rs`
- Affected frontend code: `web/src/app.rs`, `web/src/api.rs`, `web/src/pages/channels.rs`, `web/src/pages/player.rs`, `web/src/pages/admin.rs`
- Affected docs: `docs/docs/frontend.md`, `docs/docs/architecture.md`, `docs/docs/introduction.md`, provider guides, and API reference pages
- Affected tests: backend integration/security coverage, frontend behavior checks where possible, and contract-focused regression tests for provider configuration handling
