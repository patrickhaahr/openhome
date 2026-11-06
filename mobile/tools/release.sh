#!/usr/bin/env bash
# Run through the native-release-* just recipes. Never enable shell tracing here.
set -euo pipefail
umask 077

identity="${XDG_DATA_HOME:-$HOME/.local/share}/openhome/android-release"
keystore="$identity/release.p12"
password="$identity/password"
apk="mobile/app/build/outputs/apk/release/app-release.apk"
sdk_tools="${ANDROID_HOME:?Run through devenv shell}/build-tools/36.0.0"

case "${1:-}" in
    init)
        # Refuse existing/partial identities: silently rotating a key breaks upgrades.
        mkdir -p "$(dirname "$identity")"
        if ! mkdir -m 700 "$identity"; then
            echo "Signing identity already exists at $identity; preserve it, do not regenerate." >&2
            exit 1
        fi
        openssl rand -hex 32 > "$password"
        keytool -genkeypair -noprompt -storetype PKCS12 -keystore "$keystore" \
            -storepass:file "$password" -keypass:file "$password" \
            -alias openhome-release -keyalg RSA -keysize 3072 -validity 10000 \
            -dname 'CN=OpenHome Android Release'
        chmod 600 "$keystore" "$password"
        echo "Created persistent release identity at $identity. Back up this directory privately."
        ;;
    sign)
        if [[ ! -f "$keystore" || ! -f "$password" ]]; then
            echo "Release identity missing: restore its backup or run just native-release-key-init for a first release." >&2
            exit 1
        fi
        unsigned="mobile/app/build/outputs/apk/release/app-release-unsigned.apk"
        aligned="${apk%.apk}-aligned.apk"
        trap 'rm -f "$aligned"' EXIT
        "$sdk_tools/zipalign" -f -P 16 4 "$unsigned" "$aligned"
        "$sdk_tools/apksigner" sign --ks "$keystore" --ks-key-alias openhome-release \
            --ks-pass "file:$password" --out "$apk" "$aligned"
        ;;
    verify)
        "$sdk_tools/apksigner" verify --verbose --print-certs "$apk"
        "$sdk_tools/zipalign" -c -P 16 4 "$apk"
        manifest="$("$sdk_tools/aapt" dump badging "$apk")"
        printf '%s\n' "$manifest"
        if ! grep -q "^package: name='com.patrickhaahr.openhome' " <<< "$manifest" || \
            grep -q '^application-debuggable' <<< "$manifest"; then
            echo "Expected the non-debuggable native release package." >&2
            exit 1
        fi
        sha256sum "$apk"
        ;;
    *) echo "Usage: $0 init|sign|verify" >&2; exit 2 ;;
esac
