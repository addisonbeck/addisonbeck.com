{
  description = "addisonbeck.com static site";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in {
        packages.org-roam-export-el = pkgs.stdenv.mkDerivation {
          name = "org-roam-export-el";
          src = ./scripts/org-roam-export.el;
          phases = [ "installPhase" ];
          installPhase = "cp $src $out";
        };
        devShells.default = pkgs.mkShell {
          name = "addisonbeck-devshell";
          buildInputs = with pkgs; [
            rustToolchain
            nodejs_24
            just
            rsync
            fswatch
            libwebp
            poppler_utils
          ];
          shellHook = ''
            echo "addisonbeck.com devshell"
            echo "Rust: $(rustc --version)"
            echo "Node: $(node --version)"
            if [ -d "$HOME/.cache/org-roam-export" ]; then
              echo "Syncing org-roam export cache → export-cache/..."
              rsync -a --delete "$HOME/.cache/org-roam-export/" export-cache/
              echo "Export cache synced."
            else
              echo "Warning: ~/.cache/org-roam-export not found — export-cache/ may be stale"
            fi
            just --list 2>/dev/null || true
          '';
        };
      });
}
