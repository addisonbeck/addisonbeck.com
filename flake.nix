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
        devShells.default = pkgs.mkShell {
          name = "addisonbeck-devshell";
          buildInputs = with pkgs; [
            rustToolchain
            nodejs
            just
            rsync
            fswatch
          ];
          shellHook = ''
            echo "addisonbeck.com devshell"
            echo "Rust: $(rustc --version)"
            echo "Node: $(node --version)"
            just --list 2>/dev/null || true
          '';
        };
      });
}
