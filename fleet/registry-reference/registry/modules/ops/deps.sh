#!/usr/bin/env bash
# deps.sh — dependency manifest with an explicit credential class.
# keyless dependencies may be required on the default path; needs-key dependencies are blocked
# or optional and are mechanically refused if anyone marks them required.
set -u

ROOT="${FLEET_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}"
# shellcheck source=../registry/lib/err.sh
. "$ROOT/registry/lib/err.sh"

# name|credential class|path status|probe|install hint
MANIFEST='bats|keyless|required|bats --version|brew install bats-core
shellcheck|keyless|required|shellcheck --version|brew install shellcheck
git|keyless|required|git --version|xcode-select --install
jq|keyless|required|jq --version|brew install jq
node|keyless|required|node --version|brew install node
python3|keyless|required|python3 --version|brew install python
sqlite3|keyless|required|sqlite3 --version|brew install sqlite
ccusage|keyless|required|ccusage --version|npm install -g ccusage
claude|keyless|agent|claude --version|install Claude Code CLI
codex|keyless|agent|codex --version|install Codex CLI
treehouse|keyless|required|treehouse --version|brew install treehouse
no-mistakes|keyless|required|no-mistakes --version|install no-mistakes release
firstmate|keyless|required|test -f vendor/firstmate/AGENTS.md|./fleet setup --only firstmate
tiktoken|keyless|required|test -x "$ROOT/var/venv/bin/python" && "$ROOT/var/venv/bin/python" -c "import tiktoken"|./fleet setup --only tiktoken
psutil|keyless|required|test -x "$ROOT/var/venv/bin/python" && "$ROOT/var/venv/bin/python" -c "import psutil"|./fleet setup --only psutil
llmlingua|keyless|optional|python3 -c "import llmlingua"|./fleet setup --only llmlingua
phoenix|keyless|optional|python3 -c "import phoenix"|./fleet setup --only arize-phoenix
quota-axi|keyless|optional|quota-axi --version|npm install -g quota-axi
mem0|needs-key|optional|false|optional only; requires a cloud embedder key and is not wired
console-web|keyless|optional|test -f console/web/package.json|./fleet setup --only console-web'

required_missing=0
agents=0
rows=0
printf '%-15s %-11s %-9s %-9s\n' NAME CREDENTIAL PATH STATUS
printf '%-15s %-11s %-9s %-9s\n' --------------- ----------- --------- --------

# Codex is an ESM Node entrypoint. Probe it through the same runtime repair that dispatch uses;
# `command -v codex` alone only proves that the wrapper exists, not that its interpreter works.
DEPS_PROBE_REASON=""
DEPS_NODE_BIN=""

deps_node_runs_esm() {
  local candidate="$1"
  [ -x "$candidate" ] || return 1
  "$candidate" --input-type=module -e 'import { spawn } from "node:child_process"; process.exit(0)' >/dev/null 2>&1
  local probe_ec=$?
  [ "$probe_ec" -eq 0 ]
}

