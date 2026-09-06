#!/usr/bin/env bash
# test.sh — Google-style size runner. Default is the fast hermetic small lane.
# small: one process, no network, temp-dir filesystem, <1s per test file.
# medium: localhost/filesystem or real local tooling, <10s per test file.
# large: real console/upstream binaries, minutes.
# Each lane runs BOTH *.bats and *.test.sh directly under tests/<size>/.
# tests/large/slow/argsafety.test.sh is intentionally excluded: it is a known hanging probe
# suite, and the non-recursive glob is what excludes it. Reach it with the `slow` lane.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RUN_TMP="$(mktemp -d "${TMPDIR:-/tmp}/fleet-tests.XXXXXX")"
trap 'rm -rf "$RUN_TMP"' EXIT HUP INT TERM
KEYLESS_ENV=(-u ANTHROPIC_API_KEY -u OPENAI_API_KEY -u GITHUB_TOKEN -u GOOGLE_API_KEY -u GEMINI_API_KEY -u COHERE_API_KEY -u MISTRAL_API_KEY -u GROQ_API_KEY -u TOGETHER_API_KEY -u XAI_API_KEY -u DEEPSEEK_API_KEY -u OPENROUTER_API_KEY -u CCR_API_KEY)

usage() {
  printf '%s\n' \
    'test.sh [small|medium|large|all|slow]' \
    '  default: small (hermetic, no network, no API-key environment)' \
    '  medium: localhost/filesystem and local-tool tests' \
    '  large: real upstream binaries and console tests; telemetry is skipped because it binds a port' \
    '  all: small + medium + large, never tests/large/slow' \
    '  slow: explicit opt-in only; tests/large/slow/argsafety.test.sh is known to hang'
}

run_file() {
  local file="$1" kind name ledger ec
  kind="${file##*.}"
  name="$(basename "$file")"
  ledger="$RUN_TMP/$name.jsonl"
  printf '\n== %s ==\n' "$file"
  if [ "$kind" = bats ]; then
    env "${KEYLESS_ENV[@]}" FLEET_LEDGER="$ledger" bats "$file"
    ec=$?
  else
    env "${KEYLESS_ENV[@]}" FLEET_LEDGER="$ledger" bash "$file"
    ec=$?
  fi
  printf 'exit=%s %s\n' "$ec" "$file"
  return "$ec"
}

run_size() {
  local size="$1" file ec=0
  shopt -s nullglob
  # Both kinds run: run_file already dispatches .bats to bats and everything else to bash.
  # The glob is deliberately NON-recursive -- that is what keeps the known-hanging
  # tests/large/slow/argsafety.test.sh out of the large lane (it stays opt-in via `slow`).
  # Shared helpers such as tests/small/test-lib.sh are not matched: *.test.sh is required.
  for file in "$ROOT/tests/$size"/*.bats "$ROOT/tests/$size"/*.test.sh; do
    if [ "$(basename "$file")" = telemetry.bats ]; then
      printf '\n== skipped %s ==\nreason: hard constraint forbids binding a network port\n' "$file"
      continue
    fi
    run_file "$file" || ec=1
  done
  if [ "$size" = medium ] && [ -d "$ROOT/tests/medium/property" ] && command -v pytest >/dev/null 2>&1; then
    printf '\n== tests/medium/property ==\n'
    env "${KEYLESS_ENV[@]}" pytest -q "$ROOT/tests/medium/property"
    [ "$?" -eq 0 ] || ec=1
  fi
  return "$ec"
}

lane="${1:-small}"
case "$lane" in
  -h|--help) usage; exit 0 ;;
  small|medium|large) run_size "$lane"; exit "$?" ;;
  all)
    result=0
    for lane_part in small medium large; do
      run_size "$lane_part" || result=1
    done
    exit "$result"
    ;;
  slow)
    printf '%s\n' 'slow lane is opt-in and known to hang: tests/large/slow/argsafety.test.sh'
    run_file "$ROOT/tests/large/slow/argsafety.test.sh"
    exit "$?"
    ;;
  *) printf 'test.sh: unknown lane %s\n' "$lane" >&2; usage >&2; exit 2 ;;
esac
