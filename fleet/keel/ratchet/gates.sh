#!/bin/zsh
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
keel_dir="$(cd "$script_dir/.." && pwd)"
binary="$keel_dir/target/debug/fleet"
fixture_dir="$script_dir/fixtures"
state_dir_for_test="$(mktemp -d)"
exception_path="$state_dir_for_test/exception.json"
trap 'rm -rf "$state_dir_for_test"' EXIT

if [[ ! -x "$binary" ]]; then
  print -u2 "environment fault: build $binary first"
  exit 3
fi

run_expect() {
  local expected="$1"
  shift
  local actual
  if "$@"; then
    actual=0
  else
    actual=$?
  fi
  if [[ "$actual" -ne "$expected" ]]; then
    print -u2 "gate mismatch: expected=$expected actual=$actual"
    exit 8
  fi
}

FLEET_STATE="$state_dir_for_test" "$binary" ratchet seed \
  --metrics "$fixture_dir/scorecard.good.json" --change baseline-c0
run_expect 7 env FLEET_STATE="$state_dir_for_test" "$binary" ratchet check \
  --metrics "$fixture_dir/scorecard.bad.json" --change regression-r1
FLEET_STATE="$state_dir_for_test" "$binary" ratchet check \
  --metrics "$fixture_dir/scorecard.improvement.json" --change improvement-i2
run_expect 6 env FLEET_STATE="$state_dir_for_test" "$binary" ratchet check \
  --metrics "$fixture_dir/scorecard.zero.bad.json" --change zero-z0

FLEET_STATE="$state_dir_for_test" "$binary" ratchet exception create \
  --change exception-e3 --metric mutation_kill_rate \
  --expires-at 2099-01-01T00:00:00Z --reason narrow-test \
  --signed-by verifier --output "$exception_path"
FLEET_STATE="$state_dir_for_test" "$binary" ratchet check \
  --metrics "$fixture_dir/scorecard.exception.good.json" \
  --change exception-e3 --exception "$exception_path"
run_expect 6 env FLEET_STATE="$state_dir_for_test" "$binary" ratchet check \
  --metrics "$fixture_dir/scorecard.bad.json" --change regression-r1 \
  --exception "$fixture_dir/exception.expired.bad.json"
run_expect 8 env FLEET_STATE="$state_dir_for_test" "$binary" ratchet check \
  --metrics "$fixture_dir/scorecard.bad.json" --change regression-r1 \
  --exception "$fixture_dir/exception.wide.bad.json"

FLEET_STATE="$state_dir_for_test" "$binary" ratchet show
FLEET_STATE="$state_dir_for_test" "$binary" ledger verify
print "ratchet gates passed: bad and good controls exercised"
