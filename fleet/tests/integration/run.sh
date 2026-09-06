#!/usr/bin/env bash
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PASS=0
FAIL=0
for test in "$ROOT"/tests/integration/{concurrency,crash-recovery,disk-full,huge-input,idempotence,permissions,clock}.sh; do
  "$test"
  rc=$?
  if [ "$rc" -eq 0 ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
  fi
done
printf '== integration scripts: %s passed, %s failed ==\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
