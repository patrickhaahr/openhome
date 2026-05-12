[working-directory: "api"]
run:
    cargo run

[working-directory: "api"]
test:
    cargo test

[working-directory: "api"]
fmt:
    cargo fmt
    cargo clippy

[working-directory: "api"]
go:
    cargo test
    cargo fmt
    cargo clippy

[working-directory: "mobile-native"]
android-build:
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; ./gradlew :app:assembleDebug'

[working-directory: "mobile-native"]
android-run: android-build
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; android run --apks "app/build/outputs/apk/debug/app-debug.apk" --activity "com.example.openhome.MainActivity"'

[working-directory: "mobile-native"]
android-lint:
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; ./gradlew ktlintCheck :app:lint'

[working-directory: "mobile-native"]
android-test:
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; ./gradlew :app:testDebugUnitTest'
