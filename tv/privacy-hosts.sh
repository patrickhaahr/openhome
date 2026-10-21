#!/bin/sh
# Adapted from HunterAP23/lg-lockout, 10-block-lg-telemetry, commit
# e393103d89004617521c552675532ed6b7395f7a. Narrow local policy: no new OTA,
# ThinQ/IoT, voice or Content Store blocks. Existing hosts lines are preserved.
#
# MIT License
# Copyright (c) 2026 HunterAP23
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

set -eu
TARGET=/etc/hosts
BACKING=/tmp/hosts.openhome-privacy
STATE=$BACKING.state
LOCK=$BACKING.lock
SCRATCH=$BACKING.new.$$
BEGIN='# OpenHome LG privacy block BEGIN'
END='# OpenHome LG privacy block END'

# Categories retained from the pinned upstream policy: telemetry, ads and AI.
DOMAINS='
nextlgsdp.com
ibs.nextlgsdp.com
ibsstat.nextlgsdp.com
rdx2.nextlgsdp.com
lgtvcommon.com
nudge.lgtvcommon.com
homeprv.lgtvcommon.com
wiseconfig.lgtvcommon.com
pnv.lgtvcommon.com
recommend.lgtvcommon.com
cdpsvc.lgtvcommon.com
cpauth.lgtvcommon.com
cdpbeacon.lgtvcommon.com
rdl.lgtvcommon.com
wiseresource.lgtvcommon.com
qcardservice.lgtvcommon.com
qt2-kic.lab.lgtvcommon.com
lggalleryplus.com
tv.wiselg.com
lgsmartad.com
info.lgsmartad.com
ngfts.lge.com
qt2-ngfts.lge.com
kic-ngfts.lge.com
kic-qt2-ngfts.lge.com
'

log() { printf '%s\n' "openhome-privacy: $*" >&2; }
die() { log "$*"; exit 1; }
identity() { stat -c '%d:%i' "$1"; }
# fdinfo identifies the mount actually reached by opening the path, even when
# several bind mounts are stacked. awk inherits descriptor 9 in this subshell.
mount_id() { (exec 9< "$1"; awk '/^mnt_id:/ {print $2}' /proc/self/fdinfo/9); }
marker() { grep -Fqx "$BEGIN" "$TARGET"; }

load_state() {
    OWN_ID= OWN_INODE=
    [ -f "$STATE" ] || return 0
    read -r OWN_ID OWN_INODE < "$STATE" || die 'unreadable ownership record'
    case "$OWN_ID" in ''|*[!0-9]*) die 'invalid mount ownership record' ;; esac
}

owned() {
    [ -n "$OWN_ID" ] && [ "$(mount_id "$TARGET")" = "$OWN_ID" ] &&
        [ "$(identity "$TARGET")" = "$OWN_INODE" ] && marker
}

restore() {
    if owned; then
        umount "$TARGET" || die 'could not unmount our hosts override'
        [ "$(mount_id "$TARGET")" != "$OWN_ID" ] || die 'our mount remained active'
        rm -f "$STATE" "$BACKING"
        OWN_ID= OWN_INODE=
        log 'restored underlying hosts; other mounts retained'
    elif [ -n "$OWN_ID" ] && awk -v id="$OWN_ID" '$1 == id {found=1} END {exit !found}' /proc/self/mountinfo; then
        die 'another mount covers our override; refusing to unmount it'
    elif marker; then
        die 'privacy marker found without matching mount ownership; refusing to alter it'
    else
        rm -f "$STATE"
        OWN_ID= OWN_INODE=
        log 'no owned hosts override active'
    fi
}

build() {
    # Only remove our delimited section. Preserve all preceding AND following
    # base entries, including Homebrew OTA entries and unrelated local entries.
    awk -v begin="$BEGIN" -v end="$END" '
        $0 == begin {if (inside || seen++) exit 2; inside=1; next}
        $0 == end {if (!inside) exit 2; inside=0; next}
        !inside {print}
        END {if (inside) exit 2}
    ' "$TARGET" > "$SCRATCH" || die 'malformed existing privacy section'
    printf '%s\n' "$BEGIN" >> "$SCRATCH"
    for host in $DOMAINS; do
        printf '0.0.0.0 %s\n::1 %s\n' "$host" "$host" >> "$SCRATCH"
    done
    printf '%s\n' "$END" >> "$SCRATCH"
}

case "${1:-}" in ''|--restore) ;; *) die 'usage: privacy-hosts.sh [--restore]' ;; esac
[ "$(id -u)" = 0 ] || die 'root required'
[ -f "$TARGET" ] || die 'hosts file missing'
mkdir "$LOCK" 2>/dev/null || die "another run or stale lock exists: $LOCK"
trap 'rm -f "$SCRATCH"; rmdir "$LOCK"' EXIT
trap 'exit 1' HUP INT TERM
load_state
[ -n "$(mount_id "$TARGET")" ] || die 'kernel does not expose mount identity'

# Recover ownership if interrupted immediately after binding but before saving
# state. Require the exact known backing inode and a distinct mount ID.
if [ -z "$OWN_ID" ] && marker && [ -f "$BACKING" ] &&
    [ "$(identity "$TARGET")" = "$(identity "$BACKING")" ] &&
    [ "$(mount_id "$TARGET")" != "$(mount_id "$BACKING")" ]; then
    OWN_ID=$(mount_id "$TARGET")
    OWN_INODE=$(identity "$TARGET")
    printf '%s %s\n' "$OWN_ID" "$OWN_INODE" > "$STATE"
fi

if [ "${1:-}" = --restore ]; then restore; exit 0; fi
[ ! -f /var/luna/preferences/webosbrew_block_updates_extra_off ] || { log 'opt-out flag present'; exit 0; }

if owned; then
    if [ -f "$BACKING" ] && [ "$(identity "$BACKING")" = "$OWN_INODE" ]; then
        build
        cat "$SCRATCH" > "$BACKING"
        owned || die 'override ownership changed during refresh'
        log 'privacy entries refreshed in existing bind mount'
        exit 0
    fi
    # A deleted/replaced backing pathname cannot refresh the mounted inode.
    # Unmount only our verified mount, then rebuild from the underlying hosts.
    restore
elif [ -n "$OWN_ID" ]; then
    restore
elif marker; then
    die 'unowned privacy section present; refusing duplicate override'
fi

build
mv "$SCRATCH" "$BACKING"
BEFORE_ID=$(mount_id "$TARGET")
mount --bind "$BACKING" "$TARGET" 2>/dev/null ||
    mount -o bind "$BACKING" "$TARGET" || die 'bind mount failed'
OWN_ID=$(mount_id "$TARGET")
OWN_INODE=$(identity "$BACKING")
printf '%s %s\n' "$OWN_ID" "$OWN_INODE" > "$STATE"
[ "$OWN_ID" != "$BEFORE_ID" ] && owned || die 'bind verification failed'
log 'privacy hosts override active; base and OTA entries preserved'
