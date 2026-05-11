# AGENTS.md - mobile-native

Start with `../AGENTS.md`, then use this file for Android-specific work in `mobile-native/`.

## Android CLI First

- Read this before Android workflow changes if you need context on the intended agent flow: `https://android-developers.googleblog.com/2026/04/build-android-apps-3x-faster-using-any-agent.html`
- Use `android` CLI first for every Android run, device, docs, and skills workflow. Do not guess with raw `adb` or emulator commands if `android` already covers it.
- Verified locally in this repo:
  - `android info`
  - `android skills list`
  - `android skills find navigation`
  - `android docs search "compose edge-to-edge"`
  - `android emulator list|create|start|stop`
  - `android run --help`
  - `android layout -p`
  - `android screen capture`
  - `android screen resolve`
- When changing navigation, inspect skills first and use the installed project skill at `skills/navigation-3/SKILL.md`.
- When changing insets or system bars, search docs or install the `edge-to-edge` Android skill before editing.

## Project Shape

- This is a standalone Android Gradle project, not the Tauri app in `../mobile`.
- Single module only: `:app` in `settings.gradle.kts`.
- Real app entrypoint: `app/src/main/java/com/example/openhome/MainActivity.kt`.
- Navigation root: `app/src/main/java/com/example/openhome/Navigation.kt`.
- Main screen flow today:
  - `MainActivity` -> `MainNavigation()`
  - `ui/main/MainScreen.kt`
  - `ui/main/MainScreenViewModel.kt`
  - `data/DataRepository.kt`

## Current Stack That Matters

- Jetpack Compose app with Material 3.
- Already on Navigation 3: `androidx.navigation3.runtime` and `androidx.navigation3.ui` in `app/build.gradle.kts`.
- Edge-to-edge is already enabled with `enableEdgeToEdge()` in `MainActivity`, and the root screen uses `safeDrawingPadding()` in `Navigation.kt`.
- Toolchain from checked-in Gradle config:
  - AGP `9.0.1`
  - Kotlin `2.3.20`
  - Gradle `9.1.0`
  - Java toolchain `17`
  - `compileSdk` / `targetSdk` `36`, `minSdk` `24`

## Verification

- Unit tests live in `app/src/test/...`.
- Instrumented Compose UI tests live in `app/src/androidTest/...` and need a device or emulator.
- Start device work with `android emulator ...` and inspect UI with `android layout` / `android screen ...`.
- If you need build metadata or APK locations, try `android describe --project_dir "/home/ph/dev/openhome/mobile-native"`.
- In this shell, direct `./gradlew` task discovery failed until Java / `JAVA_HOME` was available. Check `android info` and local Java setup before blaming Gradle.

## Local Environment Gotcha

- `local.properties` is ignored by git and currently points at `/home/ph/Android/Sdk`.
- If SDK resolution breaks, prefer `android info` or pass `android --sdk <path> ...` instead of hardcoding another path in tracked files.
