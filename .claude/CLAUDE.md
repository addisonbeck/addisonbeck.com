# addisonbeck.com - CLAUDE.md

## Overview

addisonbeck.com is a static site that renders an org-roam second brain as a public-facing web reader. A Rust binary converts org-element AST JSON exports into HTML fragments; Astro consumes those fragments to build a fully static site with backlink navigation and full-text search via Pagefind.

The org-roam export cache at `~/.cache/org-roam-export/` is produced by a separate Emacs batch export job and must be pre-populated before the build pipeline runs.

## Architecture & Patterns

### System Diagram

```mermaid
graph TD
    A[Emacs batch export] --> B[~/.cache/org-roam-export/]
    B --> C[just render]
    C --> D[Rust renderer binary]
    D --> E[rendered/index.json]
    D --> F[rendered/alias_map.json]
    D --> G[rendered/*.html]
    E --> H[just build]
    F --> H
    G --> H
    H --> I[Astro static build]
    I --> J[site/dist/]
    J --> K[Pagefind index]
    K --> L[site/dist/pagefind/]
    L --> M[rsync deploy]
```

### Build Pipeline

1. `nix develop` — enters the Nix devshell with all tools available (Rust, Node.js, just, rsync)
2. `just render` — runs `cargo run --release` in `renderer/`, reads `~/.cache/org-roam-export/` and writes `rendered/`
3. `just build` — runs `just render` then `npm run build` in `site/`, which produces `site/dist/` and runs Pagefind indexing

`just build` assumes the org-roam export cache exists. If `~/.cache/org-roam-export/manifest.json` is missing, the renderer will fail.

### Directory Structure

```
website-redesign/
├── renderer/           # Rust crate: org-element AST → HTML fragments
├── site/               # Astro project: static site generation
├── rendered/           # Build artifact (gitignored): renderer output
├── .github/
│   └── workflows/
│       └── deploy.yml  # CI/CD: Nix build + rsync deploy
├── rust-toolchain.toml # Pins Rust 1.94.1 stable
├── justfile            # Build orchestration commands
└── flake.nix           # Nix devshell definition
```

## Stack Best Practices

### Toolchain

- **Rust**: 1.94.1 stable, pinned via `rust-toolchain.toml`. Always use `nix develop` to enter the shell — do not rely on system Rust.
- **Astro**: Static output mode with `@astrojs/svelte` for islands and `astro-pagefind` for search indexing.
- **Package manager**: npm only. Do not use pnpm or yarn.
- **Nix devshell**: All development must happen inside `nix develop`. The shell provides Rust, Node.js, just, and rsync.

### Available Commands

```bash
just render   # Run Rust renderer: reads cache, writes rendered/
just build    # Full build: render + Astro build + Pagefind index
just dev      # Watch mode: re-renders on cache changes, serves Astro dev server
```

### GitHub Actions Secrets

The deploy workflow requires exactly these four secrets — do not rename them:

| Secret | Purpose |
|--------|---------|
| `DEPLOY_HOST` | SSH hostname of the server |
| `DEPLOY_USERNAME` | SSH login username |
| `DEPLOY_KEY_PRI` | Private SSH key (PEM format) |
| `DEPLOY_PATH` | Destination path on the server |

## Anti-Patterns

- **Never commit `rendered/`** — this is a build artifact regenerated from the org-roam cache.
- **Never commit `site/dist/`** — this is the Astro build output.
- **Do not change GitHub Actions secret names** — the workflow references `DEPLOY_HOST`, `DEPLOY_USERNAME`, `DEPLOY_KEY_PRI`, and `DEPLOY_PATH` exactly.
- **Do not use pnpm** — this project uses npm. Using pnpm will create a `pnpm-lock.yaml` and break the Nix build.
- **Do not run builds outside `nix develop`** — the Rust toolchain version is pinned and must come from the Nix shell.

## Data Models

### org-roam Export Format

Each org-roam node is exported as a JSON file at:
```
~/.cache/org-roam-export/<shard>/<UUID>.json
```

Where `<shard>` is the first two characters of the UUID (e.g., `ab/abcd1234-...json`).

A `manifest.json` at the cache root lists all exported nodes:
```json
[
  { "id": "UUID", "file": "shard/UUID.json" }
]
```

Node JSON fields: `id`, `title`, `tags`, `aliases`, `links_to`, `linked_from`, `ast` (org-element AST as JSON), `point`, `level`.

Only nodes tagged `public` are included in the export.

### Renderer Output

The renderer writes to `rendered/`:
- `rendered/index.json` — array of `IndexEntry` objects (id, title, slug, aliases, tags, backlinks, last_modified)
- `rendered/alias_map.json` — map of alias slug → canonical slug
- `rendered/<UUID>.html` — pre-rendered HTML fragment for each node

## Configuration, Security, and Authentication

### Deployment

Deployment is triggered by pushing to `main`. GitHub Actions runs `nix develop --command just build` and rsyncs `site/dist/` to the configured Mail-in-a-Box server via SSH.

The SSH private key (`DEPLOY_KEY_PRI`) must be added to the server's `~/.ssh/authorized_keys` before deployment will succeed.

### Nix Devshell

```bash
nix develop       # Enter devshell
direnv allow      # If .envrc is configured, auto-enter on cd
```

The devshell uses `rust-overlay` to provide the exact Rust version from `rust-toolchain.toml`.
