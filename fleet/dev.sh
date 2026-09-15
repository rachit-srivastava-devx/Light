#!/usr/bin/env bash
# Dev entrypoint for the fleet CLI -- the `npm run dev` equivalent for this repo.
#
# Usage:
#   ./dev.sh                 # self-warms a background build watcher, then launches fleet's
#                             # interactive CLI (Claude-Code-style REPL)
#   ./dev.sh watch            # watch + rebuild loop only, never runs the binary
#   ./dev.sh <fleet-args...>  # watch + rebuild + rerun `fleet <fleet-args>` after every change
#
# The bare (no-args) case still execs straight into `cargo run --bin fleet --` rather than
# running the REPL itself through cargo-watch: `fleet` only opens its interactive REPL when BOTH
# stdin and stdout are a real terminal (src/main.rs's `is_terminal()` gate -- reedline's raw-mode
# input needs an actual tty, not a pipe). cargo-watch's `--shell`/`--exec` runs the child through
# an intermediate subshell, which does not reliably preserve tty-ness end to end; routing the
# interactive launch itself through it silently falls back to fleet's --help branch instead of
# opening the REPL. A plain `exec` here replaces this script's own process image, so the
# terminal's fds pass straight through to `cargo run` and then to `fleet`.
#
# What IS backgrounded: before that exec, bare mode ensures a detached `cargo watch --exec
# "build --bin fleet"` is running (`ensure_background_watcher` below), so edits made between REPL
# sessions are already compiled by the time you next run `./dev.sh` -- "available immediately"
# without needing a second terminal tab. The watcher never touches this terminal's tty (stdin
# from /dev/null, stdout/stderr to a log file) and outlives this script's `exec` since it is
# `disown`ed and immune to SIGHUP -- it is the SAME child-surviving-a-later-exec property real
# init systems rely on, not something specific to this script.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT_DIR"

WATCH_DIR="$ROOT_DIR/.dev-watch"
WATCH_PID_FILE="$WATCH_DIR/watch.pid"
WATCH_LOG_FILE="$WATCH_DIR/watch.log"

# cargo-watch's screen reset requires a terminfo database. CI, minimal containers, and some
# embedded terminals intentionally do not provide one; that must not prevent Fleet from running.
WATCH_CLEAR_ARG=""
if [[ -t 1 ]] && command -v clear >/dev/null 2>&1 && clear >/dev/null 2>&1; then
    WATCH_CLEAR_ARG="--clear"
fi

# Best-effort: a background warm cache is a convenience, never a precondition for the REPL to
# launch. Any failure in here prints a warning and falls through to the plain `cargo run` below
# instead of aborting the whole script (empty/missing MEMBERS is handled separately, below).
ensure_background_watcher() {
    if ! mkdir -p "$WATCH_DIR" 2>/dev/null; then
        echo "[dev] could not create $WATCH_DIR, skipping background watcher" >&2
        return 0
    fi
    # Existing, live, and actually a cargo-watch process (not some unrelated PID the OS recycled
    # onto a stale pid file) -- only then is it safe to skip spawning a duplicate.
    if [[ -f "$WATCH_PID_FILE" ]]; then
        local existing_pid
        existing_pid="$(cat "$WATCH_PID_FILE" 2>/dev/null || true)"
        if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null \
            && ps -p "$existing_pid" -o command= 2>/dev/null | grep -q "cargo-watch\|cargo watch"; then
            echo "[dev] background build watcher already warm (pid $existing_pid, log: $WATCH_LOG_FILE)" >&2
            return 0
        fi
    fi
    # mkdir is atomic even on macOS's bash 3.2 (no flock by default) -- guards two concurrent
    # bare `./dev.sh` launches from both spawning a watcher. Loser just skips spawning; the
    # winner's watcher covers both.
    if ! mkdir "$WATCH_DIR/spawn.lock" 2>/dev/null; then
        echo "[dev] another ./dev.sh is already starting the background watcher, skipping" >&2
        return 0
    fi
    trap 'rmdir "$WATCH_DIR/spawn.lock" 2>/dev/null || true' RETURN
    nohup cargo watch ${WATCH_CLEAR_ARG:+--clear} "${WATCH_ARGS[@]}" --exec "build --bin fleet" \
        >"$WATCH_LOG_FILE" 2>&1 </dev/null &
    local new_pid=$!
    disown "$new_pid" 2>/dev/null || true
    echo "$new_pid" >"$WATCH_PID_FILE"
    echo "[dev] started background build watcher (pid $new_pid, log: $WATCH_LOG_FILE)" >&2
}

if ! command -v cargo-watch >/dev/null 2>&1; then
    echo "[dev] cargo-watch not found, installing (cargo install cargo-watch)..." >&2
    cargo install cargo-watch
fi

# Watch exactly the real workspace members (from Cargo.toml's `members = [...]`), not the
# whole `crates/` tree: `crates/fleet-crew` is explicitly `exclude`d there (a Python tool with
# its own 7k-file/242MB .venv) and can never affect `cargo build --bin fleet`.
# Not `mapfile`: macOS ships bash 3.2 (mapfile/readarray need bash 4+).
MEMBERS=()
while IFS= read -r m; do
    MEMBERS+=("$m")
done < <(sed -n '/^members = \[/,/^\]/p' Cargo.toml | grep -oE '"[^"]+"' | tr -d '"')
if [[ ${#MEMBERS[@]} -eq 0 ]]; then
    echo "[dev] refusing to start: parsed 0 workspace members out of Cargo.toml (format changed?)" >&2
    exit 1
fi
# --no-vcs-ignores: without it, cargo-watch's initial .gitignore-discovery pass walks every
# directory under the detected git root regardless of the -w scope above -- including this
# repo's 43GB target/ (opens target/debug/deps directly) and fleet-crew's .venv -- adding a
# minute-plus of pure directory-walk before the first run. The -w list above is already the
# exact, hand-picked watch scope, so no .gitignore-based filtering is needed for correctness.
WATCH_ARGS=(-w Cargo.toml -w Cargo.lock --no-vcs-ignores)
for m in "${MEMBERS[@]}"; do
    WATCH_ARGS+=(-w "$m")
done

if [[ $# -eq 1 && "$1" == "watch" ]]; then
    exec cargo watch ${WATCH_CLEAR_ARG:+--clear} "${WATCH_ARGS[@]}" --exec "build --bin fleet"
elif [[ $# -gt 0 ]]; then
    CMD="cargo run --bin fleet --"
    for arg in "$@"; do
        CMD+=" $(printf '%q' "$arg")"
    done
    # --use-shell bash: printf %q above emits bash-style escaping, so the command must be
    # re-parsed by bash, not whatever /bin/sh cargo-watch would otherwise default to.
    exec cargo watch ${WATCH_CLEAR_ARG:+--clear} "${WATCH_ARGS[@]}" --use-shell bash --shell "$CMD"
else
    ensure_background_watcher
    echo "[dev] launching fleet's interactive CLI (building if needed) -- use './dev.sh watch' for a rebuild-only loop" >&2
    exec cargo run --bin fleet --
fi
