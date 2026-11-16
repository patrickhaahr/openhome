# Build the native Android app and its device tests
[group('mobile')]
[working-directory: 'mobile']
mobile-build:
    ./gradlew --console=plain :app:assembleDebug :e2e:assembleDebug

# Run Android Lint on the native app (warnings fail)
[group('mobile')]
[working-directory: 'mobile']
mobile-lint:
    ./gradlew --console=plain :app:lintDebug

# Create a persistent user-local release identity once (never replaces an existing key)
[group('mobile')]
mobile-release-key-init:
    bash mobile/tools/release.sh init

# Lint and build the release variant, then align, sign and verify without a device
[group('mobile')]
mobile-release:
    cd mobile && ./gradlew --console=plain :app:lintRelease :app:assembleRelease
    bash mobile/tools/release.sh sign
    just mobile-release-verify

# Show the APK certificate, release manifest and checksum without accessing a device
[group('mobile')]
mobile-release-verify:
    bash mobile/tools/release.sh verify

# Install the verified release on an explicit authorized device, preserving app data
[group('mobile')]
mobile-release-install serial: mobile-release-verify
    adb -s {{quote(serial)}} install -r mobile/app/build/outputs/apk/release/app-release.apk

# Run the native app's device tests on a booted emulator (optionally one class: just mobile-test SetupFlowTest)
[group('mobile')]
[working-directory: 'mobile']
mobile-test class='':
    #!/usr/bin/env bash
    set -euo pipefail
    # Emulators only: the tests clear the debug app's data, which must not reach a real phone's copy.
    serial="$(adb devices | awk '$1 ~ /^emulator-/ && $2 == "device" { print $1; exit }')"
    if [ -z "$serial" ]; then
        echo "mobile-test needs a booted Android emulator" >&2
        exit 1
    fi
    # Android allows one UiAutomation client; agent-device's snapshot helper holds it while a session is open.
    if [ -n "$(adb -s "$serial" shell pidof com.callstack.agentdevice.snapshothelper | tr -d '\r')" ]; then
        echo "mobile-test needs UiAutomation free: run 'agent-device close' first" >&2
        exit 1
    fi
    filter=()
    if [ -n {{quote(class)}} ]; then
        filter=("-Pandroid.testInstrumentationRunnerArguments.class=com.patrickhaahr.openhome.e2e."{{quote(class)}})
    fi
    # The emulator's host process grows with what it renders, past 10 GB over the whole suite, which can starve
    # the machine and the tests with it. So a run gives it room: built first, build daemons gone, no animations.
    ./gradlew --console=plain :app:assembleDebug :e2e:assembleDebug
    ./gradlew --console=plain --stop
    animations=(window_animation_scale transition_animation_scale animator_duration_scale)
    declare -A animation_was
    for scale in "${animations[@]}"; do
        animation_was[$scale]="$(adb -s "$serial" shell settings get global "$scale" | tr -d '\r')"
        adb -s "$serial" shell settings put global "$scale" 0
    done
    restore_animations() {
        for scale in "${animations[@]}"; do
            if [ "${animation_was[$scale]}" = null ]; then
                adb -s "$serial" shell settings delete global "$scale" >/dev/null || true
            else
                adb -s "$serial" shell settings put global "$scale" "${animation_was[$scale]}" || true
            fi
        done
    }
    trap restore_animations EXIT
    ANDROID_SERIAL="$serial" ./gradlew --console=plain :e2e:connectedDebugAndroidTest "${filter[@]}"

# Install the debug build on a booted emulator or device
[group('mobile')]
[working-directory: 'mobile']
mobile-install:
    ./gradlew --console=plain :app:installDebug

# Run the real API on :8000 with an invented key against a throwaway database, or restart it on an earlier one's directory (the emulator reaches it at 10.0.2.2:8000)
[group('mobile')]
mobile-fixture-api key='mobile-fixture-key' dir='':
    #!/usr/bin/env bash
    set -euo pipefail
    repo="$PWD"
    (cd api && SQLX_OFFLINE=true cargo build --bin openhome-api)
    # Outside the repo so the API's dotenv lookup finds no api/.env and stays on invented settings.
    dir={{quote(dir)}}
    if [ -z "$dir" ]; then dir="$(mktemp -d /tmp/openhome-mobile-fixture.XXXXXX)"; fi
    cd "$dir"
    echo "Throwaway database: $dir/app.db"
    DATABASE_URL="sqlite:$dir/app.db" API_KEY={{quote(key)}} NOOP_PUSH_TOKEN=mobile-fixture-noop-token \
        exec "$repo/api/target/debug/openhome-api"

# Fill an empty fixture API with invented fitness data larger than the API's list pages
[group('mobile')]
mobile-fixture-seed key='mobile-fixture-key':
    python3 mobile/tools/seed_fixture.py --api-key {{quote(key)}}

# Proxy the fixture API on :8001, committing the next N Workout creates, M Exercise creates, E Workout edits, B Body Weight records and P Profile edits but losing their responses (the emulator reaches it at 10.0.2.2:8001)
[group('mobile')]
mobile-fixture-lossy-proxy lose='1' lose-exercises='0' lose-edits='0' lose-body-weight='0' lose-profile='0':
    python3 mobile/tools/lossy_proxy.py --lose-workout-responses {{quote(lose)}} --lose-exercise-responses {{quote(lose-exercises)}} --lose-edit-responses {{quote(lose-edits)}} --lose-body-weight-responses {{quote(lose-body-weight)}} --lose-profile-responses {{quote(lose-profile)}}

# Lint and build the native app, then run its device tests when an emulator is booted
[group('mobile')]
mobile-check: mobile-lint mobile-build
    #!/usr/bin/env bash
    set -euo pipefail
    if adb devices | awk '$1 ~ /^emulator-/ && $2 == "device" { found = 1 } END { exit !found }'; then
        just mobile-test
    else
        echo "mobile-check: NO EMULATOR BOOTED, device tests NOT run. Boot one and run: just mobile-test" >&2
    fi
