#!/usr/bin/env bash
# Parallel codex operators. Every backlog item whose dependencies are satisfied gets its OWN
# process, right now, concurrently. Replaces the serial one-item-per-tick loop.
#
# The git race is removed by construction, not by hope:
#   - each worker writes findings to docs/delta.d/<ID>.md, never to the shared docs/DELTA.md
#   - workers are told not to run git at all
#   - this wrapper verifies and commits, under a mutex, after the worker exits
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$REPO" || exit 3
L="$REPO/var/loop"; mkdir -p "$L/locks" "$REPO/docs/delta.d"
ts() { date '+%Y-%m-%d %H:%M:%S'; }
say() { printf '%s  %s\n' "$(ts)" "$*" >> "$L/fanout.log"; }
MAXPAR=${FLEET_MAXPAR:-4}

# dependency graph — an item runs only when every dep is [x]
dep_of() { case "$1" in B2) echo B1;; B3) echo "B1 B2";; S1) echo S0;; S2) echo S1;; S3) echo S2;; S4) echo S3;; S5) echo S1;; *) echo "";; esac; }
state_of() { grep -m1 -oE "^## \[.\] $1 " handover/BACKLOG.md | sed -E 's/^## \[(.)\].*/\1/'; }

ready() {
  local id; local st; local d; local ok
  for id in $(grep -oE '^## \[.\] [A-Z0-9]+' handover/BACKLOG.md | sed -E 's/^## \[.\] //'); do
    st=$(state_of "$id"); [ "$st" = " " ] || [ "$st" = "~" ] || continue
    ok=1; for d in $(dep_of "$id"); do [ "$(state_of "$d")" = "x" ] || ok=0; done
    [ "$ok" = 1 ] && echo "$id"
  done
}

# NOTE: `kill -0 0` targets the whole process group and SUCCEEDS, so a missing pid file used to
# report every item as running. Require a real pid > 1.
alive() { local p; [ -f "$L/locks/$1/pid" ] || return 1; p=$(cat "$L/locks/$1/pid" 2>/dev/null)
  case "$p" in ''|0|1) return 1;; *[!0-9]*) return 1;; esac; kill -0 "$p" 2>/dev/null; }

running=0
for id in $(grep -oE '^## \[.\] [A-Z0-9]+' handover/BACKLOG.md | sed -E 's/^## \[.\] //'); do
  alive "$id" && running=$((running+1))
done
say "TICK  running=$running max=$MAXPAR ready=[$(ready | tr '\n' ' ')]"

