#!/bin/sh
# Reversible payload masks for the Alexa paths verified on this webOS 6 TV.
set -eu
DIR=/var/lib/openhome-tv
EMPTY=$DIR/alexa-empty
NOEXEC=$DIR/alexa-noexec
LEDGER=/tmp/openhome-alexa-mounts
LS=/var/lib/pearwebos/luna-send-n

lock_apps() {
    for id in amazon.alexa.view amazon.alexapr; do
        reply=$("$LS" -a com.webos.service.packagemanager -n 1 \
            luna://com.webos.applicationManager/lockApp "{\"id\":\"$id\",\"lock\":$1}")
        printf '%s\n' "$reply" | grep -q '"returnValue":true' || { printf '%s\n' "$reply" >&2; return 1; }
    done
}

same_inode() { [ "$(stat -c '%d:%i' "$1")" = "$(stat -c '%d:%i' "$2")" ]; }
mask() {
    source=$1 target=$2
    [ -e "$target" ] || return 0
    if ! same_inode "$source" "$target"; then
        if awk -v p="$target" '$2 == p {found=1} END {exit !found}' /proc/mounts; then
            echo "Refusing foreign mount: $target" >&2
            return 1
        fi
        mount --bind "$source" "$target"
    fi
    grep -qFx "$source $target" "$LEDGER" 2>/dev/null || printf '%s %s\n' "$source" "$target" >> "$LEDGER"
}

if [ "${1:-apply}" = restore ]; then
    if [ -f "$LEDGER" ]; then
        while read -r source target; do
            if [ -e "$source" ] && [ -e "$target" ] && same_inode "$source" "$target"; then
                umount "$target"
            fi
        done < "$LEDGER"
        rm "$LEDGER"
    fi
    lock_apps false
    timeout -t 15 /usr/sbin/ls-control scan-services
    echo "Alexa payload masks removed; disable boot hook to keep them removed."
    exit 0
fi
[ "${1:-apply}" = apply ] || { echo "usage: $0 [apply|restore]" >&2; exit 2; }
mkdir -p "$EMPTY"
chmod 555 "$EMPTY"
[ -f "$NOEXEC" ] || : > "$NOEXEC"
chmod 000 "$NOEXEC"

# Close cached web apps before masking their payload; no unrelated WAM restart.
status=0
lock_apps true || status=1
for id in amazon.alexa.view amazon.alexapr; do
    "$LS" -n 1 luna://com.webos.applicationManager/closeByAppId "{\"id\":\"$id\"}" >/dev/null
done

for base in /usr/palm/applications /media/system/apps/usr/palm/applications /media/cryptofs/apps/usr/palm/applications /mnt/otncabi/usr/palm/applications /mnt/otycabi/usr/palm/applications /var/palm/jail/*/usr/palm/applications; do
    for id in amazon.alexa.view amazon.alexapr; do
        [ ! -d "$base/$id" ] || mask "$EMPTY" "$base/$id"
    done
done
for base in /usr/sbin /var/palm/jail/*/usr/sbin; do
    for executable in amazon-alexa-adapter amazon-alexa-vsk; do
        [ ! -f "$base/$executable" ] || mask "$NOEXEC" "$base/$executable"
    done
done

# Cover LS2 activation as well as direct execution. Do not alter the hub itself.
for pair in 'plugin amazon-alexa-adapter' 'vsk amazon-alexa-vsk'; do
    set -- $pair
    target=/usr/share/luna-service2/services.d/amazon.alexa.adapter.$1.service
    replacement=$DIR/alexa-$1.service
    if [ ! -f "$replacement" ]; then
        sed 's|^Exec=.*|Exec=/bin/false|' "$target" > "$replacement"
    fi
    mask "$replacement" "$target"
done

# The hub caches Exec lines; reload them without restarting the hub or apps.
timeout -t 15 /usr/sbin/ls-control scan-services || status=1

# Only the two confirmed native Alexa binaries; Prime Video is unaffected.
for proc in /proc/[0-9]*; do
    executable=$(readlink "$proc/exe" 2>/dev/null || true)
    case "$executable" in
        */usr/sbin/amazon-alexa-adapter|*/usr/sbin/amazon-alexa-vsk)
            kill "${proc##*/}" 2>/dev/null || true ;;
    esac
done
echo "Alexa payloads, native executables and LS2 activation files masked."
exit "$status"
