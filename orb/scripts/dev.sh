#!/usr/bin/env bash
# Start the local backend chain and keep the app gate closed until all four backend ports are
# owned by the processes this launcher started. This script intentionally fails closed: a child
# exit, a live port collision, or an incomplete readiness chain is an error, not a warning.

set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${ORB_DEV_LOG_DIR:-/tmp/orb-dev-logs}"
LSOF_BIN="${LSOF_BIN:-lsof}"
PS_BIN="${PS_BIN:-ps}"
CURL_BIN="${CURL_BIN:-curl}"

if [ -f "$ROOT/.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . "$ROOT/.env"
  set +a
fi

GATEWAY_PORT="${GATEWAY_SIDECAR_PORT:-8082}"
VOICE_PORT="${VOICE_PROVIDER_SIDECAR_PORT:-8083}"
RELAY_PORT="${ORB_RELAY_PORT:-8765}"
RELAY_RS_ADDR="${ORB_RELAY_ADDR:-127.0.0.1:8091}"
RELAY_RS_PORT="${ORB_RELAY_RS_PORT:-${RELAY_RS_ADDR##*:}}"
METRO_PORT="${METRO_PORT:-8081}"
START_METRO="${ORB_START_METRO:-1}"

PIDS=()
PID_NAMES=()
PID_PORTS=()
PID_EXPECTED=()
CLEANED=0
STOP_REQUESTED=0

log() {
  printf '[dev] %s\n' "$*"
}

fail() {
  printf '[dev] ERROR: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

require_file() {
  [ -f "$1" ] || fail "required file is missing: $1"
}

pid_is_alive() {
  kill -0 "$1" 2>/dev/null || return 1
  local state=""
  state="$($PS_BIN -p "$1" -o state= 2>/dev/null | tr -d '[:space:]')"
  case "$state" in
    Z*) return 1 ;;
    *) return 0 ;;
  esac
}

pid_command() {
  "$PS_BIN" -p "$1" -o command= 2>/dev/null | sed 's/^[[:space:]]*//'
}

port_pids() {
  "$LSOF_BIN" -nP -t -iTCP:"$1" -sTCP:LISTEN 2>/dev/null \
    | awk '/^[0-9]+$/ { print }' \
    | sort -u
}

pid_parent() {
  "$PS_BIN" -p "$1" -o ppid= 2>/dev/null | tr -d '[:space:]'
}

pid_owns_port() {
  local pid="$1" port="$2"
  "$LSOF_BIN" -nP -a -p "$pid" -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null \
    | awk -v wanted="$pid" '$0 == wanted { found = 1 } END { exit found ? 0 : 1 }'
}

pid_is_descendant_of() {
  local child="$1" root_pid="$2" current="$1" parent=""
  local depth=0
  while [ "$depth" -lt 16 ] && [ -n "$current" ] && [ "$current" != "1" ]; do
    [ "$current" = "$root_pid" ] && return 0
    parent="$(pid_parent "$current")"
    [ "$parent" = "$current" ] && break
    current="$parent"
    depth=$((depth + 1))
  done
  return 1
}

# lsof is only a candidate source. A PID is accepted only when it is alive and the exact PID (or
# one of its live descendants, as cargo uses for `cargo run`) owns the listening socket.
pid_tree_owns_port() {
  local root_pid="$1" port="$2" owner=""
  pid_is_alive "$root_pid" || return 1
  while IFS= read -r owner; do
    [ -n "$owner" ] || continue
    pid_is_alive "$owner" || continue
    pid_owns_port "$owner" "$port" || continue
    pid_is_descendant_of "$owner" "$root_pid" && return 0
  done <<EOF
$(port_pids "$port")
EOF
  return 1
}

ensure_port_free() {
  local port="$1" owner="" command=""
  while IFS= read -r owner; do
    [ -n "$owner" ] || continue
    # Ignore dead entries from a raced/stale lsof snapshot. A live owner is a hard collision.
    if pid_is_alive "$owner" && pid_owns_port "$owner" "$port"; then
      command="$(pid_command "$owner")"
      fail "port $port is already owned by live pid $owner${command:+ ($command)}"
    fi
  done <<EOF
$(port_pids "$port")
EOF
}

process_matches_expected() {
  local command="$1" expected="$2"
  case "$command" in
    *"$expected"*) return 0 ;;
  esac
  # On macOS, `cargo run` can hand the tracked PID over to the compiled binary before the
  # readiness loop observes it. The process is still the relay started by this launcher, but its
  # command no longer contains `cargo`.
  if [ "$expected" = "cargo" ]; then
    case "$command" in
      *orb-relay-rs*) return 0 ;;
    esac
  fi
  return 1
}

verify_expected_process() {
  local pid="$1" expected="$2" command=""
  pid_is_alive "$pid" || return 1
  command="$(pid_command "$pid")"
  process_matches_expected "$command" "$expected"
}

