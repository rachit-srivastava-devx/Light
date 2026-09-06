#!/usr/bin/env bash

# Mutation adequacy gate for the refusal-kernel ratchet module. Keep this scoped:
# running cargo-mutants over the whole fleet crate takes hours.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FLOOR_FILE="$ROOT/bin/mutants-floor.txt"

if ! command -v cargo-mutants >/dev/null 2>&1; then
  echo "mutants: cargo-mutants unavailable"
  exit 3
fi

if [ ! -r "$FLOOR_FILE" ]; then
  echo "mutants: REFUSE: floor file unavailable: $FLOOR_FILE"
  exit 6
fi

floor=$(tr -d '[:space:]' < "$FLOOR_FILE")
case "$floor" in
  ''|*[!0-9/]*|*/*/*|/*|*/)
    echo "mutants: REFUSE: floor must be an integer fraction, got '$floor'"
    exit 6
    ;;
esac
floor_caught=${floor%/*}
floor_total=${floor#*/}
# An UNCALIBRATED floor must refuse, not pass. A committed floor of 0 can never be violated by any
# real score, so the gate would report success while asserting nothing -- the D19 shape exactly.
# Refuse until a measured floor is committed.
if [ "${floor_caught}" = "0" ]; then
  echo "mutants: REFUSE: floor is ${floor} -- a numerator of 0 cannot be violated by any score."
  echo "  This gate asserts nothing until calibrated. Run a full cargo-mutants pass over"
  echo "  keel/fleet/src/ratchet.rs and commit the measured caught/total to bin/mutants-floor.txt."
  exit 6
fi
case "$floor_caught" in 0|[1-9]|[1-9][0-9]*) ;; *)
  echo "mutants: REFUSE: floor numerator is not canonical: '$floor_caught'"
  exit 6
  ;;
esac
case "$floor_total" in [1-9]|[1-9][0-9]*) ;; *)
  echo "mutants: REFUSE: floor denominator must be a positive canonical integer"
  exit 6
  ;;
esac
if [ "$floor_caught" -gt "$floor_total" ]; then
  echo "mutants: REFUSE: floor cannot exceed 1: '$floor'"
  exit 6
fi

run_dir=$(mktemp -d)
cleanup() {
  rm -rf "$run_dir"
}
trap cleanup EXIT
trap 'exit 130' HUP INT TERM

cd "$ROOT"
# B21: the default copy-to-tempdir mode scopes the copy to keel/ (the --manifest-path's own
# workspace root), but the crate's test suite needs repo-root files outside that scope
# (agents.toml/skills.toml, resolved via env!("CARGO_MANIFEST_DIR") at compile time) plus python3
# on PATH -- both present in the real checkout, absent from the copy. That made the unmutated
# baseline fail before a single mutant was ever tested (verified 2026-09-03; the last real score
# was 2026-08-24). --in-place tests directly in this checkout instead, where those dependencies
# already resolve correctly. Tradeoff: --in-place mutates real source files one at a time, so this
# cannot safely run with --jobs>1 (concurrent mutants would corrupt each other's edits). cargo-mutants
# actually REJECTS --jobs together with --in-place outright (verified: exit 1, "the argument
# '--jobs <JOBS>' cannot be used with '--in-place'") -- correct fix is to omit --jobs entirely and
# let cargo-mutants default to its own single-worker in-place behavior, not pass --jobs 1.
echo "mutants: running cargo-mutants over fleet/src/ratchet.rs (in-place, serial; can take an hour or more; no output until done)..." >&2
cargo mutants \
  --manifest-path keel/Cargo.toml \
  --package fleet \
  --file 'fleet/src/ratchet.rs' \
  --in-place \
  --output "$run_dir" \
  --colors never >"$run_dir/cargo-mutants.log" 2>&1
mutants_status=$?

case "$mutants_status" in
  0|2|3) ;;
  *)
    sed -n '1,120p' "$run_dir/cargo-mutants.log"
    echo "mutants: REFUSE: cargo-mutants measurement failed (exit=$mutants_status)"
    exit 6
    ;;
esac

out_dir="$run_dir/mutants.out"
if [ ! -d "$out_dir" ]; then
  echo "mutants: REFUSE: cargo-mutants produced no results"
  exit 6
fi

count_lines() {
  local result_file="$1"
  if [ -f "$result_file" ]; then
    wc -l < "$result_file" | tr -d '[:space:]'
  else
    echo 0
  fi
}

caught=$(count_lines "$out_dir/caught.txt")
missed=$(count_lines "$out_dir/missed.txt")
timed_out=$(count_lines "$out_dir/timeout.txt")
total=$((caught + missed + timed_out))

printf 'mutants: caught=%d total=%d floor=%s\n' "$caught" "$total" "$floor"
if [ "$total" -eq 0 ]; then
  echo "mutants: REFUSE: measuring nothing (total==0)"
  exit 6
fi

if [ $((caught * floor_total)) -lt $((floor_caught * total)) ]; then
  printf 'mutants: REFUSE: mutation score dropped: %d/%d < %s\n' \
    "$caught" "$total" "$floor"
  exit 6
fi

printf 'mutants: ok: %d/%d >= %s\n' "$caught" "$total" "$floor"