deps_resolve_esm_node() {
  local candidate node_dir search_dirs ambient_node
  DEPS_NODE_BIN=""
  if [ -n "${FLEET_NODE_BIN:-}" ]; then
    candidate="$FLEET_NODE_BIN"
    case "$candidate" in
      */*) ;;
      *) candidate="$(command -v "$candidate" 2>/dev/null || true)" ;;
    esac
    if deps_node_runs_esm "$candidate"; then DEPS_NODE_BIN="$candidate"; return 0; fi
    return 1
  fi
  if [ "${FLEET_NODE_SEARCH_PATHS+x}" = x ]; then
    search_dirs="$FLEET_NODE_SEARCH_PATHS"
  else
    search_dirs="/opt/homebrew/bin:/usr/local/bin:/opt/local/bin:/usr/bin"
  fi
  ambient_node="$(command -v node 2>/dev/null || true)"
  if [ -n "$ambient_node" ] && deps_node_runs_esm "$ambient_node"; then
    DEPS_NODE_BIN="$ambient_node"
    return 0
  fi
  if [ "${FLEET_NODE_SEARCH_PATHS+x}" != x ]; then
    for node_dir in ${PATH//:/ }; do
      candidate="$node_dir/node"
      if deps_node_runs_esm "$candidate"; then DEPS_NODE_BIN="$candidate"; return 0; fi
    done
  fi
  for node_dir in ${search_dirs//:/ }; do
    candidate="$node_dir/node"
    if deps_node_runs_esm "$candidate"; then DEPS_NODE_BIN="$candidate"; return 0; fi
  done
  return 1
}

deps_probe_codex() {
  local codex_bin codex_path first_line env_path output probe_ec ambient_node
  DEPS_PROBE_REASON=""
  codex_bin="${FLEET_CODEX_BIN:-codex}"
  codex_path="$(command -v "$codex_bin" 2>/dev/null || true)"
  if [ -z "$codex_path" ]; then
    DEPS_PROBE_REASON="codex command '$codex_bin' was not found on PATH"
    return 1
  fi
  env_path="$PATH"
  first_line="$(sed -n '1p' "$codex_path" 2>/dev/null || true)"
  case "$first_line" in
    *node*)
      if ! deps_resolve_esm_node; then
        ambient_node="$(command -v node 2>/dev/null || printf '%s' 'not found')"
        DEPS_PROBE_REASON="codex wrapper found at $codex_path, but no ESM-capable Node.js interpreter was usable (ambient node: $ambient_node)"
        return 1
      fi
      env_path="$(dirname "$DEPS_NODE_BIN"):$PATH"
      ;;
  esac
  output="$(cd "$ROOT" && PATH="$env_path" "$codex_bin" --version 2>&1)"
  probe_ec=$?
  if [ "$probe_ec" -ne 0 ]; then
    DEPS_PROBE_REASON="codex --version failed with exit $probe_ec: $(printf '%s\n' "$output" | tail -5 | tr '\n' ' ')"
    return 1
  fi
  return 0
}

while IFS='|' read -r name credential path_status probe install_hint; do
  [ -n "$name" ] || continue
  rows=$((rows + 1))
  if [ "$credential" = needs-key ] && [ "$path_status" = required ]; then
    printf 'deps: needs-key dependency on required path: %s\n' "$name" >&2
    exit "$ERR_INVARIANT"
  fi
  if [ "$path_status" = blocked ]; then
    printf '%-15s %-11s %-9s %-9s\n' "$name" "$credential" "$path_status" blocked
    continue
  fi
  if [ "$path_status" = agent ]; then
    if [ "$name" = codex ]; then
      if deps_probe_codex; then
        dep_status=ok
        agents=$((agents + 1))
      else
        dep_status=unusable
        printf '  reason: %s\n' "$DEPS_PROBE_REASON"
      fi
    elif (cd "$ROOT" && eval "$probe") >/dev/null 2>&1; then
      dep_status=ok
      agents=$((agents + 1))
    else
      dep_status=absent
      printf '  reason: %s\n' "${name} --version probe failed"
    fi
  elif (cd "$ROOT" && eval "$probe") >/dev/null 2>&1; then
    dep_status=ok
  else
    dep_status=missing
    [ "$path_status" = required ] && required_missing=$((required_missing + 1))
  fi
  printf '%-15s %-11s %-9s %-9s\n' "$name" "$credential" "$path_status" "$dep_status"
  [ "$dep_status" = missing ] && printf '  install: %s\n' "$install_hint"
done <<EOF
$MANIFEST
EOF

printf '\nmanifest rows: %s required missing: %s agents present: %s\n' "$rows" "$required_missing" "$agents"
[ "$agents" -gt 0 ] || die "$ERR_NOTOOL" no_agent 'no Claude or Codex CLI found' 'install claude or codex CLI'
[ "$required_missing" -eq 0 ] || die "$ERR_NOTOOL" deps_missing "$required_missing required dependency/ies missing" 'run ./fleet setup for the missing tools'
printf '%s\n' 'deps: keyless required path is clear; needs-key paths are blocked/optional'
exit "$ERR_OK"
