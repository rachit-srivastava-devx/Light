#!/usr/bin/env bash

set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FLEET="$ROOT/keel/target/debug/fleet"
PASS=0
FAIL=0
RUN_RC=0

ok() {
  PASS=$((PASS + 1))
  printf '  ok   %s\n' "$1"
}

no() {
  FAIL=$((FAIL + 1))
  printf '  FAIL %s   %s\n' "$1" "${2:-}"
}

finish() {
  printf '== %s passed, %s failed ==\n' "$PASS" "$FAIL"
  [ "$FAIL" -eq 0 ]
}

require_fleet() {
  if [ ! -x "$FLEET" ]; then
    printf 'FATAL: %s is not built; expected environment exit 3\n' "$FLEET"
    return 3
  fi
  return 0
}

new_tmp() {
  mktemp -d "${TMPDIR:-/tmp}/fleet-integ.XXXXXX"
}

make_repo() {
  local repo="$1"
  local marker="$2"
  mkdir -p "$repo"
  (
    cd "$repo" || exit 3
    git init -q || exit 3
    printf 'fn main() { println!("%s"); }\n' "$marker" > main.rs
    git add -A || exit 3
    git -c user.email=fleet@test -c user.name=fleet commit -qm init || exit 3
  )
}

run_capture() {
  local output="$1"
  shift
  "$@" >"$output" 2>&1
  RUN_RC=$?
  return 0
}

assert_rc() {
  local expected="$1"
  local actual="$2"
  local label="$3"
  if [ "$actual" -eq "$expected" ]; then
    ok "$label (exit $expected)"
  else
    no "$label (exit $expected)" "rc=$actual"
  fi
}

assert_output_has() {
  local output="$1"
  local needle="$2"
  local label="$3"
  if python3 - "$output" "$needle" <<'PY'
import sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
raise SystemExit(0 if sys.argv[2] in text else 1)
PY
  then
    ok "$label"
  else
    no "$label" "output=$(python3 - "$output" <<'PY'
import sys
print(open(sys.argv[1], encoding="utf-8", errors="replace").read()[-400:])
PY
)"
  fi
}

assert_output_nonempty() {
  local output="$1"
  local label="$2"
  if [ -s "$output" ]; then
    ok "$label"
  else
    no "$label" 'empty output'
  fi
}

assert_output_captured() {
  local output="$1"
  local label="$2"
  if [ -f "$output" ]; then
    ok "$label"
  else
    no "$label" 'output capture missing'
  fi
}

ledger_dump_to() {
  local output="$1"
  "$FLEET" ledger dump >"$output" 2>&1
  RUN_RC=$?
  return 0
}

ledger_verify() {
  local output="$1"
  "$FLEET" ledger verify >"$output" 2>&1
  RUN_RC=$?
  return 0
}

artifact_id_from() {
  python3 - "$1" <<'PY'
import re, sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
ids = re.findall(r"artifact=([0-9a-f]{64})", text)
print(ids[0] if ids else "")
PY
}

assert_no_agent_processes() {
  local ps_file="$1"
  ps -axo pid=,command= >"$ps_file" 2>&1
  RUN_RC=$?
  if [ "$RUN_RC" -ne 0 ]; then
    no "no orphaned worker processes" "ps rc=$RUN_RC"
    return 0
  fi
  if python3 - "$ps_file" "$FLEET" <<'PY'
import os, sys
fleet = sys.argv[2]
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    fields = line.strip().split(None, 1)
    if len(fields) == 2 and fields[0].isdigit() and int(fields[0]) != os.getpid():
        if fleet in fields[1] and "__agent" in fields[1]:
            raise SystemExit(1)
raise SystemExit(0)
PY
  then
    ok "no orphaned worker processes"
  else
    no "no orphaned worker processes" "$(python3 - "$ps_file" "$FLEET" <<'PY'
import sys
fleet = sys.argv[2]
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    if fleet in line and "__agent" in line:
        print(line.rstrip())
PY
)"
  fi
}
