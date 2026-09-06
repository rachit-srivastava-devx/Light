#!/usr/bin/env bash
# Stop hook: the session may not end on a red verify gate — but ONLY when code changed this session.
#
# Docs/config/chat sessions never leave the marker (l8-auto-verify.sh writes it on a code edit), so
# this gate is invisible to them. When code did change, it finds the repo's verify command and runs
# it; a red gate blocks the stop (exit 2, stderr fed back to the model) so the agent cannot
# stop-and-claim-done on a build it hasn't proven green. Loop-guarded via stop_hook_active; gives up
# gracefully after 5 red attempts so the human is never trapped.
#
# Gate resolution order (first hit wins):
#   1. $CLAUDE_PROJECT_DIR/.fleet/gate.cmd          (explicit, autopilot-style)
#   2. package.json "verify" script  -> npm run -s verify
#   3. Makefile "verify:" target     -> make -s verify
#   4. pyproject.toml/pytest.ini/tests/ -> pytest -q
#   5. package.json "test" script    -> npm test --silent
# No gate found -> exit 0 (can't verify here; don't trap the session).
#
# Builder-only constraint: the lead's first contract suite is SUPPOSED to be red (T1), and the
# verifier is read-only — both are exempt via FLEET_ROLE.
set -u
INPUT="$(cat)"

case "${FLEET_ROLE:-}" in lead|verifier|initializer) exit 0 ;; esac

ROOT="${CLAUDE_PROJECT_DIR:-$PWD}"
MARKER="$ROOT/.fleet/.session-touched-code"
COUNT_FILE="$ROOT/.fleet/.l8-stop-count"

# No code changed this session -> nothing to gate.
[ -f "$MARKER" ] || { rm -f "$COUNT_FILE" 2>/dev/null; exit 0; }

# --- resolve the gate command ---
GATE=""
if [ -f "$ROOT/.fleet/gate.cmd" ]; then
  GATE="$(cat "$ROOT/.fleet/gate.cmd")"
elif [ -f "$ROOT/package.json" ] && grep -qE '"verify"[[:space:]]*:' "$ROOT/package.json"; then
  GATE="npm run -s verify"
elif [ -f "$ROOT/Makefile" ] && grep -qE '^verify:' "$ROOT/Makefile"; then
  GATE="make -s verify"
elif [ -f "$ROOT/pyproject.toml" ] || [ -f "$ROOT/pytest.ini" ] || [ -d "$ROOT/tests" ]; then
  command -v pytest >/dev/null 2>&1 && GATE="pytest -q"
elif [ -f "$ROOT/package.json" ] && grep -qE '"test"[[:space:]]*:' "$ROOT/package.json"; then
  GATE="npm test --silent"
fi

# No resolvable gate -> don't trap the session; leave a gentle note.
if [ -z "$GATE" ]; then
  echo '{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"L8 gate: code changed but no verify command was found in this repo. Verify manually before treating this as done."}}'
  exit 0
fi

# --- loop guard ---
ACTIVE="$(printf '%s' "$INPUT" | jq -r '.stop_hook_active // false' 2>/dev/null)"
if [ "$ACTIVE" = "true" ]; then
  N=$(( $(cat "$COUNT_FILE" 2>/dev/null || echo 0) + 1 ))
  echo "$N" > "$COUNT_FILE"
  if [ "$N" -ge 5 ]; then
    rm -f "$COUNT_FILE"
    echo "⟦L8 gate⟧ still red after 5 attempts — allowed to stop. Document the blockers in claude-progress.txt before you go." >&2
    exit 0
  fi
fi

# --- run the gate, quietly ---
mkdir -p "$ROOT/.fleet/logs" 2>/dev/null
LOG="$ROOT/.fleet/logs/l8-stop-gate-$(date +%s).log"
if ( cd "$ROOT" && bash -c "$GATE" ) > "$LOG" 2>&1; then
  rm -f "$COUNT_FILE" "$MARKER" 2>/dev/null   # green: clear the session so re-stops are free
  exit 0
else
  {
    echo "⟦L8 gate⟧ STOP BLOCKED — verify is red: $GATE"
    echo "Last 15 lines (full log: $LOG):"
    tail -15 "$LOG"
    echo "Fix the failures, re-run the gate to green, note it in claude-progress.txt, then finish."
  } >&2
  exit 2
fi
