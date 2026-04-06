# addisonbeck.com build orchestration
# Run `just --list` to see all recipes

# Default: show available commands
default:
    @just --list

# Render org-roam export to HTML fragments
render:
    cargo run --release --manifest-path renderer/Cargo.toml -- \
        --input ~/.cache/org-roam-export \
        --output rendered

# Build full site: render then Astro build
build: render
    cd site && npm run build

# Development: watch mode
# Reruns renderer when export cache changes, then triggers Astro dev server
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Starting Astro dev server..."
    cd site && npm run dev &
    ASTRO_PID=$!
    trap "kill $ASTRO_PID 2>/dev/null" EXIT
    echo "Watching ~/.cache/org-roam-export for changes..."
    while true; do
        if command -v fswatch &>/dev/null; then
            fswatch -1 ~/.cache/org-roam-export/ && just render
        else
            sleep 30 && just render
        fi
    done
