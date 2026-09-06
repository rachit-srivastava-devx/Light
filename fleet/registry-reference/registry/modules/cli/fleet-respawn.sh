#!/usr/bin/env bash
# Quota-respawn watcher. Runs every 10 min via launchd. Zero tokens when idle.
# 1) If any fleet tmux pane shows the usage-limit message at rest -> send "continue".
#    (Safe to retry early: if still limited, Claude Code just re-prints the message.)
# 2) If the firstmate session died but backlog has open work -> relaunch it resumed.
set -u
LOG="$HOME/.fleet/respawn.log"
SESSION="${FM_SESSION:-firstmate}"
FM_HOME="${FM_HOME:-$HOME/firstmate}"
LIMIT_RE='usage limit reached|out of (extra )?usage|limit resets|resets [0-9]{1,2}(:[0-9]{2})?(am|pm)'
log() { echo "$(date '+%F %T') $*" >> "$LOG"; }

command -v tmux >/dev/null || exit 0

# --- 1) nudge rate-limited panes -------------------------------------------
tmux list-panes -a -F '#{session_name}:#{window_index}.#{pane_index}|#{window_name}' 2>/dev/null |
while IFS='|' read -r target wname; do
  case "$wname" in fm-*|firstmate|captain|claude*) ;; *) continue ;; esac
  tail="$(tmux capture-pane -pt "$target" -S -25 2>/dev/null)" || continue
  if echo "$tail" | grep -qiE "$LIMIT_RE"; then
    # only nudge if pane looks idle (no active spinner/busy signature)
    if ! echo "$tail" | grep -qiE 'esc (to )?interrupt|Working…|Working\.\.\.'; then
      tmux send-keys -t "$target" "continue" Enter
      log "nudged $target ($wname) after limit message"
    fi
  fi
done

# --- 2) resurrect a dead fleet if work remains ------------------------------
if ! tmux has-session -t "$SESSION" 2>/dev/null; then
  if [ -f "$FM_HOME/data/backlog.md" ] && grep -qiE '^\s*[-*] \[ \]|pending|in.progress' "$FM_HOME/data/backlog.md"; then
    tmux new-session -d -s "$SESSION" -c "$FM_HOME" 'claude -c'
    sleep 20
    tmux send-keys -t "$SESSION" "ahoy — reconcile the fleet and continue all unfinished backlog work" Enter
    log "resurrected session '$SESSION' with open backlog"
  fi
fi
