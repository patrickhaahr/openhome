#!/usr/bin/env bash

set -euo pipefail

readonly url="http://10.0.40.3/remotes/edifier"
readonly commands=(
  bluetooth
  optical
  mute
  volume-up
  volume-down
  power
)

show_help() {
  printf 'Usage: %s <command>\n\nAvailable commands:\n' "$0"
  for command in "${commands[@]}"; do
    printf '  curl --request POST --data-urlencode "command=%s" "%s"\n' \
      "$command" "$url"
  done
}

if [[ $# -ne 1 || "$1" == "-h" || "$1" == "--help" ]]; then
  show_help
  [[ $# -eq 1 ]] || exit 1
  exit 0
fi

command="$1"
if [[ ! " ${commands[*]} " =~ " ${command} " ]]; then
  printf 'Unknown Edifier command: %s\n\n' "$command" >&2
  show_help >&2
  exit 1
fi

printf 'curl --request POST --data-urlencode "command=%s" "%s"\n' \
  "$command" "$url"
curl --request POST --data-urlencode "command=$command" "$url"
printf '\n'
