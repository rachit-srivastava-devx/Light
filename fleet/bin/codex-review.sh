#!/usr/bin/env bash
# The reviewer half of the loop. A builder may never review itself (KT law 10), so every
# deliverable gets an ADVERSARIAL second pass from a separate process with a different brief.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$REPO" || exit 3
L="$REPO/var/loop"; mkdir -p "$L"
say() { printf '%s  %s\n' "$(date '+%F %T')" "$*" >> "$L/review.log"; }
# Single-flight. The fan-out driver had this and the reviewer did not, so the 30-minute scheduler
# and a manual kick both started a review of B1 and two codex processes wrote the same
# docs/REVIEW-B1.md concurrently. mkdir is the atomic primitive; macOS has no flock(1).
RLOCK="$L/review.lock"
if [ -d "$RLOCK" ]; then
  rp=$(cat "$RLOCK/pid" 2>/dev/null)
  case "$rp" in ''|0|1|*[!0-9]*) rp='' ;; esac
  if [ -n "$rp" ] && kill -0 "$rp" 2>/dev/null; then
    say "SKIP  reviewer already running (pid $rp)"; exit 0
  fi
  say "STALE reviewer lock from dead pid ${rp:-?} — reclaiming"; rm -rf "$RLOCK"
fi
mkdir -p "$RLOCK" || exit 3
echo $$ > "$RLOCK/pid"
trap 'rm -rf "$RLOCK"' EXIT INT TERM

reviewed=0
for f in docs/delta.d/*.md; do
  [ -e "$f" ] || continue
  ID=$(basename "$f" .md)
  [ -f "docs/REVIEW-$ID.md" ] && continue
  # never review a deliverable whose builder is still writing it
  if [ -f "$L/locks/$ID/pid" ] && kill -0 "$(cat "$L/locks/$ID/pid")" 2>/dev/null; then
    say "WAIT  $ID builder still running"; continue
  fi
  say "REVIEW $ID"
  {
    printf '# You are the ADVERSARIAL REVIEWER. You did not write this and you do not trust it.\n\n'
    printf 'Repo: %s (path contains a space — quote it). Background: handover/KT-CODEX.md.\n\n' "$REPO"
    printf 'Deliverable under review: docs/delta.d/%s.md\n' "$ID"
    printf 'Its acceptance criteria are item %s in handover/BACKLOG.md. Those criteria are the contract.\n\n' "$ID"
    cat <<'RB'
Your job is to find where it is wrong, not to confirm it is right.

1. RUN IT AS A USER. Do not read the prose and agree. Execute every command it cites. If it says
   a claim is IMPLEMENTED, run the thing and watch it resolve. An API 200 is not a rendered page;
   a green test is not a working feature. That is the most repeated failure in this project.
2. CHECK THE ARITHMETIC. Is the denominator published? Do the counts add up by hand? Were the
   hard cases classified or quietly dropped? A missing denominator is a fail on its own.
3. HUNT THE KNOWN CHEATS: a gate/margin/detector weakened to make something pass; `0` written
   where the honest value is `null`; a check that passes vacuously on empty input; a detector
   that fires on its own documentation; an item marked done whose log says PARTIAL or BLOCKED.
4. Verify independently: FLEET_MUTANTS=0 bash verify.sh — paste the real result including red.

Write docs/REVIEW-<ID>.md: what was claimed, what you actually ran (with exit codes), what you
observed, verdict ACCEPT / ACCEPT-WITH-FINDINGS / REJECT, and for a REJECT exactly what must change.
Finding a real defect is a SUCCESS for you. An ACCEPT with nothing found is the suspicious outcome.

Do NOT run git — concurrent workers share this checkout. Do not edit the deliverable, DELTA.md,
BACKLOG.md, or keel/. Create only docs/REVIEW-<ID>.md.
RB
  } > "$L/review-brief-$ID.txt"
  timeout 3600 codex exec --dangerously-bypass-approvals-and-sandbox -C "$REPO" \
    "$(cat "$L/review-brief-$ID.txt")" > "$L/review-$ID.log" 2>&1
  rrc=$?
  # A review with no verdict is not a review. Leave no file rather than a convincing stub.
  if [ -f "docs/REVIEW-$ID.md" ] && ! grep -qE '\b(ACCEPT|ACCEPT-WITH-FINDINGS|REJECT)\b' "docs/REVIEW-$ID.md"; then
    say "VOID  $ID review has no verdict (rc=$rrc) — removing so the next tick retries"
    rm -f "docs/REVIEW-$ID.md"
  fi
  v=$(grep -oE '\b(ACCEPT-WITH-FINDINGS|ACCEPT|REJECT)\b' "docs/REVIEW-$ID.md" 2>/dev/null | tail -1)
  say "VERDICT $ID ${v:-none}"
  reviewed=$((reviewed+1))
done
say "TICK  reviewed=$reviewed"
exit 0
