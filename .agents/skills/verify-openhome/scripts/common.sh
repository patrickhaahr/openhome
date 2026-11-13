# Sourced by the verify-openhome helpers. Resolves the repo root and the run directory.
set -euo pipefail
REPO_ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
STATE_ROOT="${XDG_STATE_HOME:-$HOME/.local/state}/openhome-verify"
# Runs and evidence live outside the checkout, so they survive worktree removal.
RUN_DIR="${OPENHOME_VERIFY_RUN:-}"
if [ -z "$RUN_DIR" ] && [ -L "$STATE_ROOT/current-$(printf %s "$REPO_ROOT" | sha256sum | cut -c1-12)" ]; then
  RUN_DIR="$(readlink -f "$STATE_ROOT/current-$(printf %s "$REPO_ROOT" | sha256sum | cut -c1-12)")"
fi
CURRENT_LINK="$STATE_ROOT/current-$(printf %s "$REPO_ROOT" | sha256sum | cut -c1-12)"
load_run() {
  [ -n "$RUN_DIR" ] && [ -f "$RUN_DIR/run.env" ] || { echo "No verify run for $REPO_ROOT; start one with up.sh" >&2; exit 1; }
  # shellcheck disable=SC1091
  . "$RUN_DIR/run.env"
}
free_port() { python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()'; }