start_process() {
  local name="$1" port="$2" cwd="$3" logfile="$4" expected="$5"
  shift 5
  local env_args=() pid=""
  while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do
    env_args+=("$1")
    shift
  done
  [ "$#" -gt 0 ] || fail "start_process($name) missing command separator"
  shift
  [ "$#" -gt 0 ] || fail "start_process($name) missing command"

  mkdir -p "$LOG_DIR"
  log "starting $name (log: $logfile)"
  (
    cd "$cwd"
    if [ "${#env_args[@]}" -gt 0 ]; then
      exec env "${env_args[@]}" "$@"
    else
      exec "$@"
    fi
  ) >"$logfile" 2>&1 &
  pid="$!"
  PIDS+=("$pid")
  PID_NAMES+=("$name")
  PID_PORTS+=("$port")
  PID_EXPECTED+=("$expected")
}

child_or_fail() {
  local index="$1" pid="${PIDS[$1]}" name="${PID_NAMES[$1]}" status=""
  if pid_is_alive "$pid"; then
    return 0
  fi
  if wait "$pid"; then
    status=0
  else
    status="$?"
  fi
  fail "$name (pid $pid) exited during startup with status $status"
}

wait_for_http() {
  local index="$1" url="$2" timeout_s="${3:-30}" name="${PID_NAMES[$1]}" port="${PID_PORTS[$1]}"
  local deadline=$((SECONDS + timeout_s)) code=""
  while [ "$SECONDS" -lt "$deadline" ]; do
    child_or_fail "$index"
    if verify_expected_process "${PIDS[$index]}" "${PID_EXPECTED[$index]}" \
      && pid_tree_owns_port "${PIDS[$index]}" "$port"; then
      code="$("$CURL_BIN" --silent --show-error --connect-timeout 1 --max-time 2 \
        --output /dev/null --write-out '%{http_code}' "$url" 2>/dev/null || true)"
      if [ "$code" = "200" ]; then
        log "$name ready: $url"
        return 0
      fi
    fi
    sleep 0.25
  done
  fail "$name did not become ready at $url within ${timeout_s}s"
}

wait_for_tcp() {
  local index="$1" timeout_s="${2:-30}" name="${PID_NAMES[$1]}" port="${PID_PORTS[$1]}"
  local deadline=$((SECONDS + timeout_s))
  while [ "$SECONDS" -lt "$deadline" ]; do
    child_or_fail "$index"
    if verify_expected_process "${PIDS[$index]}" "${PID_EXPECTED[$index]}" \
      && pid_tree_owns_port "${PIDS[$index]}" "$port"; then
      log "$name ready on :$port"
      return 0
    fi
    sleep 0.25
  done
  fail "$name did not own :$port within ${timeout_s}s"
}

wait_for_whole_chain() {
  local timeout_s="${1:-30}" deadline="" code="" index=""
  deadline=$((SECONDS + timeout_s))
  while [ "$SECONDS" -lt "$deadline" ]; do
    for index in "${!PIDS[@]}"; do
      child_or_fail "$index"
    done
    code="$("$CURL_BIN" --silent --show-error --connect-timeout 1 --max-time 2 \
      --output /dev/null --write-out '%{http_code}' \
      "http://127.0.0.1:$RELAY_PORT/readyz" 2>/dev/null || true)"
    if [ "$code" = "200" ]; then
      log "whole-chain readiness passed: 8082/8083/8765/$RELAY_RS_PORT"
      return 0
    fi
    sleep 0.25
  done
  fail "whole-chain readiness failed: 8082/8083/8765/$RELAY_RS_PORT"
}

stop_owned_processes() {
  local index="" pid="" owner=""
  for index in "${!PIDS[@]}"; do
    pid="${PIDS[$index]}"
    if pid_is_alive "$pid"; then
      kill "$pid" 2>/dev/null || true
    fi
    while IFS= read -r owner; do
      [ -n "$owner" ] || continue
      pid_is_alive "$owner" || continue
      if pid_is_descendant_of "$owner" "$pid"; then
        kill "$owner" 2>/dev/null || true
      fi
    done <<EOF
$(port_pids "${PID_PORTS[$index]}")
EOF
  done
}

cleanup() {
  [ "$CLEANED" -eq 0 ] || return 0
  CLEANED=1
  stop_owned_processes
  local pid=""
  for pid in "${PIDS[@]}"; do
    wait "$pid" 2>/dev/null || true
  done
}

on_signal() {
  STOP_REQUESTED=1
  exit 130
}

