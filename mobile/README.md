# Native Android release preparation

Native acceptance is pending. Expo remains operational and is not deprecated.
The native release package is `com.patrickhaahr.openhome`; the debug package is
`com.patrickhaahr.openhome.debug` and Expo is `com.example.openhome.expo`.
These installs have separate app data; installing native does not migrate Expo's
local data. Native Setup Flow downloads existing fitness data from the Axum API.

From the repository root, with the installed devenv Android SDK/JDK:

```sh
# First release only; restore an existing identity instead when moving machines.
devenv shell -- just native-release-key-init
devenv shell -- just native-release
devenv shell -- just native-release-verify
```

The release recipe runs release lint/build, aligns the APK and signs it with
Android `apksigner`. It does not use the Android debug certificate. Output:
`mobile/app/build/outputs/apk/release/app-release.apk`. Verification prints the
public signing certificate fingerprint, manifest package/version/SDK information
and SHA-256 checksum, and rejects a debuggable or wrong-package APK.
Debug lint/build remain `just native-lint native-build`; release preparation runs
no instrumentation and contacts no device.

Resource optimization is disabled in `gradle.properties`: AGP 9.5 with devenv's
SDK AAPT2 produced optimization metadata without the resource archive, yielding
an APK without its manifest. Packaging the linked resources directly preserves
an installable APK; release verification catches a missing manifest.

Signing material lives outside git at
`${XDG_DATA_HOME:-$HOME/.local/share}/openhome/android-release/`:
`release.p12` (PKCS12, alias `openhome-release`) and `password` (randomly generated).
The directory is created with mode 700 and files with mode 600. Passwords are read
from files by the native tools, never passed as literal command arguments or
stored in Gradle/repository configuration. Initialization refuses an existing
directory, including an incomplete one, rather than replacing a signing identity.

Back up **both files together** in private encrypted storage before relying on
this install. Keep the same identity for every upgrade and restore it with the
same restrictive permissions on another machine. Loss of the key/password
prevents signing compatible updates; do not resolve a signature mismatch by
uninstalling or clearing app data. Record the public certificate fingerprint
separately and compare it before installing subsequent builds. Increment
`versionCode` in `app/build.gradle.kts` for future distributed updates.

After physical-device authorization, install with an explicit serial:

```sh
devenv shell -- just native-release-install SERIAL
```

This verifies the existing APK and uses `adb install -r`, preserving the native
app's data. It does not build, launch, uninstall, clear data or run tests. Do not
run `native-test` on a physical phone: instrumentation clears the debug app data.

Release preparation is not acceptance. The remaining emulator suite was waived
in favor of a separate physical Pixel journey: use an isolated fixture API and
invented credentials/data; verify offline creation/repeat/adjust/complete,
restart recovery, History corrections, Profile/Body Weight, mobile-data queuing,
Wi-Fi-only sync and lost-response retries with independent server reads. Check
Home errors/acknowledgements, nested Android back, the 150-second default and
remembered per-exercise timer, visible keyboard/layout, accessible targets and
dark-only two-tab navigation. Do not use real household data or trigger household
actions without separate authorization. Final acceptance and Expo deprecation
remain gated on that physical journey.
