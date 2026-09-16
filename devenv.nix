{ pkgs, lib, config, inputs, ... }:

{
  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    channel = "stable";
  };

  # https://devenv.sh/packages/
  packages = with pkgs; [
    # Rust / Axum API
    openssl
    pkg-config
    sqlx-cli
    sqlite

    # Expo / React Native
    bun
    watchman

    # Repo tooling
    just
    git
  ];

  # https://devenv.sh/integrations/android/
  # Versions match React Native 0.86: compileSdk/targetSdk 36, NDK 27.1
  android = {
    enable = true;
    reactNative.enable = true;
    platforms.version = [ "36" ];
    buildTools.version = [ "36.0.0" ];
    ndk = {
      enable = true;
      version = [ "27.1.12297006" ];
    };
    # Flip this on when you want a headless emulator (large download)
    emulator.enable = true;
  };

  enterShell = ''
    repo_root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
    export DATABASE_URL="sqlite:$repo_root/api/data/app.db"
    export ANDROID_SDK_ROOT="$ANDROID_HOME"
    echo "OpenHome development shell (devenv)"
    echo "Run 'just' from the repo root to list available commands"
  '';

  # https://devenv.sh/tests/
  enterTest = ''
    just test
  '';

  # https://devenv.sh/git-hooks/
  # Hooks run through devenv shell so they work even when git commit is
  # invoked outside the devenv environment (plain PATH lacks cargo/just).
  git-hooks.hooks.just-fmt = {
    enable = true;
    name = "just fmt (format check)";
    entry = "${pkgs.devenv}/bin/devenv";
    args = [ "shell" "--" "just" "fmt" ];
    pass_filenames = false;
  };
  git-hooks.hooks.just-lint = {
    enable = true;
    name = "just lint (clippy)";
    entry = "${pkgs.devenv}/bin/devenv";
    args = [ "shell" "--" "just" "lint" ];
    pass_filenames = false;
  };
}
