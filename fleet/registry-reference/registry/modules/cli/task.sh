#!/usr/bin/env bash
# shellcheck disable=SC1010
# Task queue for the autopilot. Tasks live at <workspace>/tasks/{inbox,active,done,failed}/.
# Subcommands:
#   task.sh add   --repo <path-under-company/> [--tier safe|risky] --title "<one-line>" -- <intent...>
#   task.sh run   --repo ...                    (add, then run it now through autopilot)
#   task.sh list                                (show the queue)
#   task.sh drain                               (run every inbox task once, in order)
set -u
FLEET_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"
WS="$(cd "$FLEET_DIR/.." && pwd)"
Q="$WS/tasks"; mkdir -p "$Q"/{inbox,active,done,failed}
slug() { echo "$1" | tr '[:upper:] ' '[:lower:]-' | tr -cd 'a-z0-9-' | cut -c1-40; }
run_autopilot() {
  if [ -n "${FLEET_AUTOPILOT:-}" ] && [ -x "$FLEET_AUTOPILOT" ]; then
    "$FLEET_AUTOPILOT" "$1"
    return $?
  fi
  echo "task: no autopilot implementation is installed; set FLEET_AUTOPILOT to an executable" >&2
  return 2
}

cmd="${1:-list}"; shift || true

_add() {   # prints the created task-file path
  local repo="" tier="risky" title="" body=""
  while [ $# -gt 0 ]; do case "$1" in
    --repo) need_val --repo "${2-}"; repo="$2"; shift 2;; --tier) need_val --tier "${2-}"; tier="$2"; shift 2;;
    --title) need_val --title "${2-}"; title="$2"; shift 2;; --) shift; body="$*"; break;;
    *) body="$body $1"; shift;; esac; done
  [ -n "$repo" ]  || { echo "add: --repo <path under company/> is required" >&2; return 1; }
  [ -n "$title" ] || title="$(echo "$body" | cut -c1-60)"
  local id; id="$(date +%Y%m%d-%H%M%S)-$(slug "$title")"
  local f="$Q/inbox/$id.md"
  { echo "id: $id"; echo "repo: $repo"; echo "tier: $tier"; echo "title: $title"; echo;
    echo "$body" | sed 's/^ *//'; } > "$f"
  echo "$f"
}

case "$cmd" in
  add)  f="$(_add "$@")" && echo "queued: $f" ;;
  run)  f="$(_add "$@")" && { echo "running: $f"; run_autopilot "$f"; } ;;
  list)
    for s in inbox active done failed; do
      n=$(ls -1 "$Q/$s" 2>/dev/null | wc -l | tr -d ' '); echo "== $s ($n) =="
      ls -1 "$Q/$s" 2>/dev/null | sed 's/^/   /'
    done ;;
  drain)
    shopt -s nullglob
    for f in "$Q"/inbox/*.md; do
      echo "=== draining $(basename "$f") ==="
      run_autopilot "$f" || echo "  (task failed — see tasks/failed/)"
    done
    echo "inbox drained." ;;
  *) echo "usage: task.sh {add|run|list|drain} ..."; exit 1 ;;
esac