preflight() {
  require_command "$LSOF_BIN"
  require_command "$PS_BIN"
  require_command "$CURL_BIN"
  require_file "$ROOT/backend/gateway-sidecar/src/index.ts"
  require_file "$ROOT/backend/voice-provider-sidecar/src/index.ts"
  require_file "$ROOT/backend/relay-py/src/orb_relay/app.py"
  require_file "$ROOT/backend/relay-rs/Cargo.toml"
  [ -x "${ORB_VITE_NODE_BIN:-$ROOT/node_modules/.bin/vite-node}" ] \
    || fail "vite-node is missing; run npm install first"
  [ -x "${ORB_PYTHON_BIN:-$ROOT/backend/relay-py/.venv/bin/python}" ] \
    || fail "Python relay venv is missing; run: cd backend/relay-py && python3 -m venv .venv"
  require_command "${ORB_CARGO_BIN:-cargo}"
}

main() {
  trap cleanup EXIT
  trap on_signal INT TERM
  preflight

  local port="" index="" relay_index="" vite_node="${ORB_VITE_NODE_BIN:-$ROOT/node_modules/.bin/vite-node}"
  local python_bin="${ORB_PYTHON_BIN:-$ROOT/backend/relay-py/.venv/bin/python}"
  local cargo_bin="${ORB_CARGO_BIN:-cargo}"
  for port in "$GATEWAY_PORT" "$VOICE_PORT" "$RELAY_PORT" "$RELAY_RS_PORT"; do
    ensure_port_free "$port"
  done
  [ "$START_METRO" = "0" ] || ensure_port_free "$METRO_PORT"

  start_process "gateway-sidecar" "$GATEWAY_PORT" "$ROOT" "$LOG_DIR/gateway-sidecar.log" \
    "vite-node" "GATEWAY_SIDECAR_PORT=$GATEWAY_PORT" -- "$vite_node" \
    backend/gateway-sidecar/src/index.ts
  start_process "voice-provider-sidecar" "$VOICE_PORT" "$ROOT" "$LOG_DIR/voice-sidecar.log" \
    "vite-node" "VOICE_PROVIDER_SIDECAR_PORT=$VOICE_PORT" -- "$vite_node" \
    backend/voice-provider-sidecar/src/index.ts
  start_process "relay-py" "$RELAY_PORT" "$ROOT/backend/relay-py" "$LOG_DIR/relay-py.log" \
    "uvicorn" "ORB_GATEWAY_URL=http://127.0.0.1:$GATEWAY_PORT" \
    "ORB_VOICE_URL=http://127.0.0.1:$VOICE_PORT" "ORB_RELAY_ADDR=127.0.0.1:$RELAY_RS_PORT" -- \
    "$python_bin" -m uvicorn orb_relay.app:app --host 127.0.0.1 --port "$RELAY_PORT"
  # ORB_RELAY_PROVIDER was NEVER set here, and provider.rs defaults it to "fake" — so the official
  # one-command dev startup has always run the realtime relay on canned transcripts. Live symptom,
  # reported by the owner on 2026-08-28: the orb connects, the mic captures, the status line says
  # "Listening", and no reply ever comes; relay-rs logs `provider=fake-t0` and relay-py answers a
  # fake transcript ("the final transcript"). Nothing in any test suite covers which provider the dev
  # chain boots with, which is why 594 TS + 250 Python tests were green throughout.
  #
  # Default to the real path and point it at the voice sidecar we just started. Still overridable —
  # `ORB_RELAY_PROVIDER=fake scripts/dev.sh` keeps the offline/keyless chain for anyone who wants it.
  start_process "relay-rs" "$RELAY_RS_PORT" "$ROOT/backend/relay-rs" "$LOG_DIR/relay-rs.log" \
    "cargo" "ORB_RELAY_ADDR=127.0.0.1:$RELAY_RS_PORT" \
    "ORB_RELAY_PROVIDER=${ORB_RELAY_PROVIDER:-http}" \
    "ORB_RELAY_PROVIDER_URL=${ORB_RELAY_PROVIDER_URL:-http://127.0.0.1:$VOICE_PORT}" \
    -- "$cargo_bin" run --quiet

  wait_for_http 0 "http://127.0.0.1:$GATEWAY_PORT/healthz"
  wait_for_http 1 "http://127.0.0.1:$VOICE_PORT/healthz"
  wait_for_http 2 "http://127.0.0.1:$RELAY_PORT/healthz"
  wait_for_tcp 3
  wait_for_whole_chain

  if [ "$START_METRO" != "0" ]; then
    start_process "metro" "$METRO_PORT" "$ROOT" "$LOG_DIR/metro.log" \
      "react-native" -- "$ROOT/node_modules/.bin/react-native" start --port "$METRO_PORT"
    wait_for_tcp 4 30
  fi

  log "backend chain is ready; app startup is now allowed"
  while :; do
    for index in "${!PIDS[@]}"; do
      child_or_fail "$index"
    done
    sleep 1
  done
}

# Unit tests source this file to exercise lifecycle helpers without starting the stack.
if [ "${DEV_SH_LIBRARY_ONLY:-0}" != "1" ]; then
  main "$@"
fi
