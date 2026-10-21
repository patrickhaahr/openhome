#!/bin/sh
set -u
DIR=/var/lib/openhome-tv
[ ! -f "$DIR/disabled" ] || exit 0
retry() {
    for attempt in 1 2 3; do
        "$1" && return 0
        echo "Retry $attempt: $1" >&2
        sleep 2
    done
    return 1
}
status=0
# Privacy failures must never prevent Alexa blocking (or vice versa).
retry "$DIR/alexa-block.sh" || status=1
retry "$DIR/privacy-settings.sh" || status=1
exit "$status"
