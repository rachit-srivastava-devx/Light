#!/usr/bin/env bash
# shellcheck disable=SC2207
# install-l8-default.sh — propagate L8 DEFAULT MODE into buildable subrepos.
#
# The workspace root is wired via .claude/settings.json (references fleet/hooks/l8-*.sh directly).
# But a `claude` launched INSIDE company/products/<repo> has that repo as CLAUDE_PROJECT_DIR, so the
# root hooks don't apply. This script copies the L8 hooks + contract into each such repo's
# .claude/hooks/ and merges the hook wiring into its .claude/settings.json — idempotently, additively,
# and without clobbering existing autopilot hooks (it skips a second Stop gate if one already exists).
#
# Usage:
#   fleet/registry/modules/cli/install-l8-default.sh [--dry-run] [repo ...]
#   (no repo args -> auto-discover company/products/* that look like projects)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"   # workspace root (…/Principal Engineering)
SRC="$ROOT/hooks"
CONTRACT="$ROOT/docs/guides/L8-OPERATING-CONTRACT.md"
DRY=0
TARGETS=()

for a in "$@"; do
  case "$a" in
    --dry-run) DRY=1 ;;
    *) TARGETS+=("$a") ;;
  esac
done

command -v jq >/dev/null 2>&1 || { echo "FATAL: jq required." >&2; exit 1; }
[ -f "$CONTRACT" ] || { echo "FATAL: $CONTRACT missing." >&2; exit 1; }

# Auto-discover if no explicit targets. NUL-delimited to survive spaces in paths.
if [ "${#TARGETS[@]}" -eq 0 ]; then
  while IFS= read -r -d '' f; do TARGETS+=("$(dirname "$f")"); done < <(
    find "$ROOT/company/products" -maxdepth 2 \
      \( -name package.json -o -name pyproject.toml -o -name go.mod -o -name Cargo.toml \) \
      -not -path '*/node_modules/*' -print0 2>/dev/null
  )
  # de-dup
  if [ "${#TARGETS[@]}" -gt 0 ]; then
    IFS=$'\n' TARGETS=($(printf '%s\n' "${TARGETS[@]}" | sort -u)); unset IFS
  fi
fi

[ "${#TARGETS[@]}" -eq 0 ] && { echo "No target repos found under company/products/."; exit 0; }

# jq program: additively add the L8 hooks. Idempotent (keyed on the l8-* script name). Skips adding
# the Stop gate when ANY Stop hook already references a *stop-gate* script (autopilot's or ours).
# The command path is quoted for spaces ("Principal Engineering"); ${CLAUDE_PROJECT_DIR} is baked as a
# literal token that Claude Code resolves at run time.
read -r -d '' JQ <<'JQEOF' || true
def has_cmd(evt; needle):
  ([ .hooks[evt][]?.hooks[]?.command // empty ] | any(contains(needle)));
def add_group(evt; group):
  .hooks = ((.hooks // {}) | .[evt] = (((.[evt]) // []) + [group]));

(if has_cmd("SessionStart"; "l8-session-context") then .
 else add_group("SessionStart";
   {hooks:[{type:"command",command:("bash \""+$P+"/.claude/hooks/l8-session-context.sh\""),timeout:10,statusMessage:"loading L8 operating contract"}]})
 end)
| (if has_cmd("PostToolUse"; "l8-auto-verify") then .
 else add_group("PostToolUse";
   {matcher:"Edit|Write|MultiEdit",hooks:[{type:"command",command:("bash \""+$P+"/.claude/hooks/l8-auto-verify.sh\""),timeout:40,statusMessage:"L8 auto-verify"}]})
 end)
| (if has_cmd("Stop"; "stop-gate") then .
 else add_group("Stop";
   {hooks:[{type:"command",command:("bash \""+$P+"/.claude/hooks/l8-stop-gate.sh\""),timeout:600,statusMessage:"L8 verify gate"}]})
 end)
| (if has_cmd("PreToolUse"; "injection-guard") then .
 else add_group("PreToolUse";
   {matcher:"Bash|Write|Edit|MultiEdit",hooks:[{type:"command",command:("bash \""+$P+"/.claude/hooks/injection-guard.sh\""),timeout:10,statusMessage:"injection-guard"}]})
 end)
JQEOF

for repo in "${TARGETS[@]}"; do
  repo="$(cd "$repo" && pwd)"
  echo "== $repo =="
  if [ "$DRY" = 0 ]; then
    mkdir -p "$repo/.claude/hooks"
    for f in l8-session-context.sh l8-auto-verify.sh l8-stop-gate.sh injection-guard.sh; do
      cp "$SRC/$f" "$repo/.claude/hooks/$f"; chmod +x "$repo/.claude/hooks/$f"
    done
    cp "$CONTRACT" "$repo/.claude/hooks/L8-DEFAULT.md"
  else
    echo "   would copy l8-*.sh + injection-guard.sh + L8-DEFAULT.md -> .claude/hooks/"
  fi

  SETTINGS="$repo/.claude/settings.json"
  BASE='{}'
  [ -f "$SETTINGS" ] && BASE="$(cat "$SETTINGS")"
  MERGED="$(printf '%s' "$BASE" | jq --arg P '${CLAUDE_PROJECT_DIR}' "$JQ")"

  if ! printf '%s' "$MERGED" | jq empty 2>/dev/null; then
    echo "   ERROR: merge produced invalid JSON — skipping (settings untouched)." >&2
    continue
  fi

  if [ "$DRY" = 1 ]; then
    echo "   --- merged .hooks (dry-run) ---"
    printf '%s\n' "$MERGED" | jq '.hooks'
  else
    [ -f "$SETTINGS" ] && cp "$SETTINGS" "$SETTINGS.bak-$(date +%Y%m%d-%H%M%S)"
    printf '%s\n' "$MERGED" > "$SETTINGS"
    echo "   installed"
  fi
done

echo "Done. Restart any claude session inside a target repo for its hooks to load."
