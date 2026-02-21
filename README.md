# OTVI (Open Television Interface)

OTVI is a Rust web app that provides a **generic television interface driven by YAML provider files**.

It supports:
- loading **multiple YAML provider configs**,
- provider-specific **login / logout** APIs with custom headers and request bodies,
- provider-specific **browse channels** APIs,
- **play channel** flows for HLS / DASH (including DRM metadata pass-through),
- a server-rendered Rust UI for login, browse, play, and logout.

## Why YAML-driven?

Many TV providers already have working APIs in mobile or Android TV apps.
OTVI maps those existing flows into YAML to reduce implementation effort.

A provider only needs to describe API details (URLs, methods, headers, body templates, and response mappings), and OTVI handles the runtime orchestration.

## Run locally

```bash
cargo run
```

By default, provider files are loaded from `./providers`.
Override with:

```bash
OTVI_CONFIG_DIR=/path/to/yamls cargo run
```

Then open:

- http://localhost:3000

## YAML format

Each provider YAML contains:
- metadata (`id`, `name`, `description`),
- `login` action (including dynamic input fields and optional `auth_token_path`),
- optional `logout` action,
- `browse_channels` action + `response_mapping`,
- optional `play_channel` action + optional response mapping override.

### Important template variables

Request templates use Handlebars syntax:

- `{{session_id}}`
- `{{auth_token}}` (available after successful login when `auth_token_path` is configured)
- custom login fields (`{{identifier}}`, `{{password}}`, etc.)
- `{{channel_id}}` in play flow

### Response mapping paths

Mappings use slash-separated JSON pointer-like paths (for example `/data/channels/0/name`).

## Docker

```bash
docker build -t otvi .
docker run --rm -p 3000:3000 -v $(pwd)/providers:/app/providers otvi
```

## Notes on playback

The web player uses a basic `<video>` element.
Actual DASH + DRM playback may require a custom JavaScript player (for example Shaka or dash.js) and EME integration depending on provider DRM setup.

OTVI keeps DRM license URL available in config/mapping so providers can incrementally enhance frontend playback.
