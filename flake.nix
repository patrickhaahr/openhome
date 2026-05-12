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

        android-cli-upstream = pkgs.stdenvNoCC.mkDerivation {
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

        android-cli = pkgs.writeShellApplication {
          name = "android";
          runtimeInputs = [
            android-cli-upstream
            pkgs.android-tools
            pkgs.coreutils
            pkgs.gnused
            pkgs.gnugrep
          ];
          text = ''
            sdk_arg=""
            args=()

            while [ "$#" -gt 0 ]; do
              case "$1" in
                --sdk)
                  if [ "$#" -lt 2 ]; then
                    break
                  fi
                  sdk_arg="$2"
                  args+=("$1" "$2")
                  shift 2
                  ;;
                --sdk=*)
                  sdk_arg="''${1#--sdk=}"
                  args+=("$1")
                  shift
                  ;;
                *)
                  args+=("$1")
                  shift
                  ;;
              esac
            done

            sdk_root="$sdk_arg"
            if [ -z "$sdk_root" ] && [ -n "''${ANDROID_SDK_ROOT:-}" ]; then
              sdk_root="$ANDROID_SDK_ROOT"
            fi
            if [ -z "$sdk_root" ] && [ -n "''${ANDROID_HOME:-}" ]; then
              sdk_root="$ANDROID_HOME"
            fi
            if [ -z "$sdk_root" ] && [ -f local.properties ]; then
              sdk_root=$(sed -n 's/^sdk\.dir=//p' local.properties | head -n 1)
            fi

            if [ -n "$sdk_root" ] && [ -d "$sdk_root" ]; then
              shim_root="''${XDG_CACHE_HOME:-$HOME/.cache}/android-cli-nix-sdk"
              shim_sdk="$shim_root/$(printf '%s' "$sdk_root" | sha256sum | cut -d' ' -f1)"

              if [ ! -e "$shim_sdk" ]; then
                mkdir -p "$shim_root"
                cp -a "$sdk_root" "$shim_sdk.tmp"
                rm -f "$shim_sdk.tmp/platform-tools/adb"
                install -Dm755 "$(command -v adb)" "$shim_sdk.tmp/platform-tools/adb"
                mv "$shim_sdk.tmp" "$shim_sdk"
              elif [ "$shim_sdk/platform-tools/adb" -ot "$(command -v adb)" ]; then
                install -Dm755 "$(command -v adb)" "$shim_sdk/platform-tools/adb"
              fi

              export ANDROID_SDK_ROOT="$shim_sdk"
              export ANDROID_HOME="$shim_sdk"

              if [ -z "$sdk_arg" ]; then
                args=(--sdk "$shim_sdk" "''${args[@]}")
              fi
            fi

            exec ${android-cli-upstream}/bin/android "''${args[@]}"
          '';
        };

        android-cli-shim-check = pkgs.runCommand "android-cli-shim-check" {
          nativeBuildInputs = [
            android-cli
            pkgs.android-tools
            pkgs.coreutils
            pkgs.diffutils
          ];
        } ''
          sdk="$TMPDIR/sdk"
          export HOME="$TMPDIR/home"
          export XDG_CACHE_HOME="$TMPDIR/cache"
          mkdir -p "$sdk/platform-tools"
          touch "$sdk/platform-tools/source.properties"

          output=$(ANDROID_SDK_ROOT="$sdk" android info 2>&1)

          shim_sdk=$(printf '%s\n' "$output" | sed -n 's/^sdk: //p' | grep . | tail -n 1)
          case "$shim_sdk" in
            "$XDG_CACHE_HOME"/android-cli-nix-sdk/*)
              ;;
            *)
              printf '%s\n' "$output"
              exit 1
              ;;
          esac

          [ -x "$shim_sdk/platform-tools/adb" ]
          cmp -s "$shim_sdk/platform-tools/adb" "${pkgs.android-tools}/bin/adb"

          touch $out
        '';
      in {
        packages.android-cli = android-cli;
        checks.android-cli-shim = android-cli-shim-check;

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            android-cli
            android-tools
            cargo
            cargo-watch
            clippy
            jdk17
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
            export JAVA_HOME="${pkgs.jdk17}"
            export PATH="$JAVA_HOME/bin:$PATH"
            echo "OpenHome API shell"
            echo "Run API commands from repo root, e.g. cargo test --manifest-path api/Cargo.toml"
          '';
        };
      });
}