launch() {
  local ID="$1"
  mkdir -p "$L/locks/$ID"; ts > "$L/locks/$ID/started"
  sed "s|^## \[ \] $ID |## [~] $ID |" handover/BACKLOG.md > handover/BACKLOG.md.tmp && mv handover/BACKLOG.md.tmp handover/BACKLOG.md 2>/dev/null
  {
    cat handover/KT-CODEX.md
    printf '\n\n=========================\n# YOUR TASK: %s\n\n' "$ID"
    awk -v id="$ID" '$0 ~ "^## \\[.\\] " id " " {p=1}
      p && /^## \[/ && $0 !~ ("^## \\[.\\] " id " ") {exit} p {print}' handover/BACKLOG.md
    cat <<BRIEF

=========================
# RULES FOR THIS TICK — you are ONE OF SEVERAL AGENTS WORKING THIS REPO CONCURRENTLY

Others are editing other files in this same checkout at this same moment. Therefore:

1. Work ONLY on $ID. Do not touch another item's deliverable.
2. Write your findings to \`docs/delta.d/$ID.md\` — a fresh file that is yours alone.
   Do NOT edit \`docs/DELTA.md\`; a merge step folds your fragment in later. Use the same
   voice as DELTA.md: what you expected, what actually happened, what it cost, what you changed.
3. Do NOT run any git command. Not add, not commit, not stash, not checkout. The wrapper
   commits your work after verifying it. Running git will corrupt a peer's in-flight work.
4. Do NOT edit handover/BACKLOG.md. The wrapper marks your item done.
5. Run \`FLEET_MUTANTS=0 bash verify.sh\` before you stop and say the real result.
6. If your item is impossible or wrong, say so in \`docs/delta.d/$ID.md\` and stop. A finding
   honestly recorded beats a green tick. Never weaken a gate, a margin or a detector to pass.
7. Last line of your output must be exactly one of:  DONE  |  PARTIAL  |  BLOCKED
8. If you make real, demonstrable progress (whether or not verify is fully green), end
   \`docs/delta.d/$ID.md\` with a line starting exactly \`MORNING_TEST:\` followed by ONE concrete
   shell command the human owner can run by hand at 9am to see your work for real — not
   \`verify.sh\`, not a unit test name. A command that prints real output the owner reads and
   judges themselves. If nothing is demoable yet (pure investigation, no shippable change), write
   \`MORNING_TEST: none — <one line why>\` instead of inventing one.
BRIEF
  } > "$L/brief-$ID.txt"
  (
    WORKER_CLI="${FLEET_WORKER_CLI:-codex}"
    case "$WORKER_CLI" in
      codex)
        timeout 5400 codex exec --dangerously-bypass-approvals-and-sandbox -C "$REPO" \
          "$(cat "$L/brief-$ID.txt")" > "$L/codex-$ID.log" 2>&1
        ;;
      claude)
        timeout 5400 claude -p --dangerously-skip-permissions \
          "$(cat "$L/brief-$ID.txt")" > "$L/codex-$ID.log" 2>&1
        ;;
      *)
        echo "codex-fanout: unknown FLEET_WORKER_CLI=$WORKER_CLI (want codex|claude)" > "$L/codex-$ID.log"
        false
        ;;
    esac
    rc=$?
    echo "$rc" > "$L/locks/$ID/rc"
    verdict=$(tail -5 "$L/codex-$ID.log" | grep -oE '\b(DONE|PARTIAL|BLOCKED)\b' | tail -1)
    # verify.sh does not touch git — run it OUTSIDE the mutex so concurrent workers's
    # (slow, several-minute) verify runs actually overlap. Only git add/commit need
    # the lock. (Found live 2026-08-30: 5 of 6 workers sat idle in the mutex spin-wait
    # while one held it through its own verify.sh run — real work was serialized to 1x
    # even at FLEET_MAXPAR=6, defeating the fan-out.)
    FLEET_MUTANTS=0 timeout 900 bash verify.sh > "$L/verify-$ID.log" 2>&1; vrc=$?
    res=$(grep -E 'passed,' "$L/verify-$ID.log" | tail -1)
    # commit under a mutex so concurrent workers cannot interleave index writes.
    # macOS has no flock(1). mkdir is atomic on every filesystem we care about.
    for _ in $(seq 1 1200); do mkdir "$L/git.mutex" 2>/dev/null && break; sleep 1; done
    trap 'rmdir "$L/git.mutex" 2>/dev/null' RETURN 2>/dev/null
    (
      if [ "$vrc" -eq 0 ]; then
        [ "$verdict" = "DONE" ] && sed "s|^## \[~\] $ID |## [x] $ID |" handover/BACKLOG.md > handover/BACKLOG.md.tmp && mv handover/BACKLOG.md.tmp handover/BACKLOG.md
        printf '%s | %s | %s | verify green: %s\n' "$(date +%F\ %H:%M)" "$ID" "${verdict:-unknown}" "$res" >> handover/PROGRESS.md
        git add -A ':!*.pyc' ':!var/' ':!keel/target' 2>/dev/null
        git -c core.hooksPath=/dev/null commit -q -m "$ID (${verdict:-unknown}): $WORKER_CLI worker, verify green" 2>/dev/null
        say "GREEN $ID verdict=${verdict:-unknown} rc=$rc"
      else
        printf '%s | %s | blocked | verify RED rc=%s: %s\n' "$(date +%F\ %H:%M)" "$ID" "$vrc" "${res:-none}" >> handover/PROGRESS.md
        git add handover/PROGRESS.md "docs/delta.d/$ID.md" 2>/dev/null
        git -c core.hooksPath=/dev/null commit -q -m "$ID: verify RED, findings kept, code not committed" 2>/dev/null
        say "RED   $ID rc=$rc vrc=$vrc — code left uncommitted"
      fi
    )
    rmdir "$L/git.mutex" 2>/dev/null
    rm -rf "$L/locks/$ID"
  ) &
  echo $! > "$L/locks/$ID/pid"
  say "LAUNCH $ID pid=$! brief=$(wc -c < "$L/brief-$ID.txt")b"
}

for ID in $(ready); do
  [ "$running" -ge "$MAXPAR" ] && { say "CAP   $MAXPAR reached, $ID waits"; break; }
  alive "$ID" && { say "SKIP  $ID already running"; continue; }
  launch "$ID"; running=$((running+1)); sleep 3
done
say "TICK  end, $running in flight"
exit 0
