#!/usr/bin/env bash
set -eu

repo_root=$(cd "$(dirname "$0")/../.." && pwd)
fixture_dir=$(mktemp -d)
cleanup() {
  kill "${server_pid:-}" 2>/dev/null || true
  wait "${server_pid:-}" 2>/dev/null || true
  rm -rf "$fixture_dir"
}
trap cleanup EXIT

python3 "$repo_root/tests/lane/fixture_server.py" > "$fixture_dir/port" &
server_pid=$!
for _ in 1 2 3 4 5; do
  [ -s "$fixture_dir/port" ] && break
  sleep 1
done
[ -s "$fixture_dir/port" ] || { echo "lane test: fixture did not start" >&2; exit 3; }
port=$(<"$fixture_dir/port")

if FREELANE_URL="http://127.0.0.1:$port/busy" FREELANE_MODEL=first-requested \
  FREELANE_LANES="http://127.0.0.1:$port/ok|second-requested" \
  FREELANE_CONFIG=/dev/null FREELANE_BUDGET_S=5 \
  "$repo_root/bin/freelane.sh" "fixture prompt" >"$fixture_dir/out" 2>"$fixture_dir/err"; then
  :
else
  echo "lane test: failover invocation failed" >&2
  cat "$fixture_dir/err" >&2
  exit 8
fi
grep -q '^SECOND_LANE_OK$' "$fixture_dir/out"
grep -q 'resolved_model=served-by-second-lane requested=second-requested lane=2/2' "$fixture_dir/err"
grep -q 'tried=127.0.0.1:busy,127.0.0.1:answered' "$fixture_dir/err"
if grep -q 'resolved_model=first-requested' "$fixture_dir/err"; then
  echo "lane test: requested model was forged as resolved model" >&2
  exit 8
fi

down_rc=0
FREELANE_URL="http://127.0.0.1:$port/busy" \
  FREELANE_LANES="http://127.0.0.1:$port/down|second-requested" \
  FREELANE_CONFIG=/dev/null FREELANE_BUDGET_S=5 \
  "$repo_root/bin/freelane.sh" "fixture prompt" >"$fixture_dir/down.out" 2>"$fixture_dir/down.err"
down_rc=$?
[ "$down_rc" -eq 3 ] || { echo "lane test: all-down exit was $down_rc, expected 3" >&2; exit 8; }
# B10: this grep's result used to be DISCARDED, so the assertion that the failure names every
# lane tried could never fail. An assertion whose status is thrown away is not an assertion.
grep -q '127.0.0.1:busy,127.0.0.1:http-503' "$fixture_dir/down.err" || {
  echo "lane test: all-down stderr did not name both failed lanes:" >&2
  sed -n '1,3p' "$fixture_dir/down.err" >&2
  exit 8
}

before_retry=$(date +%s)
FREELANE_URL="http://127.0.0.1:$port/rate" \
  FREELANE_LANES="http://127.0.0.1:$port/ok|second-requested" \
  FREELANE_CONFIG=/dev/null FREELANE_BUDGET_S=5 \
  "$repo_root/bin/freelane.sh" "fixture prompt" >"$fixture_dir/retry.out" 2>"$fixture_dir/retry.err"
after_retry=$(date +%s)
[ "$((after_retry - before_retry))" -ge 1 ] || { echo "lane test: Retry-After was not honoured" >&2; exit 8; }
grep -q '127.0.0.1:rate-limit,127.0.0.1:answered' "$fixture_dir/retry.err"

FREELANE_URL="http://127.0.0.1:$port/timeout" \
  FREELANE_LANES="http://127.0.0.1:$port/ok|second-requested" \
  FREELANE_CONFIG=/dev/null FREELANE_BUDGET_S=3 \
  "$repo_root/bin/freelane.sh" "fixture prompt" >"$fixture_dir/timeout.out" 2>"$fixture_dir/timeout.err"
grep -q '127.0.0.1:timeout,127.0.0.1:answered' "$fixture_dir/timeout.err"
echo "lane failover: PASS (busy/timeout failover, provenance, all-down exit 3, Retry-After)"
