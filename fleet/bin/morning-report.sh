#!/usr/bin/env bash
# Assembles what happened overnight into one thing to read at 9am, plus concrete commands
# to run by hand. Reuses plain grep/git log — no new tooling.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$REPO" || exit 3
OUT="$REPO/var/loop/MORNING-REPORT.md"

{
  echo "# Morning report — $(date '+%Y-%m-%d %H:%M %Z')"
  echo
  echo "## Commits since midnight"
  echo '```'
  git log --since="today 00:00" --format='%ai  %h  %s' 2>/dev/null || echo "(none — git log failed or nothing committed)"
  echo '```'
  echo
  echo "## Phase state"
  echo '```'
  grep -oE '^## \[.\] [A-Z0-9]+ — .*' handover/BACKLOG.md | head -25
  echo '```'
  echo
  echo "## Things to actually run and look at yourself"
  echo
  found=0
  for f in docs/delta.d/*.md; do
    [ -f "$f" ] || continue
    line=$(grep -m1 '^MORNING_TEST:' "$f" 2>/dev/null) || continue
    found=1
    id="$(basename "$f" .md)"
    echo "- **$id**: ${line#MORNING_TEST: }"
  done
  if [ "$found" -eq 0 ]; then
    echo "_No item finished with a MORNING_TEST line yet — nothing new to manually try today."
    echo "This is a real state, not a bug: check \`var/loop/fanout.log\` for what's still in flight._"
  fi
  echo
  echo "## Last known verify.sh summary + what's red in it"
  if pgrep -f 'bash verify\.sh' >/dev/null 2>&1; then
    echo
    echo "**verify.sh is running RIGHT NOW (probably a concurrent worker) — var/verify.log is a**"
    echo "**single shared, non-parameterized path, so under FLEET_MAXPAR>1 it can be read while"
    echo "another worker is mid-write. Treat the block below as possibly stale/interleaved, not fact.**"
    echo "(Real bug, not this script's — see docs/delta.d/B-VERIFY-LOG-RACE.md.)"
    echo
  fi
  echo '```'
  grep -E '^-- [0-9]+ passed' var/verify.log 2>/dev/null | tail -1 || echo "(no recent verify.sh summary recorded)"
  grep -B1 'FAIL ' var/verify.log 2>/dev/null | grep -v '^--$' | tail -10
  echo '```'
} > "$OUT"

cat "$OUT"
