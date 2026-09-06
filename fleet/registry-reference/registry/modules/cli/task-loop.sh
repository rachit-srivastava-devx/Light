#!/usr/bin/env bash
# Unattended drain loop: keep running queued tasks through the autopilot, sleeping through
# quota windows, until the inbox is empty (or --once). Run in tmux:
#   tmux new -d -s autopilot "fleet/registry/modules/cli/task-loop.sh"
# Env: FLEET_YOLO=1 (container only), FLEET_QUOTA_SLEEP (default 2100), POLL (default 300).
set -u
FLEET_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"
# shellcheck source=../../lib/err.sh
. "$FLEET_DIR/registry/lib/err.sh" 2>/dev/null || { echo "task-loop: registry/lib/err.sh missing" >&2; exit 3; }

usage() {
  cat <<'EOF'
task-loop.sh [--once] [--help]
  Unattended drain loop: keep running queued tasks through the autopilot,
  sleeping through quota windows, until the inbox is empty (or --once).
  Run in tmux:
    tmux new -d -s autopilot "fleet/registry/modules/cli/task-loop.sh"

  --once   drain until the inbox is observed empty, then exit 0 (skips the
           infinite poll -- use this for a bounded/testable run)
  --help   show this message and exit 0; performs no work

Env:
  FLEET_YOLO=1        container-only (read by task.sh/the autopilot, not here)
  FLEET_QUOTA_SLEEP   read by the autopilot's quota-sleep path (default 2100)
  POLL                seconds between empty-inbox checks (default 300)
EOF
}

ONCE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --once) ONCE="--once"; shift ;;
    *) reject_unknown_flag "$1" ;;
  esac
done

WS="$(cd "$FLEET_DIR/.." && pwd)"; Q="$WS/tasks"
POLL="${POLL:-300}"
mkdir -p "$Q/inbox"
while :; do
  if compgen -G "$Q/inbox/*.md" >/dev/null; then
    bash "$FLEET_DIR/registry/modules/cli/task.sh" drain
  else
    [ "$ONCE" = "--once" ] && { echo "inbox empty — exiting (--once)"; exit 0; }
    echo "[$(date '+%F %T')] inbox empty — sleeping ${POLL}s"; sleep "$POLL"
  fi
done
