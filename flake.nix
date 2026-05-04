{
  description = "OpenHome API development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            cargo-watch
            clippy
	    just
            openssl
            pkg-config
            rust-analyzer
            rustc
            rustfmt
            sqlite
            sqlx-cli
          ];

          shellHook = ''
            repo_root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
            export DATABASE_URL="sqlite:$repo_root/api/data/app.db"
            echo "OpenHome API shell"
            echo "Run API commands from repo root, e.g. cargo test --manifest-path api/Cargo.toml"
          '';
        };
      });
}
