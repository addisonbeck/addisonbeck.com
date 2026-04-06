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

# Development: full build then preview server with export cache watcher
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    just build
    cd site && npm run preview &
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
