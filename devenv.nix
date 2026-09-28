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

    # Firmware / PlatformIO (`pio`)
    platformio

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

  # devenv runs `prek run -a` on every shell entry (fmt + clippy ~3min).
  # Hooks are already enforced at commit time, so make the shell-entry run a
  # no-op; remove this if you want lint re-checked on each `cd`.
  tasks."devenv:git-hooks:run".exec = lib.mkForce ":";

  enterShell = ''
    repo_root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
    export DATABASE_URL="sqlite:$repo_root/api/data/app.db"
    export ANDROID_SDK_ROOT="$ANDROID_HOME"
    # Emulator needs its bundled libs on LD_LIBRARY_PATH (otherwise it dies on a
    # libc++ symbol error), and AVDs live in ~/.android/avd, not devenv's
    # default repo-local .android/avd directory.
    export LD_LIBRARY_PATH="$ANDROID_HOME/emulator/lib64:$LD_LIBRARY_PATH"
    export ANDROID_AVD_HOME="$HOME/.android/avd"
    echo "OpenHome development shell (devenv)"
    echo "Run 'just' from the repo root to list available commands"
  '';

  # https://devenv.sh/tests/
  enterTest = ''
    just api-test
  '';

  # https://devenv.sh/git-hooks/
  # Runs through devenv shell so it works even when git commit is invoked
  # outside the devenv environment (plain PATH lacks cargo/just).
  git-hooks.hooks.just-check = {
    enable = true;
    name = "just check";
    entry = "${pkgs.devenv}/bin/devenv";
    args = [ "shell" "--" "just" "check" ];
    pass_filenames = false;
  };
}
