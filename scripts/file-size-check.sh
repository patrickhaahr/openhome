#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
root=$PWD

hook_command=$(git config --get hook.file-size-guard.command || true)
read -r hook _ <<<"$hook_command"
if [[ -z ${hook-} || ! -x $hook ]]; then
  echo "git config hook.file-size-guard.command does not name an executable; install the global file-size-guard hook." >&2
  exit 2
fi

unset GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
export GIT_DIR=$scratch GIT_WORK_TREE=$root
git init -q
git add -A -- "${@:-.}"
"$hook"
