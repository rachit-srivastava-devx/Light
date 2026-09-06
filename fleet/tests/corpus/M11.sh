#!/usr/bin/env bash
# M11 — every lane in bin/lanes.conf must answer THROUGH bin/freelane.sh, not merely be reachable
# by a hand curl (B10 item 3). The DevToolBox lane sat in bin/lanes.conf for a day answering
# every real call with HTTP 400 "Missing \"prompt\" field" because freelane.sh sent it an
# OpenAI-shaped body regardless of what the lane actually wanted -- a raw `curl -X POST ... {
# "prompt": ... }` against that same URL would have looked perfectly healthy the whole time. No
# detector asserted the property that actually matters: freelane.sh's own dialect-aware request
# construction and response parsing must be what gets a real reply out of each configured lane.
#
# This detector does not re-implement that check by hand-curling each lane (that would recreate
# exactly the blind spot that let the DevToolBox lane rot) -- it drives the real bin/freelane.sh
# against each real lane record from bin/lanes.conf, in isolation, and inspects freelane's own
# self-reported trace (the `tried=...:answered` / `resolved_model=` line it already emits) to
# confirm THAT lane, and not some other one, is what produced the reply.
set -u
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
FREELANE="$ROOT/bin/freelane.sh"
LANES_CONF="$ROOT/bin/lanes.conf"

[ -x "$FREELANE" ] || { echo "M11: bin/freelane.sh missing or not executable"; exit 77; }
[ -r "$LANES_CONF" ] || { echo "M11: bin/lanes.conf missing"; exit 77; }
command -v curl >/dev/null 2>&1 || exit 77
command -v python3 >/dev/null 2>&1 || exit 77

# A sandbox with no outbound network at all cannot exercise any real lane -- that is an
# environment fact, not a lane defect, and must be EXCLUDED (exit 77), never silently passed and
# never counted as a catch. Probed against the built-in default lane's host, not a lanes.conf
# entry, so this check itself never depends on the file under test.
if ! curl -sS -m 8 -o /dev/null "https://api.llm7.io/v1/chat/completions" 2>/dev/null; then
  echo "M11: no outbound network reachability in this environment -- excluded, not a lane defect"
  exit 77
fi

# Parse bin/lanes.conf the same way bin/freelane.sh's add_lane() does: strip a trailing comment,
# skip blank lines, require at least one '|'. Deliberately duplicated rather than sourced --
# freelane.sh is a script, not a library, and the point of this detector is to feed it real,
# independently-derived lane records and watch what IT does with them.
LANES=()
while IFS= read -r raw_line || [ -n "$raw_line" ]; do
  line=${raw_line%%#*}
  trimmed=${line//[[:space:]]/}
  [ -n "$trimmed" ] || continue
  case "$line" in
    *'|'*) LANES+=("$line") ;;
    *) ;;
  esac
done < "$LANES_CONF"

total=${#LANES[@]}
if [ "$total" -eq 0 ]; then
  echo "M11: zero lane records found in bin/lanes.conf -- measuring nothing is a failure"
  exit 1
fi

lane_host() {
  python3 - "$1" <<'PY'
from urllib.parse import urlparse
import sys
u = urlparse(sys.argv[1])
print(u.hostname or sys.argv[1])
PY
}

checked=0
caught=0
declare -a CAUGHT_CASES=()

for lane_record in "${LANES[@]}"; do
  lane_url=${lane_record%%|*}
  lane_url=$(printf '%s' "$lane_url" | awk '{$1=$1;print}')
  host=$(lane_host "$lane_url")
  checked=$((checked + 1))

  # Force the built-in default lane to fail instantly (connection-refused on localhost, not a
  # network round trip) so the ONLY lane freelane.sh can possibly succeed through is the one
  # record under test, fed back in verbatim via FREELANE_LANES. FREELANE_CONFIG is pointed at
  # /dev/null so the real bin/lanes.conf is not also loaded a second time behind it.
  out=$(
    FREELANE_URL="http://127.0.0.1:1/m11-force-fail" \
    FREELANE_MODEL="m11-unreachable-default" \
    FREELANE_CONFIG=/dev/null \
    FREELANE_LANES="$lane_record" \
    FREELANE_BUDGET_S=25 \
    bash "$FREELANE" "Reply with exactly the single word: PONG" 2>&1
  )
  rc=$?

  reason=''
  if [ "$rc" -ne 0 ]; then
    reason="freelane exited $rc (expected 0): $(printf '%s' "$out" | tail -1)"
  elif ! printf '%s' "$out" | grep -q "${host}:answered"; then
    reason="freelane's own trace does not show ${host}:answered: $(printf '%s' "$out" | tail -1)"
  elif printf '%s' "$out" | grep -q 'resolved_model=UNRESOLVED'; then
    reason="lane answered but reported no resolved model (UNRESOLVED) -- not a countable lane"
  elif ! printf '%s' "$out" | grep -qE 'resolved_model=[^ ]+'; then
    reason="no resolved_model= reported at all"
  fi

  if [ -n "$reason" ]; then
    echo "M11: CAUGHT lane $lane_record ($host): $reason"
    caught=$((caught + 1))
    CAUGHT_CASES+=("$lane_record")
  else
    echo "M11: ok, $host answered through bin/freelane.sh: $(printf '%s' "$out" | grep -o 'resolved_model=[^]]*' | head -1)"
  fi
done

echo "M11: $((checked - caught)) of $checked bin/lanes.conf lane(s) answered through bin/freelane.sh (denominator: $checked)"
if [ "$caught" -gt 0 ]; then
  echo "M11: ${#CAUGHT_CASES[@]} lane(s) did not answer through freelane.sh:"
  for c in "${CAUGHT_CASES[@]}"; do echo "    - $c"; done
  exit 1
fi
exit 0
