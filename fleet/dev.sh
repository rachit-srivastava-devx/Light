#!/usr/bin/env bash
# Dev entrypoint for the fleet CLI -- the `npm run dev` equivalent for this repo.
#
# Usage:
#   ./dev.sh                 # build, then launch fleet's interactive CLI (Claude-Code-style REPL)
#   ./dev.sh watch            # watch + rebuild loop only, never runs the binary
#   ./dev.sh <fleet-args...>  # watch + rebuild + rerun `fleet <fleet-args>` after every change
#
# The bare (no-args) case execs straight into `cargo run --bin fleet --` rather than going
# through cargo-watch: `fleet` only opens its interactive REPL when BOTH stdin and stdout are
# a real terminal (src/main.rs's `is_terminal()` gate -- reedline's raw-mode input needs an
# actual tty, not a pipe). cargo-watch's `--shell`/`--exec` runs the child through an
# intermediate subshell, which does not reliably preserve tty-ness end to end; routing the
# interactive launch through it silently falls back to fleet's --help branch instead of
# opening the REPL. A plain `exec` here replaces this script's own process image, so the
# terminal's fds pass straight through to `cargo run` and then to `fleet`.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT_DIR"

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
    exec cargo watch --clear "${WATCH_ARGS[@]}" --exec "build --bin fleet"
elif [[ $# -gt 0 ]]; then
    CMD="cargo run --bin fleet --"
    for arg in "$@"; do
        CMD+=" $(printf '%q' "$arg")"
    done
    # --use-shell bash: printf %q above emits bash-style escaping, so the command must be
    # re-parsed by bash, not whatever /bin/sh cargo-watch would otherwise default to.
    exec cargo watch --clear "${WATCH_ARGS[@]}" --use-shell bash --shell "$CMD"
else
    echo "[dev] launching fleet's interactive CLI (building if needed) -- use './dev.sh watch' for a rebuild-only loop" >&2
    exec cargo run --bin fleet --
fi
