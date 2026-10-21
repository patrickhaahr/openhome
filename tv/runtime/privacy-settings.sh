#!/bin/sh
# Supported settings on 55NANO806PA/webOS 6.5.3; no broad EULA withdrawal.
set -eu
DIR=/var/lib/openhome-tv
LS=/var/lib/pearwebos/luna-send-n
ACR=$DIR/acr-disabled.xml
acr_target_mounted() { awk '$2 == "/tmp/acr.xml" {found=1} END {exit !found}' /proc/mounts; }

acr_mounted() {
    [ -f "$ACR" ] && [ -f /tmp/acr.xml ] &&
        acr_target_mounted &&
        [ "$(stat -c '%d:%i' "$ACR")" = "$(stat -c '%d:%i' /tmp/acr.xml)" ]
}

acr_off() {
    [ -f /tmp/acr.xml ] || return 0
    if acr_mounted; then
        if grep -Eq '(ACR_On|capture|send_data)="true"' /tmp/acr.xml; then
            echo 'Owned ACR override changed unexpectedly; refusing an in-place rewrite.' >&2
            return 1
        fi
        mount -o bind,remount,ro "$ACR" /tmp/acr.xml
        echo 'ACR read-only override already active.'
        return 0
    fi
    if acr_target_mounted; then
        echo 'Refusing foreign ACR config mount.' >&2
        return 1
    fi
    [ -f "$DIR/acr.xml.orig" ] || cp /tmp/acr.xml "$DIR/acr.xml.orig"
    sed 's/ACR_On="true"/ACR_On="false"/g; s/capture="true"/capture="false"/g; s/send_data="true"/send_data="false"/g' /tmp/acr.xml > /tmp/openhome-acr.xml
    # Only write before mounting, so readers never see partial backing content.
    cat /tmp/openhome-acr.xml > "$ACR"
    rm /tmp/openhome-acr.xml
    mount --bind "$ACR" /tmp/acr.xml
    mount -o bind,remount,ro "$ACR" /tmp/acr.xml
    echo 'ACR capture/send disabled with a read-only runtime override.'
}
case "${1:-apply}" in
    restore)
        if [ -f /tmp/openhome-privacy-recheck.pid ]; then
            pid=$(cat /tmp/openhome-privacy-recheck.pid)
            case "$pid" in ''|*[!0-9]*) exit 1 ;; esac
            if [ -r "/proc/$pid/cmdline" ] && tr '\000' ' ' < "/proc/$pid/cmdline" | grep -Fq "$DIR/privacy-settings.sh recheck"; then
                kill "$pid"
            fi
            rm /tmp/openhome-privacy-recheck.pid
        fi
        if acr_mounted; then
            umount /tmp/acr.xml
        elif acr_target_mounted; then
            echo 'Refusing to restore over a foreign ACR config mount.' >&2
            exit 1
        fi
        if [ -f "$DIR/acr.xml.orig" ]; then
            cp "$DIR/acr.xml.orig" /tmp/openhome-acr-restore.xml
            mv /tmp/openhome-acr-restore.xml /tmp/acr.xml
        fi
        echo 'ACR runtime restored; run local tv-restore-settings for settings.'
        exit 0 ;;
    recheck)
        sleep 30
        acr_off
        sleep 60
        acr_off
        exit 0 ;;
    apply) ;;
    *) echo "usage: $0 [apply|recheck|restore]" >&2; exit 2 ;;
esac
mkdir -p "$DIR"
for payload in \
    '{"category":"general","settings":{"homePromotion":"off","sportsAlarm":"off"}}' \
    '{"category":"option","settings":{"livePromotion":"off","webOSPromotionVideo":"off","promotionStreamer":"off","livePlus":"off","watchedListCollection":"off","dbgLogUpload":false,"faultLogUpload":false}}' \
    '{"category":"other","settings":{"contentRecommendation":"off"}}'; do
    reply=$("$LS" -n 1 luna://com.webos.settingsservice/setSystemSettings "$payload")
    printf '%s\n' "$reply" | grep -q '"returnValue":true' || { printf '%s\n' "$reply" >&2; exit 1; }
done
acr_off
# Bounded checks after services settle; no permanent polling daemon.
if [ -f "$DIR/privacy-settings.sh" ] && [ ! -f /tmp/openhome-privacy-recheck.pid ]; then
    setsid "$DIR/privacy-settings.sh" recheck </dev/null > /tmp/openhome-privacy-recheck.log 2>&1 &
    echo $! > /tmp/openhome-privacy-recheck.pid
fi
echo 'Promotion/sports/privacy settings applied; existing preload policy preserved.'
