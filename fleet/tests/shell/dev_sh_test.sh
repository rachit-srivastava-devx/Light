#!/usr/bin/env bash
# Hermetic branching tests for ../../dev.sh. Stubs `cargo` (including its real dispatch-to-
# `cargo-<subcommand>` behavior for unrecognized subcommands like `watch`) so every scenario
# runs in milliseconds with no real compile, no network, and no long-running watch process --
# this proves dev.sh picks the right command for each input, not that cargo itself works
# (that's cargo's own test suite / the real e2e test in
# src/tests/interactive_pty_e2e/main.rs, which drives the actual fleet binary over a real tty).
#
# Usage: ./tests/shell/dev_sh_test.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEV_SH="$REPO_ROOT/dev.sh"

FAILURES=0
pass() { echo "  ok - $1"; }
fail() { echo "  FAIL - $1" >&2; FAILURES=$((FAILURES + 1)); }

# Args are joined with \x1f (unit separator) so an arg containing a space or quote can never be
# mistaken for a field boundary when we compare recorded invocations byte-for-byte.
join_args() {
    local out="" sep=""
    for a in "$@"; do
        out+="$sep$a"
        sep=$'\x1f'
    done
    printf '%s' "$out"
}

# mktemp -d, never a digest-derived path (S5): two runs of this suite must never collide.
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# --- fixture: a scratch "repo" with dev.sh's own Cargo.toml member list, but nothing else ---
FIXTURE="$WORK/fixture"
mkdir -p "$FIXTURE"
cp "$DEV_SH" "$FIXTURE/dev.sh"
chmod +x "$FIXTURE/dev.sh"
cat >"$FIXTURE/Cargo.toml" <<'EOF'
[workspace]
members = [
    "crates/cli",
    "crates/types",
    "src",
]
EOF

# --- fixture: same, but with a Cargo.toml that has no parseable members block ---
FIXTURE_EMPTY="$WORK/fixture_empty"
mkdir -p "$FIXTURE_EMPTY"
cp "$DEV_SH" "$FIXTURE_EMPTY/dev.sh"
chmod +x "$FIXTURE_EMPTY/dev.sh"
printf '[workspace]\n' >"$FIXTURE_EMPTY/Cargo.toml"

# --- stub bin dir: a fake `cargo` that records its argv, replicating real cargo's own
# subcommand-plugin dispatch (`cargo watch ...` execs a `cargo-watch` binary found on PATH,
# passing the remaining args) so the recorded invocation lands in the file that matches what
# actually ran, exactly as it would with the real cargo + cargo-watch installed. ---
STUB_BIN="$WORK/stub-bin"
mkdir -p "$STUB_BIN"

RECORD_CARGO="$WORK/cargo.invocation"
RECORD_WATCH="$WORK/cargo-watch.invocation"

cat >"$STUB_BIN/cargo" <<EOF
#!/usr/bin/env bash
if [[ "\${1:-}" == "watch" ]]; then
    shift
    args=""
    sep=""
    for a in "\$@"; do args+="\$sep\$a"; sep=\$'\x1f'; done
    printf '%s' "\$args" > "$RECORD_WATCH"
else
    args=""
    sep=""
    for a in "\$@"; do args+="\$sep\$a"; sep=\$'\x1f'; done
    printf '%s' "\$args" > "$RECORD_CARGO"
fi
exit 0
EOF
chmod +x "$STUB_BIN/cargo"

# A standalone `cargo-watch` binary too, in case dev.sh (or a future version of it) ever
# invokes the plugin binary directly instead of through `cargo watch`.
cp "$STUB_BIN/cargo" "$STUB_BIN/cargo-watch"

run_dev_sh() {
    # A fresh PATH per call (stub bin first) so `command -v cargo-watch` in dev.sh finds the
    # stub, never the real tool -- and stdin is /dev/null throughout (S4: dev.sh's own `while
    # read` loop over process substitution never touches our stdin, but the interactive branch
    # execs into a program that would otherwise block forever waiting on it).
    ( cd "$1" && PATH="$STUB_BIN:$PATH" ./dev.sh "${@:2}" </dev/null )
}

