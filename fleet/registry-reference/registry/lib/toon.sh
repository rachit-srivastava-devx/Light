#!/usr/bin/env bash
# toon.sh — TOON emitters (AXI principle 1). Source, do not execute.
# TOON is line-oriented:  name[N]{f1,f2,f3}:  then one indented row per record.
# Quoting rule (fixed, tested): a value is quoted when it contains a comma, a double
# quote, a colon, a leading/trailing space, or is empty. Embedded quotes are doubled.
# Newlines and tabs are escaped to \n / \t because the format is line-oriented.
# Assumes bash 3.2. Pure: no wall-clock reads — callers pass timestamps in.
set -u

toon_quote() {
  local v="${1-}" needq=0
  # Decide on the ORIGINAL value (so a newline/tab still forces quotes after escaping),
  # then escape. Order matters: escaping first would hide the newline from the test.
  case "$v" in
    ""|*,*|*'"'*|*:*|' '*|*' '|*$'\n'*|*$'\t'*) needq=1 ;;
  esac
  v="${v//$'\n'/\\n}"; v="${v//$'\t'/\\t}"; v="${v//$'\r'/}"
  if [ "$needq" = 1 ]; then v="${v//\"/\"\"}"; printf '"%s"' "$v"; else printf '%s' "$v"; fi
}

# toon_preamble <tool> <description> <iso_ts>
toon_preamble() {
  printf 'tool: %s\n' "${1:?tool}"
  printf 'description: %s\n' "${2:?description}"
  printf 'generatedAt: %s\n' "$(toon_quote "${3:?iso_ts}")"
}

# toon_open <name> <count> <comma,separated,fields>
toon_open() { printf '%s[%s]{%s}:\n' "${1:?name}" "${2:?count}" "${3:?fields}"; }

# toon_row <val> [val...]
toon_row() {
  local first=1 v out=""
  for v in "$@"; do
    if [ "$first" = 1 ]; then first=0; else out="$out,"; fi
    out="$out$(toon_quote "$v")"
  done
  printf '  %s\n' "$out"
}

# toon_empty <name>  — AXI principle 5: definitive empty state, never blank output
toon_empty() { printf '%s[0]:\n' "${1:?name}"; }

# toon_next <suggestion> [more...] — AXI principle 9: contextual disclosure
toon_next() {
  printf 'help[%s]:\n' "$#"
  local s; for s in "$@"; do printf '  %s\n' "$s"; done
}
