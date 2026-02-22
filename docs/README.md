# OTVI Documentation

This directory contains the OTVI documentation site, built with [Docusaurus](https://docusaurus.io/).

## Prerequisites

- [Bun](https://bun.sh/) (recommended) or Node.js 20+

## Development

```bash
# Install dependencies
bun install

# Start the development server
bun start

# Build for production
bun run build

# Serve the production build locally
bun run serve
```

## Project Structure

```
docs/
├── docs/                  # Markdown documentation pages
│   ├── introduction.md
│   ├── getting-started.md
│   ├── architecture.md
│   ├── configuration.md
│   ├── providers/         # Provider guide
│   ├── api-reference/     # API reference
│   ├── frontend.md
│   ├── deployment.md
│   └── admin-guide.md
├── src/
│   ├── pages/             # Custom pages (landing page)
│   └── css/               # Custom styles
├── docusaurus.config.ts   # Docusaurus configuration
├── sidebars.ts            # Sidebar navigation
└── package.json
```
