#!/usr/bin/env bash
# Start Fleet's local development server.
# One process owns builds; the CLI runs directly so reedline keeps its real TTY.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT_DIR"
DEV_DIR="$ROOT_DIR/.dev-watch"
PID_FILE="$DEV_DIR/dev-server.pid"
LOG_FILE="$DEV_DIR/dev-server.log"
BUILDER_LOG_FILE="$LOG_FILE"
BUILDER_PID=""
if [[ -z "${CARGO_TARGET_DIR:-}" && "$(uname -m)" == "arm64" ]]; then
    export CARGO_TARGET_DIR="$ROOT_DIR/target/aarch64-apple-darwin"
fi
BINARY="${CARGO_TARGET_DIR:-$ROOT_DIR/target}/debug/fleet"
if [[ -n "${FLEET_CARGO_BIN:-}" ]]; then
    CARGO_BIN="$FLEET_CARGO_BIN"
elif [[ "$(uname -m)" == "arm64" ]]; then
    USER_HOME="$(/usr/bin/dscl . -read "/Users/$(id -un)" NFSHomeDirectory 2>/dev/null | awk '{print $2}')"
    if [[ -x "$USER_HOME/.cargo-arm64/bin/cargo" ]]; then
        CARGO_BIN="$USER_HOME/.cargo-arm64/bin/cargo"
        export PATH="$USER_HOME/.cargo-arm64/bin:$PATH"
    else
        CARGO_BIN="$(command -v cargo)"
    fi
else
    CARGO_BIN="$(command -v cargo)"
fi

if [[ "$(uname -m)" == "arm64" ]] && command -v rustup >/dev/null 2>&1 \
    && rustup toolchain list 2>/dev/null | grep -q '^stable-aarch64-apple-darwin'; then
    export RUSTUP_TOOLCHAIN="${FLEET_RUST_TOOLCHAIN:-stable-aarch64-apple-darwin}"
fi

mkdir -p "$DEV_DIR"

is_dev_server() {
    local pid="$1"
    [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null \
        && ps -p "$pid" -o command= 2>/dev/null | grep -qE '(^|/)(bacon|cargo-watch|cargo watch)( |$)'
}

newest_source_is_newer() {
    if [[ ! -x "$BINARY" ]]; then
        return 0
    fi
    [[ -n "$(find Cargo.toml Cargo.lock src crates -type f -newer "$BINARY" -print -quit 2>/dev/null)" ]]
}

start_builder() {
    if [[ -f "$PID_FILE" ]] && is_dev_server "$(<"$PID_FILE")"; then
        if [[ ! -e "$BUILDER_LOG_FILE" && -e "$DEV_DIR/watch.log" ]]; then
            BUILDER_LOG_FILE="$DEV_DIR/watch.log"
        fi
        return 0
    fi
    # Adopt the previous script's watcher so upgrading dev.sh never creates a second builder.
    if [[ -f "$DEV_DIR/watch.pid" ]] && is_dev_server "$(<"$DEV_DIR/watch.pid")"; then
        cp "$DEV_DIR/watch.pid" "$PID_FILE"
        BUILDER_LOG_FILE="$DEV_DIR/watch.log"
        return 0
    fi
    if command -v bacon >/dev/null 2>&1; then
        echo "[dev] using bacon as the single build owner" >&2
        nohup env FLEET_CARGO_BIN="$CARGO_BIN" bacon --headless --job fleet-build \
            >"$LOG_FILE" 2>&1 </dev/null &
        BUILDER_PID=$!
    elif command -v cargo-watch >/dev/null 2>&1; then
        echo "[dev] bacon not found; using cargo-watch as the single build owner" >&2
        nohup env FLEET_CARGO_BIN="$CARGO_BIN" cargo-watch -w Cargo.toml -w Cargo.lock \
            --no-vcs-ignores -w src -w crates --shell "bash scripts/fleet-build.sh" \
            >"$LOG_FILE" 2>&1 </dev/null &
        BUILDER_PID=$!
    else
        echo "[dev] install bacon (recommended) or cargo-watch" >&2
        exit 3
    fi
    echo "$BUILDER_PID" >"$PID_FILE"
}

wait_for_latest_build() {
    if ! newest_source_is_newer; then
        return 0
    fi
    echo "[dev] waiting for the first successful build..." >&2
    BUILDER_PID="$(<"$PID_FILE")"
    local deadline=$((SECONDS + 180))
    while newest_source_is_newer; do
        if ! is_dev_server "$BUILDER_PID"; then
            echo "[dev] build server exited before producing a current binary; see $BUILDER_LOG_FILE" >&2
            tail -40 "$BUILDER_LOG_FILE" >&2 || true
            exit 8
        fi
        if (( SECONDS >= deadline )); then
            echo "[dev] build did not produce a current binary; see $BUILDER_LOG_FILE" >&2
            tail -40 "$BUILDER_LOG_FILE" >&2 || true
            exit 8
        fi
        sleep 0.25
    done
}

start_builder
wait_for_latest_build
if [[ $# -eq 1 && "$1" == "watch" ]]; then
    echo "[dev] build watcher is running; log: $BUILDER_LOG_FILE" >&2
    while is_dev_server "$BUILDER_PID"; do
        sleep 1
    done
    echo "[dev] build watcher exited; see $BUILDER_LOG_FILE" >&2
    tail -40 "$BUILDER_LOG_FILE" >&2 || true
    exit 8
fi
echo "[dev] launching latest Fleet binary; build log: $BUILDER_LOG_FILE" >&2
exec "$BINARY" "$@"
