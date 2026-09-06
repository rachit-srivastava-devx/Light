#!/usr/bin/env bash
# Regenerates Light/STATUS.md from Light/status/*.status files.
# Any agent (or the orchestrator) can run this any time: bash status/render.sh
set -euo pipefail
cd "$(dirname "$0")/.."   # -> Light/

rows="$(mktemp)"
trap 'rm -f "$rows"' EXIT

for f in status/*.status; do
  [ -e "$f" ] || continue
  id=$(grep '^ID='      "$f" | head -1 | cut -d= -f2-)
  feature=$(grep '^FEATURE=' "$f" | head -1 | cut -d= -f2-)
  repo=$(grep '^REPO='    "$f" | head -1 | cut -d= -f2-)
  status=$(grep '^STATUS='  "$f" | head -1 | cut -d= -f2-)
  agent=$(grep '^AGENT='   "$f" | head -1 | cut -d= -f2-)
  since=$(grep '^SINCE='   "$f" | head -1 | cut -d= -f2-)
  note=$(grep '^NOTE='    "$f" | head -1 | cut -d= -f2-)
  printf '%s|%s|%s|%s|%s|%s|%s\n' "$id" "$feature" "$repo" "$status" "$agent" "$since" "$note" >> "$rows"
done

{
  echo "# Light/ build status"
  echo
  echo "_Regenerated $(date '+%Y-%m-%d %H:%M %Z') from \`status/*.status\` — do not hand-edit this file; edit your feature's own status file and re-run \`bash status/render.sh\`._"
  echo
  echo "| # | Feature | Repo | Status | Agent | Since | Note |"
  echo "|---|---|---|---|---|---|---|"
  if [ -s "$rows" ]; then
    sort -t'|' -k1,1 -V "$rows" | awk -F'|' '{printf "| %s | %s | %s | %s | %s | %s | %s |\n", $1,$2,$3,$4,$5,$6,$7}'
  else
    echo "| — | (no features registered yet) | — | — | — | — | — |"
  fi
} > STATUS.md

n=$(wc -l < "$rows" | tr -d ' ')
echo "Wrote STATUS.md ($n feature rows)"
