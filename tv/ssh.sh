#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
exec ssh -T -S "$PWD/tv/private/ssh.sock" -o BatchMode=yes \
    -o ConnectTimeout=8 -o ServerAliveInterval=5 -o ServerAliveCountMax=2 -o StrictHostKeyChecking=yes \
    -o UserKnownHostsFile="$PWD/tv/private/known_hosts" root@10.0.40.2 "$@"
