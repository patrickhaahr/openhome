#!/bin/sh
# Read-only live assertions; run before and after applying the relevant changes.
set -eu
case "$1" in
    privacy)
        grep -q '^# OpenHome LG privacy block' /etc/hosts
        grep -q '^0.0.0.0 rdx2.nextlgsdp.com$' /etc/hosts
        grep -q '^0.0.0.0 info.lgsmartad.com$' /etc/hosts
        echo 'PASS: telemetry/ad entries are in the effective hosts file.' ;;
    alexa)
        for base in /usr/palm/applications /var/palm/jail/*/usr/palm/applications; do
            for id in amazon.alexa.view amazon.alexapr; do
                [ ! -e "$base/$id/index.html" ] || { echo "FAIL: Alexa payload readable at $base/$id"; exit 1; }
            done
        done
        for base in /usr/sbin /var/palm/jail/*/usr/sbin; do
            for name in amazon-alexa-adapter amazon-alexa-vsk; do
                [ ! -x "$base/$name" ] || { echo "FAIL: Alexa executable available at $base/$name"; exit 1; }
            done
        done
        for proc in /proc/[0-9]*; do
            executable=$(readlink "$proc/exe" 2>/dev/null || true)
            case "$executable" in
                */usr/sbin/amazon-alexa-adapter|*/usr/sbin/amazon-alexa-vsk)
                    echo 'FAIL: native Alexa process is running'; exit 1 ;;
            esac
        done
        echo 'PASS: Alexa payloads unavailable and no native Alexa process running.' ;;
    *) echo 'usage: probe.sh privacy|alexa' >&2; exit 2 ;;
esac
