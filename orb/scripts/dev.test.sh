#!/usr/bin/env bash
# Small shell regression suite for scripts/dev.sh. It deliberately avoids opening real ports.

set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV_SH_LIBRARY_ONLY=1 . "$ROOT/scripts/dev.sh"

fail_test() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local haystack="$1" needle="$2"
  case "$haystack" in
    *"$needle"*) ;;
    *) fail_test "expected '$needle' in '$haystack'" ;;
  esac
}

process_matches_expected 'target/debug/orb-relay-rs' cargo \
  || fail_test 'compiled relay binary was not accepted as the cargo child'
if process_matches_expected 'target/debug/orb-relay-rs' vite-node; then
  fail_test 'compiled relay binary matched an unrelated process expectation'
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/orb-dev-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

# A dead lsof PID is ignored, while a live owner is reported as a collision.
fake_lsof="$tmp/lsof"
fake_ps="$tmp/ps"
cat >"$fake_lsof" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_LSOF_PID:-999999}"
EOF
cat >"$fake_ps" <<'EOF'
#!/usr/bin/env bash
if [ "$1" = "-p" ] && [ "$3" = "-o" ] && [ "$4" = "command=" ]; then
  printf '%s\n' 'unrelated-process --port 1234'
elif [ "$1" = "-p" ] && [ "$3" = "-o" ] && [ "$4" = "ppid=" ]; then
  printf '%s\n' '1'
fi
EOF
chmod +x "$fake_lsof" "$fake_ps"
LSOF_BIN="$fake_lsof"
PS_BIN="$fake_ps"

ensure_port_free 1234
FAKE_LSOF_PID="$$"; export FAKE_LSOF_PID
if (ensure_port_free 1234) 2>"$tmp/collision.log"; then
  fail_test "live port owner was not rejected"
fi
assert_contains "$(cat "$tmp/collision.log")" 'port 1234 is already owned by live pid'

# A child that exits before readiness is a hard failure, including exit status in the message.
(
  PIDS=()
  PID_NAMES=()
  PID_PORTS=()
  PID_EXPECTED=()
  start_process 'short-lived' 1234 "$tmp" "$tmp/child.log" 'sh' -- sh -c 'exit 7'
  sleep 0.1
  child_or_fail 0
) 2>"$tmp/child.log.err" && fail_test 'child exit was not fail-closed'
assert_contains "$(cat "$tmp/child.log.err")" 'short-lived (pid'
assert_contains "$(cat "$tmp/child.log.err")" 'status 7'

printf 'dev launcher shell tests passed\n'
