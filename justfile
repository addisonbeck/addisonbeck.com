# addisonbeck.com build orchestration
# Run `just --list` to see all recipes

# Default: show available commands
default:
    @just --list

# Render org-roam export to HTML fragments
render:
    cargo run --release --manifest-path renderer/Cargo.toml -- \
        --input export-cache \
        --output rendered

# Astro build + Pagefind index (assumes render has already run)
site-build:
    cd site && npm run build

# Build full site: render then Astro build
build: render
    cd site && npm install && npm run build

# Update all dependencies: nix flake, npm, cargo
update-deps:
    nix flake update
    cd site && npm upgrade --save-exact && npm install
    cargo update --manifest-path renderer/Cargo.toml

# Upgrade all dependencies to absolute latest, including major version bumps
upgrade-deps:
    nix flake update
    cd site && npx --yes npm-check-updates --upgrade && npm install
    cargo update --manifest-path renderer/Cargo.toml

# Development: render then Astro dev server with file watchers
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    just build
    (fswatch -o ~/.cache/org-roam-export/ | while read -r; do rsync -a --delete ~/.cache/org-roam-export/ export-cache/ && just render && just site-build; done) &
    CACHE_PID=$!
    (fswatch -o renderer/src/ | while read -r; do just render && just site-build; done) &
    RENDERER_PID=$!
    trap "kill $CACHE_PID $RENDERER_PID 2>/dev/null" EXIT
    cd site && npm run dev -- --host
