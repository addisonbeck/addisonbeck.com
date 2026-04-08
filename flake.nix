{
  description = "addisonbeck.com static site";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    treefmt-nix,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {inherit system overlays;};
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

      # Wrapper that calls the project-local prettier so plugins (prettier-plugin-astro,
      # prettier-plugin-svelte) resolve from site/node_modules rather than the nix store.
      # Auto-installs npm deps on first run so `nix fmt` works without entering nix develop.
      sitePrettier = pkgs.writeShellScriptBin "site-prettier" ''
        ROOT="$(git rev-parse --show-toplevel)"
        if [ ! -f "$ROOT/site/node_modules/.bin/prettier" ]; then
          echo "site-prettier: installing npm deps in site/..." >&2
          (cd "$ROOT/site" && ${pkgs.nodejs_24}/bin/npm install --silent)
        fi
        # treefmt passes paths relative to the project root. Absolutize non-flag
        # args before cd-ing into site/ so prettier can find both the files
        # (via absolute paths) and the plugins (via site/node_modules).
        ARGS=()
        for arg in "$@"; do
          case "$arg" in
            -*) ARGS+=("$arg") ;;
            *)  ARGS+=("$ROOT/$arg") ;;
          esac
        done
        cd "$ROOT/site"
        exec "$ROOT/site/node_modules/.bin/prettier" "''${ARGS[@]}"
      '';

      treefmtEval = treefmt-nix.lib.evalModule pkgs {
        # Nix formatter
        programs.alejandra.enable = true;

        # Rust formatter — uses `rustfmt` from PATH so the rust-overlay toolchain
        # version is used rather than a hardcoded nixpkgs rustfmt.
        settings.formatter.rustfmt = {
          command = "rustfmt";
          options = ["--edition" "2021"];
          includes = ["*.rs"];
        };

        # Web formatter — delegates to the project-local prettier so astro/svelte
        # plugins are available.
        settings.formatter.prettier = {
          command = "${sitePrettier}/bin/site-prettier";
          options = ["--write"];
          includes = ["*.astro" "*.svelte" "*.ts" "*.mjs" "*.css" "*.json"];
        };

        # Exclude generated/cached artifacts and lockfiles from formatting.
        settings.global.excludes = [
          "export-cache/**"
          "rendered/**"
          "target/**"
          "site/dist/**"
          "site/node_modules/**"
          "*.lock"
          "flake.lock"
          "package-lock.json"
        ];
      };
    in {
      packages.org-roam-export-el = pkgs.stdenv.mkDerivation {
        name = "org-roam-export-el";
        src = ./scripts/org-roam-export.el;
        phases = ["installPhase"];
        installPhase = "cp $src $out";
      };

      # Expose as `nix fmt` target.
      formatter = treefmtEval.config.build.wrapper;

      devShells.default = pkgs.mkShell {
        name = "addisonbeck-devshell";
        buildInputs = with pkgs; [
          rustToolchain
          nodejs_24
          just
          rsync
          fswatch
          libwebp
          poppler-utils
          treefmtEval.config.build.wrapper
        ];
        shellHook = ''
                                echo "addisonbeck.com devshell"
                                echo "Rust: $(rustc --version)"
                                echo "Node: $(node --version)"
                                export PATH="$PWD/scripts:$PATH"
                                if [ -d "$HOME/.cache/org-roam-export" ]; then
                                  echo "Syncing org-roam export cache → export-cache/..."
                                  rsync -a --delete "$HOME/.cache/org-roam-export/" export-cache/
                                  echo "Export cache synced."
                                else
                                  echo "Warning: ~/.cache/org-roam-export not found — export-cache/ may be stale"
                                fi
                                # Ensure site node_modules are present so prettier plugins resolve correctly.
                                if [ ! -d site/node_modules ]; then
                                  echo "Installing site npm dependencies..."
                                  (cd site && npm install)
                                fi
                                # Install pre-commit hook with the absolute nix store path to treefmt
                                # so it works outside the devshell (git hooks run in a plain shell).
                                if [ -d .git/hooks ]; then
                                  cat > .git/hooks/pre-commit << 'HOOK'
          #!/usr/bin/env bash
          set -euo pipefail
          ${treefmtEval.config.build.wrapper}/bin/treefmt
          git add -u
          HOOK
                                  chmod +x .git/hooks/pre-commit
                                fi
                                just --list 2>/dev/null || true
        '';
      };
    });
}
