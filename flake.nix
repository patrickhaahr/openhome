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

        android-cli = pkgs.stdenvNoCC.mkDerivation {
          pname = "android-cli";
          version = "latest";

          src = pkgs.fetchurl {
            url = "https://dl.google.com/android/cli/latest/linux_x86_64/android-cli";
            sha256 = "sha256-pX5XLaxz2ziLsa6UKrWt5gb8a9psHskA1X43Zd5DY0A=";
          };

          dontUnpack = true;
          nativeBuildInputs = [
            pkgs.autoPatchelfHook
            pkgs.makeWrapper
          ];
          buildInputs = [
            pkgs.glibc
          ];

          installPhase = ''
            runHook preInstall
            mkdir -p $out/bin
            mkdir -p $out/libexec
            cp $src $out/libexec/android
            chmod +x $out/libexec/android
            makeWrapper $out/libexec/android $out/bin/android \
              --set-default JAVA_TOOL_OPTIONS "-Djdk.lang.Process.launchMechanism=VFORK" \
              --add-flags "--no-metrics"
            runHook postInstall
          '';
        };
      in {
        packages.android-cli = android-cli;

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            android-cli
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
