#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
socket="$PWD/tv/private/ssh.sock"
if timeout 8 ssh -S "$socket" -o BatchMode=yes -o ConnectTimeout=3 \
    -o StrictHostKeyChecking=yes -o UserKnownHostsFile="$PWD/tv/private/known_hosts" \
    root@10.0.40.2 true >/dev/null 2>&1; then
    exit 0
fi
# A reboot can leave a local master/socket whose remote transport is dead.
timeout 3 ssh -S "$socket" -O exit root@10.0.40.2 >/dev/null 2>&1 || true
rm -f "$socket"
exec ssh -M -S "$PWD/tv/private/ssh.sock" -o ControlPersist=3600 \
    -o ServerAliveInterval=5 -o ServerAliveCountMax=2 \
    -o StrictHostKeyChecking=yes -o UserKnownHostsFile="$PWD/tv/private/known_hosts" \
    -o ConnectTimeout=8 -o NumberOfPasswordPrompts=1 -fnNT root@10.0.40.2
