#!/usr/bin/env bash
# alive.sh — is a background run actually alive?
#
# `pgrep -fc 'codex exec'` RETURNS 0 FOR A LIVE PROCESS when the prompt passed as an argument
# contains newlines — the pattern never matches the flattened arg string. That false zero was read
# all night as "the agents finished", and at least one live run was very nearly killed on it.
#
# File growth is the honest signal: a working agent writes. `ps -p` on a captured pid is the second.
# Neither depends on matching a multi-line command line.
set -u
D="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
W="${1:-3}"
printf '%-22s %10s %10s  %s\n' TRACK LINES DELTA STATUS
printf '%-22s %10s %10s  %s\n' ---------------------- ---------- ---------- ------
files=""
while IFS= read -r f; do [ -n "$f" ] && files="$files$f\n"; done <<EOT
$(find "$D/var/runs" -maxdepth 2 -type f -name 'out.log' -print 2>/dev/null | head -12)
EOT
snap="$(mktemp "${TMPDIR:-/tmp}/alive.XXXXXX")"
printf '%b' "$files" | while IFS= read -r f; do
  [ -n "$f" ] || continue
  printf '%s\t%s\n' "$f" "$(wc -l < "$f" 2>/dev/null | tr -d ' ')" >>"$snap"
done
sleep "$W"
while IFS="$(printf '\t')" read -r f before; do
  [ -n "$f" ] || continue
  now="$(wc -l < "$f" 2>/dev/null | tr -d ' ')"
  d=$(( now - before ))
  if [ "$d" -gt 0 ]; then st="ALIVE"; else
    # No growth in the window is not proof of death — an agent can think for a while. Age the file.
    age=$(( $(date +%s) - $(stat -f %m "$f" 2>/dev/null || echo 0) ))
    if [ "$age" -lt 120 ]; then st="thinking (${age}s idle)"; else st="done/stalled (${age}s idle)"; fi
  fi
  printf '%-22s %10s %10s  %s\n' "$(basename "$f" .out)" "$now" "$d" "$st"
done <"$snap"
rm -f "$snap"
