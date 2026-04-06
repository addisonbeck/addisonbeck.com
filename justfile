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

# Build full site: render then Astro build
build: render
    cd site && npm run build

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

# Kill any running preview servers
kill-preview:
    @pkill -f "astro preview" 2>/dev/null || true

# Development: full build then preview server with export cache watcher
dev: kill-preview
    #!/usr/bin/env bash
    set -euo pipefail
    just build
    cd site && npm run preview -- --host &
    PREVIEW_PID=$!
    trap "kill $PREVIEW_PID 2>/dev/null" EXIT
    echo "Watching ~/.cache/org-roam-export for changes (Ctrl+C to stop)..."
    while true; do
        if command -v fswatch &>/dev/null; then
            fswatch -1 ~/.cache/org-roam-export/ && just build
        else
            sleep 30 && just build
        fi
    done
