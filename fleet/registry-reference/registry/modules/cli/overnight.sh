#!/usr/bin/env bash
# Headless Ralph loop with fresh context per iteration (kills context rot),
# completion promise, stall detection, and quota-aware sleep.
# Usage: overnight.sh <project-dir> [max_iterations]   (run inside tmux: tmux new -d -s loop-X '.../overnight.sh dir 30')
# Env: FLEET_YOLO=1 -> --dangerously-skip-permissions (ONLY inside a container/devcontainer).
set -u
D="$(cd "$(dirname "$0")/../../.." && pwd)"
# shellcheck source=../../lib/err.sh
. "$D/registry/lib/err.sh" 2>/dev/null || { echo "overnight: registry/lib/err.sh missing" >&2; exit 3; }

usage() {
  cat <<'EOF'
overnight.sh <project-dir> [max_iterations]
  Headless Ralph loop: fresh context per iteration, completion-promise
  detection, stall detection (exits 2 after 3 no-commit iterations), and
  quota-aware sleep. Run inside tmux:
    tmux new -d -s loop-X '.../overnight.sh <project-dir> 30'

  <project-dir>     directory containing LOOP-PROMPT.md (required)
  [max_iterations]  iteration cap (default 30)

Env:
  FLEET_YOLO=1          --dangerously-skip-permissions (container/devcontainer only)
  FLEET_LOOP_MODEL=name per-loop --model override (default: repo default model)
  FLEET_QUOTA_SLEEP=N   seconds to sleep on a quota hit (default 2100)
  PROMISE=TAG           completion tag to look for (default ALL-FEATURES-PASS)
EOF
}

DIR=""; MAX=""
while [ $# -gt 0 ]; do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --*) reject_unknown_flag "$1" ;;
    *)
      if [ -z "$DIR" ]; then DIR="$1"; elif [ -z "$MAX" ]; then MAX="$1"; fi
      shift ;;
  esac
done
[ -n "$DIR" ] || die "$ERR_USAGE" missing_dir "a project directory is required" 'overnight.sh <project-dir> [max_iters]'

MAX="${MAX:-30}"
PROMISE="${PROMISE:-ALL-FEATURES-PASS}"
cd "$DIR" || exit 1
[ -f LOOP-PROMPT.md ] || { echo "no LOOP-PROMPT.md in $DIR"; exit 1; }
mkdir -p .fleet/logs
PERM_FLAGS=(--permission-mode acceptEdits)
[ "${FLEET_YOLO:-0}" = "1" ] && PERM_FLAGS=(--dangerously-skip-permissions)
# optional per-loop model (best-model routing); unset = repo default. e.g. FLEET_LOOP_MODEL=sonnet
MODEL_FLAGS=(); [ -n "${FLEET_LOOP_MODEL:-}" ] && MODEL_FLAGS=(--model "$FLEET_LOOP_MODEL")

STALL=0; LAST_HEAD="$(git rev-parse HEAD 2>/dev/null || echo none)"
for i in $(seq 1 "$MAX"); do
  LOG=".fleet/logs/loop-$(date +%Y%m%d-%H%M%S)-i$i.log"
  echo "=== iteration $i/$MAX $(date '+%F %T') ==="
  # MODEL_FLAGS[@] is empty whenever FLEET_LOOP_MODEL is unset (the documented default), and
  # under `set -u` bash 3.2 treats "${empty_array[@]}" as an unbound variable, not an empty
  # expansion -- reproduced live: a real invocation crashed here with
  # "MODEL_FLAGS[@]: unbound variable" on every run that didn't set FLEET_LOOP_MODEL. The
  # ${arr[@]+"${arr[@]}"} guard expands to nothing when the array is empty and to the exact
  # quoted elements otherwise; see tests/small/loop-entrypoints.test.sh's real-invocation case.
  claude -p "$(cat LOOP-PROMPT.md)" "${PERM_FLAGS[@]}" ${MODEL_FLAGS[@]+"${MODEL_FLAGS[@]}"} > "$LOG" 2>&1
  tail -3 "$LOG"

  if grep -q "<promise>$PROMISE</promise>" "$LOG"; then
    echo "COMPLETE after $i iterations."; exit 0
  fi
  if grep -qiE 'usage limit reached|out of (extra )?usage' "$LOG"; then
    echo "quota hit — sleeping before retry"; sleep "${FLEET_QUOTA_SLEEP:-2100}"; continue
  fi

  HEAD="$(git rev-parse HEAD 2>/dev/null || echo none)"
  if [ "$HEAD" = "$LAST_HEAD" ]; then
    STALL=$((STALL+1))
    [ "$STALL" -ge 3 ] && { echo "STALLED: 3 iterations with no commit. Stopping — read claude-progress.txt."; exit 2; }
  else
    STALL=0; LAST_HEAD="$HEAD"
  fi
  sleep 5
done
echo "max iterations reached without promise — review claude-progress.txt"; exit 3
