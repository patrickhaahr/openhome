# Native Android

Native Kotlin Android is the supported mobile client after physical Pixel acceptance
on 6 November 2026. Expo is deprecated for new mobile development; its source and
recipes remain available during transition. Existing API and Dashboard features
and stored fitness data are unchanged.
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

## First use and support

Open OpenHome and complete Setup Flow with your Axum Base URL and API Key while
online. The download makes History, Library, Profile and Body Weight available
locally. Use HTTPS outside a trusted network. Keep pending work on the device:
do not uninstall or clear app data before it has synchronized.

Workout opens first. Resume preserves the current session; Repeat Last Workout
copies performance as unconfirmed sets. Confirm individual sets or Complete Exercise
before finishing. History, Library and avatar-accessible Profile work offline.
Fitness saves locally and synchronizes automatically on Wi-Fi. A mobile-data
connection alone leaves changes pending; an unreachable API leaves them on-device
with feedback. Reopen the app on working Wi-Fi to check its synchronization status.

Home commands are immediate over any reachable API connection, never queued for
later. An acknowledgement does not confirm TV, Speaker or Lights state. Backlight
shows intended, acknowledged and observed facts separately; refresh after an
unknown outcome. Bedtime scheduling and TV Sync capture/recovery belong to Axum.

The release is installed on the acceptance Pixel with invented fixture data
retained. Its temporary fixture server is stopped;
that installation is not configured for the household API. Do not discard real
pending work to change servers. There is no supported in-app server-switch flow.

See [acceptance and limitations](../docs/native-android-acceptance.md) before
describing device/network coverage. Release installation alone is not acceptance.