read_record() {
    [[ -f "$1" ]] && cat "$1" || echo '<none>'
}

# 1. Bare invocation must launch fleet directly via `cargo run`, and must never go through
#    cargo-watch at all (that was the actual bug: it used to run `cargo watch --exec build`,
#    which builds but never runs the binary, so the interactive CLI never opened).
rm -f "$RECORD_CARGO" "$RECORD_WATCH"
if run_dev_sh "$FIXTURE"; then
    expected="$(join_args run --bin fleet --)"
    if [[ ! -f "$RECORD_WATCH" ]] && [[ "$(read_record "$RECORD_CARGO")" == "$expected" ]]; then
        pass "bare invocation execs 'cargo run --bin fleet --' directly, bypassing cargo-watch"
    else
        fail "bare invocation: cargo=$(read_record "$RECORD_CARGO") watch=$(read_record "$RECORD_WATCH")"
    fi
else
    fail "bare invocation: dev.sh exited non-zero"
fi

# 2. `./dev.sh watch` must go through the rebuild-only cargo-watch loop, never launching fleet.
rm -f "$RECORD_CARGO" "$RECORD_WATCH"
if run_dev_sh "$FIXTURE" watch; then
    watch_args="$(read_record "$RECORD_WATCH")"
    if [[ ! -f "$RECORD_CARGO" ]] && [[ "$watch_args" == *$'\x1f--exec\x1f'* ]] && [[ "$watch_args" == *'build --bin fleet'* ]]; then
        pass "'watch' goes through cargo-watch --exec 'build --bin fleet', never running fleet directly"
    else
        fail "'watch': cargo=$(read_record "$RECORD_CARGO") watch=$watch_args"
    fi
else
    fail "'watch': dev.sh exited non-zero"
fi

# 3. Fleet args are forwarded through cargo-watch's --shell, correctly quoted (space, then a
#    lone double quote) so a real shell re-parse reproduces the original argv exactly.
rm -f "$RECORD_CARGO" "$RECORD_WATCH"
if run_dev_sh "$FIXTURE" gate --id 'with space' --note 'a"b'; then
    watch_args="$(read_record "$RECORD_WATCH")"
    if [[ ! -f "$RECORD_CARGO" ]] \
        && [[ "$watch_args" == *$'\x1f--shell\x1f'* ]] \
        && [[ "$watch_args" == *"cargo run --bin fleet -- gate --id with\\ space --note a\\\"b"* ]]; then
        pass "fleet-args pass through cargo-watch --shell with the original argv intact, correctly quoted"
    else
        fail "fleet-args: cargo=$(read_record "$RECORD_CARGO") watch=$watch_args"
    fi
else
    fail "fleet-args: dev.sh exited non-zero"
fi

# 4. A Cargo.toml with zero parseable workspace members must refuse to start (exit 1) rather
#    than silently launching cargo-watch with an empty -w list.
rm -f "$RECORD_CARGO" "$RECORD_WATCH"
set +e
run_dev_sh "$FIXTURE_EMPTY" >"$WORK/empty.stdout" 2>"$WORK/empty.stderr"
status=$?
set -e
if [[ "$status" -eq 1 ]] && [[ ! -f "$RECORD_CARGO" ]] && [[ ! -f "$RECORD_WATCH" ]] && grep -q "refusing to start" "$WORK/empty.stderr"; then
    pass "zero parsed workspace members refuses to start (exit 1), no cargo/cargo-watch invoked"
else
    fail "empty-members case: status=$status stderr=$(cat "$WORK/empty.stderr")"
fi

echo
if [[ "$FAILURES" -eq 0 ]]; then
    echo "dev_sh_test.sh: all checks passed"
    exit 0
else
    echo "dev_sh_test.sh: $FAILURES check(s) failed" >&2
    exit 1
fi
