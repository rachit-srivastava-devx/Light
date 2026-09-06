#!/usr/bin/env bash
# SessionStart hook: inject the compact L8 operating contract into every session.
# For SessionStart, plain stdout is added to Claude's context (docs: code.claude.com/docs/en/hooks),
# so we simply print the injected block from fleet/L8-DEFAULT.md. Fail-safe: never errors the session.
set -u

# Resolve the contract file relative to this script, with a project-dir fallback.
SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
for CAND in \
  "$SELF_DIR/../L8-DEFAULT.md" \
  "${CLAUDE_PROJECT_DIR:-}/fleet/L8-DEFAULT.md" \
  "${CLAUDE_PROJECT_DIR:-}/.claude/hooks/L8-DEFAULT.md"; do
  if [ -f "$CAND" ]; then CONTRACT="$CAND"; break; fi
done

[ -z "${CONTRACT:-}" ] && exit 0   # nothing to inject; do not disturb the session

# Print only the region between the INJECT markers (keeps the header/comments out of context).
awk '/<!-- BEGIN-INJECT -->/{f=1;next} /<!-- END-INJECT -->/{f=0} f' "$CONTRACT" 2>/dev/null \
  || cat "$CONTRACT" 2>/dev/null || true
exit 0
