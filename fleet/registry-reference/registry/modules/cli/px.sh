#!/usr/bin/env bash
# px.sh — GUARDED pxpipe launcher (token policy, goal #4).
#
# pxpipe (github.com/teamchong/pxpipe, MIT) renders bulky text context as dense PNGs and cuts
# Claude token usage 59-70%. BUT it is LOSSY: it garbles hashes/UUIDs/secrets, and Opus/Sonnet
# misread dense renders ~7% of the time. Fable-5 reads them near-perfectly.
#
# THE RULE (enforced below): pxpipe is allowed ONLY for Fable-5 bulk/grunt loops
# (log triage, changelog/docstring drafts, README first-passes, mass summarize).
# It is FORBIDDEN for Opus/Sonnet contract, code, migration, or money work — a 7% misread
# on a schema or an ID is a correctness incident, not a saving.
#
#   Usage:  px.sh <claude args...>        # model must resolve to fable-5 (or haiku)
set -u
MODEL="${FLEET_MODEL_RESEARCH:-fable}"
case "$MODEL" in
  fable|claude-fable-5|haiku|haiku-4-5) ;;
  *) echo "px.sh: refusing model '$MODEL'. pxpipe is lossy on Opus/Sonnet (correctness risk)." >&2
     echo "       Use it only for Fable-5 bulk work: FLEET_MODEL_RESEARCH=fable px.sh ..." >&2
     exit 1 ;;
esac
if ! command -v pxpipe >/dev/null 2>&1 && ! npx --no-install pxpipe --version >/dev/null 2>&1; then
  echo "px.sh: pxpipe not installed. Install (opt-in): npm i -g pxpipe   (or run via: npx pxpipe)" >&2
  echo "       Details + caveats: fleet/TOKEN-POLICY.md" >&2
  exit 1
fi
echo "px.sh: pxpipe proxy active for model=$MODEL (Fable-5 bulk lane). Do NOT pipe secrets/IDs through this." >&2
# pxpipe runs a local proxy Claude Code points at (ANTHROPIC_BASE_URL). Exact flags: see its README.
exec pxpipe -- claude --model "$MODEL" "$@"
