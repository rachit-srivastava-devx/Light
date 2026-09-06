#!/usr/bin/env bash
# What is the autonomous loop doing right now?
R="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$R" || exit 3
echo "== scheduler =="
launchctl list 2>/dev/null | grep -E 'PID|ai\.fleet\.codex-loop' | head -2 || echo "  NOT LOADED"
echo "== now =="
if [ -d var/loop/tick.lock ] && kill -0 "$(cat var/loop/tick.lock/pid 2>/dev/null||echo 0)" 2>/dev/null; then
  echo "  RUNNING $(cat var/loop/tick.lock/item 2>/dev/null) since $(cat var/loop/tick.lock/started 2>/dev/null)"
else echo "  idle"; fi
echo "== backlog =="; grep -E '^## \[' handover/BACKLOG.md | sed 's/^## /  /'
echo "== last 8 log lines =="; tail -8 var/loop/tick.log 2>/dev/null || echo "  (none yet)"
echo "== progress =="; tail -5 handover/PROGRESS.md 2>/dev/null
echo "== repo =="; printf "  commits %s | head %s | dirty %s\n" \
  "$(git log --oneline|wc -l|tr -d ' ')" "$(git rev-parse --short HEAD)" \
  "$(git status --porcelain|grep -vcE '\.pyc|__pycache__|var/')"
