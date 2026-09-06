#!/usr/bin/env bash
# One tick of the autonomous codex operator. Scheduled every 30 min by launchd.
# Exit: 0 ok/nothing-to-do · 3 environment · 6 backlog empty (done)
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 3
LOCK="$REPO/var/loop/tick.lock"
LOG="$REPO/var/loop/tick.log"
mkdir -p "$REPO/var/loop"
ts() { date '+%Y-%m-%d %H:%M:%S'; }
say() { printf '%s  %s\n' "$(ts)" "$*" >> "$LOG"; }

# ---- single-flight: a previous tick still working is NOT a failure, it is the normal case.
if [ -d "$LOCK" ]; then
  pid=$(cat "$LOCK/pid" 2>/dev/null || echo 0)
  if kill -0 "$pid" 2>/dev/null; then
    started=$(cat "$LOCK/started" 2>/dev/null || echo '?')
    say "SKIP  tick still running (pid $pid, since $started) on: $(cat "$LOCK/item" 2>/dev/null||echo '?')"
    exit 0
  fi
  say "STALE lock from dead pid $pid — reclaiming"
  rm -rf "$LOCK"
fi
mkdir -p "$LOCK" || exit 3
echo $$ > "$LOCK/pid"; ts > "$LOCK/started"
cleanup() { rm -rf "$LOCK"; }
trap cleanup EXIT INT TERM

command -v codex >/dev/null || { say "FATAL codex not on PATH"; exit 3; }

# ---- pick: first [~] (resume, it was interrupted) else first [ ]
ITEM=$(grep -m1 '^## \[~\]' handover/BACKLOG.md || true)
[ -n "$ITEM" ] || ITEM=$(grep -m1 '^## \[ \]' handover/BACKLOG.md || true)
if [ -z "$ITEM" ]; then
  say "DONE  no unchecked backlog items remain"
  exit 6
fi
ID=$(printf '%s' "$ITEM" | sed -E 's/^## \[.\] ([A-Z0-9]+).*/\1/')
printf '%s' "$ID" > "$LOCK/item"
say "START $ID  ($ITEM)"

# mark in-progress so a crashed tick resumes rather than restarts
sed "s|^## \[ \] $ID |## [~] $ID |" handover/BACKLOG.md > handover/BACKLOG.md.tmp && mv handover/BACKLOG.md.tmp handover/BACKLOG.md 2>/dev/null || true

BEFORE=$(git rev-parse HEAD)

# ---- brief: KT + the one item + the non-negotiables. Full backlog is deliberately NOT included:
# one item per tick is the whole point.
{
  cat handover/KT-CODEX.md
  printf '\n\n===============================================================\n'
  printf '# YOUR TASK THIS TICK: %s\n\n' "$ID"
  awk -v id="$ID" '
    $0 ~ "^## \\[.\\] " id " " {p=1}
    p && /^## \[/ && $0 !~ ("^## \\[.\\] " id " ") {exit}
    p {print}
  ' handover/BACKLOG.md
  cat <<'BRIEF'

===============================================================
# HOW THIS TICK ENDS

Work ONLY the item above. Do not start the next one.

1. If it is finished and `FLEET_MUTANTS=0 bash verify.sh` is green:
   - change its line in handover/BACKLOG.md from `[~]` to `[x]`
   - record what you found in docs/DELTA.md as the next D<n> — include the unflattering parts
   - commit:  git -c core.hooksPath=/dev/null commit -am "<real message>"
2. If it is NOT finished: leave it `[~]`, commit the partial work anyway if verify is green, and
   write in handover/PROGRESS.md exactly where you stopped and what the next tick should do first.
3. If verify is RED at the end, do not commit. Say so in handover/PROGRESS.md.

Append ONE line to handover/PROGRESS.md before you stop:
   <date> | <ID> | done|partial|blocked | <one sentence of what actually happened>

Do not create git worktrees. Do not work outside this directory. Do not weaken a gate, a
pre-registered margin, or a detector to make something pass — record the failure instead. A finding
honestly recorded is worth more here than a green tick.
BRIEF
} > var/loop/brief.txt

say "BRIEF $(wc -c < var/loop/brief.txt) bytes -> codex"
timeout 5400 codex exec \
  --dangerously-bypass-approvals-and-sandbox \
  -C "$REPO" \
  "$(cat var/loop/brief.txt)" \
  >> "var/loop/codex-$ID.log" 2>&1
rc=$?
say "CODEX $ID rc=$rc"

# ---- independent verification. codex's own claim is not evidence (law 10).
FLEET_MUTANTS=0 timeout 900 bash verify.sh > var/loop/verify-last.log 2>&1
vrc=$?
RES=$(grep -E 'passed,' var/loop/verify-last.log | tail -1)
say "VERIFY rc=$vrc  ${RES:-<no summary line>}"

AFTER=$(git rev-parse HEAD)
DIRTY=$(git status --porcelain | grep -vcE '\.pyc|__pycache__|var/' || true)

if [ "$vrc" -ne 0 ]; then
  # red: keep the evidence, do not let it pollute the next tick
  if [ "$DIRTY" -gt 0 ]; then
    git stash push -q -m "tick-$ID-red-$(date +%s)" -- . ':!var/' 2>/dev/null \
      && say "RED   stashed $DIRTY dirty paths (recover: git stash list)"
  fi
  printf '%s | %s | blocked | verify red rc=%s: %s\n' "$(date +%F)" "$ID" "$vrc" "${RES:-none}" \
    >> handover/PROGRESS.md
  git -c core.hooksPath=/dev/null commit -q -m "tick $ID: verify red, work stashed" \
    handover/PROGRESS.md handover/BACKLOG.md 2>/dev/null
elif [ "$AFTER" = "$BEFORE" ] && [ "$DIRTY" -gt 0 ]; then
  # green but codex forgot to commit — commit for it rather than lose the work
  git add -A ':!*.pyc' ':!var/' ':!keel/target' 2>/dev/null
  git -c core.hooksPath=/dev/null commit -q -m "tick $ID: work committed by the loop (verify green)"
  say "GREEN committed on codex's behalf"
else
  say "GREEN $(git log --oneline "$BEFORE..$AFTER" | wc -l | tr -d ' ') new commit(s)"
fi

say "END   $ID  head=$(git rev-parse --short HEAD)  next=$(grep -m1 -oE '^## \[[ ~]\] [A-Z0-9]+' handover/BACKLOG.md | awk '{print $3}' || echo none)"
exit 0
