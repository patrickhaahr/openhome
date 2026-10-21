#!/bin/sh
# Small supervised Node client, independent of LG Home's CRIU snapshots.
set -eu
dir=/var/lib/openhome-tv
pidfile=$dir/client-supervisor.pid
matches() {
    [ -r "$pidfile" ] || return 1
    pid=$(cat "$pidfile" 2>/dev/null) || return 1
    case "$pid" in ''|*[!0-9]*) return 1;; esac
    [ -r "/proc/$pid/cmdline" ] && tr '\000' ' ' < "/proc/$pid/cmdline" | grep -q '/var/lib/openhome-tv/client-service.sh supervise'
}
case "${1:-start}" in
    start)
        [ ! -e "$dir/client-disabled" ] || exit 0
        matches && exit 0
        setsid /bin/sh "$dir/client-service.sh" supervise </dev/null >> "$dir/client.log" 2>&1 &
        sleep 1
        ;;
    supervise)
        # The kernel releases this lock after crashes and power loss. Children
        # close descriptor 9 so they cannot keep a dead supervisor's lock alive.
        exec 9> "$dir/client.lock"
        flock -n 9 || exit 0
        echo $$ > "$pidfile"
        child=''
        trap '[ -z "$child" ] || { kill "$child" 2>/dev/null || true; wait "$child" || true; }; rm -f "$pidfile"; exit 0' TERM INT
        while [ ! -e "$dir/client-disabled" ]; do
            /usr/bin/node "$dir/client.js" 9>&- &
            child=$!
            wait "$child" || true
            child=''
            # Bound disk use even if configuration/network is temporarily unavailable.
            tail -n 30 "$dir/client.log" > "$dir/client.log.tmp"
            cat "$dir/client.log.tmp" > "$dir/client.log"
            rm -f "$dir/client.log.tmp"
            sleep 5 9>&- & child=$!; wait "$child" || true; child=''
        done
        rm -f "$pidfile"
        ;;
    stop)
        if matches; then
            kill "$pid"
            for attempt in 1 2 3 4 5; do matches || exit 0; sleep 1; done
            echo 'TV client supervisor did not stop' >&2; exit 1
        fi
        ;;
    *) echo 'usage: client-service.sh start|stop|supervise' >&2; exit 2;;
esac
