#!/usr/bin/env bash
set -euo pipefail

max_tokens=5000
max_bytes=$((max_tokens * 4))
generated='(^|/)(node_modules|vendor|dist|build|out|target|\.sqlx|\.tanstack|__generated__|generated)/|(\.lock|\.lockb|-lock\.json|-lock\.yaml|\.sum|\.min\.(js|css)|\.map|\.snap|\.svg)$|\.gen\.|_generated\.|\.pb\.'

cd "$(git rev-parse --show-toplevel)"

offenders=0
while IFS= read -r -d '' path; do
  [[ -f $path ]] || continue
  [[ $path =~ $generated ]] && continue
  size=$(stat -c %s -- "$path")
  ((size <= max_bytes)) && continue
  attrs=$(git check-attr file-size-guard linguist-generated linguist-vendored -- "$path")
  grep -qE 'file-size-guard: unset|linguist-(generated|vendored): set' <<<"$attrs" && continue
  head -n 5 -- "$path" | grep -qiE '@generated|do not edit|code generated' && continue
  grep -qI . -- "$path" || continue
  printf '%7d bytes (~%d tokens)  %s\n' "$size" $((size / 4)) "$path"
  offenders=$((offenders + 1))
done < <(git ls-files -z --cached --others --exclude-standard -- "$@")

((offenders == 0)) && exit 0
echo "$offenders file(s) over $max_tokens tokens ($max_bytes bytes)." >&2
exit 1
