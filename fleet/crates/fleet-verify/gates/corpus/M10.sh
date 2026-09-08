#!/usr/bin/env bash
# M10 — install.sh must never clobber a binary or symlink it did not put there (B9).
# On the author's own machine ~/.local/bin/fleet was a symlink into a DIFFERENT, unrelated `fleet`
# repository. A plain `./install.sh` would have overwritten it, and `--uninstall` would have deleted
# it, with no warning at all -- destroying someone else's tool and workflow. `foreign_bin()` guards
# both paths now; this mechanises the four directions it must get right, each in its own throwaway
# fixture dir so no real machine state (including the real $HOME) is ever touched.
#
# Fixture 3 nests BIN_DIR *inside* the fixture's own ROOT_DIR on purpose: that exact layout is what
# exposed a real bug during this detector's own construction (see docs/delta.d/B9.md) -- the guard's
# path-prefix comparison was written to apply only to symlinks, but ran unconditionally, so a plain
# foreign file sitting under a BIN_DIR nested inside ROOT_DIR was silently classified as "ours" and
# clobbered. Fixed in install.sh; this fixture pins the fix down.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
command -v cargo >/dev/null 2>&1 || exit 77
command -v rustc >/dev/null 2>&1 || exit 77
[ "$(uname -s)" = "Darwin" ] || exit 77   # install.sh itself refuses off-Darwin

N=4
BAD=0
FAILS=()

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# A minimal, fast-building stand-in for the real `keel` crate: it only needs to answer `--help`
# with the exact marker string install.sh's foreign_bin() looks for, and `completions zsh` with
# anything (install.sh pipes that straight to a file). Using this instead of the real ~300+ crate
# workspace keeps this detector on the order of a few seconds instead of a full release build.
mkdir -p "$WORK/keel/src"
cat > "$WORK/keel/Cargo.toml" <<'EOF'
[package]
name = "fleet"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "fleet"
path = "src/main.rs"

[profile.release]
opt-level = 0
debug = false
EOF
cat > "$WORK/keel/src/main.rs" <<'EOF'
fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("--help") => println!("fleet — a local macOS harness that takes a task to a frozen, attested change."),
        Some("completions") => println!("# stub completion"),
        _ => println!("fleet stub"),
    }
}
EOF
cp "$ROOT/install.sh" "$WORK/install.sh"
chmod +x "$WORK/install.sh"

HOME_REAL="$HOME"
run_install() {
    # Never let a fixture touch the real $HOME (install.sh writes ~/.zfunc unconditionally, with
    # no override for it) -- point HOME at a private fake one, but keep cargo/rustup pointed at
    # the real toolchain so the build still works.
    HOME="$WORK/fakehome" \
    CARGO_HOME="${CARGO_HOME:-$HOME_REAL/.cargo}" \
    RUSTUP_HOME="${RUSTUP_HOME:-$HOME_REAL/.rustup}" \
    FLEET_BIN_DIR="$1" \
    FLEET_STATE="$2" \
    "$WORK/install.sh" ${3:-} </dev/null
}

# --- Fixture 1: free path installs -----------------------------------------------------------
B1="$WORK/f1-bin"; S1="$WORK/f1-state"; mkdir -p "$WORK/fakehome"
OUT1="$(run_install "$B1" "$S1")"; RC1=$?
if [ "$RC1" -ne 0 ] || [ ! -x "$B1/fleet" ]; then
    echo "M10: fixture1 (free path) expected exit 0 + installed binary, got exit=$RC1 exists=$([ -x "$B1/fleet" ] && echo yes || echo no)"
    FAILS+=("fixture1")
    BAD=$((BAD+1))
fi

# --- Fixture 2: our own binary already installed -> recognised, not refused -------------------
if [ "$RC1" -eq 0 ]; then
    OUT2="$(run_install "$B1" "$S1")"; RC2=$?
    if [ "$RC2" -ne 0 ] || printf '%s' "$OUT2" | grep -q 'install refused'; then
        echo "M10: fixture2 (own binary) expected exit 0 and no refusal, got exit=$RC2"
        FAILS+=("fixture2")
        BAD=$((BAD+1))
    fi
else
    echo "M10: fixture2 skipped a real assertion because fixture1 did not install (counted as failed, not silently excused)"
    FAILS+=("fixture2")
    BAD=$((BAD+1))
fi

# --- Fixture 3: a foreign script must refuse, and must be left byte-for-byte untouched --------
# BIN_DIR is deliberately nested inside $WORK (this fixture's ROOT_DIR) -- see header comment.
B3="$WORK/f3-bin"; S3="$WORK/f3-state"
mkdir -p "$B3"
printf '#!/bin/sh\necho "not the real fleet"\n' > "$B3/fleet"
chmod +x "$B3/fleet"
SUM3_BEFORE="$(shasum -a 256 "$B3/fleet" | awk '{print $1}')"
OUT3="$(run_install "$B3" "$S3")"; RC3=$?
SUM3_AFTER="$(shasum -a 256 "$B3/fleet" | awk '{print $1}')"
if [ "$RC3" -ne 7 ] || [ "$SUM3_BEFORE" != "$SUM3_AFTER" ]; then
    echo "M10: fixture3 (foreign script, BIN_DIR nested under ROOT_DIR) expected exit 7 + untouched file, got exit=$RC3 before=$SUM3_BEFORE after=$SUM3_AFTER"
    FAILS+=("fixture3")
    BAD=$((BAD+1))
fi

# --- Fixture 4: --uninstall on a foreign symlink must refuse and leave it alive ----------------
B4="$WORK/f4-bin"; S4="$WORK/f4-state"
mkdir -p "$B4"
FOREIGN_TARGET="/nonexistent/some-other-fleet-repo/fleet"
ln -sf "$FOREIGN_TARGET" "$B4/fleet"
LINK4_BEFORE="$(readlink "$B4/fleet")"
OUT4="$(run_install "$B4" "$S4" --uninstall)"; RC4=$?
LINK4_AFTER="$(readlink "$B4/fleet" 2>/dev/null || echo MISSING)"
if [ "$RC4" -ne 7 ] || [ "$LINK4_BEFORE" != "$LINK4_AFTER" ]; then
    echo "M10: fixture4 (--uninstall on foreign symlink) expected exit 7 + symlink alive, got exit=$RC4 before=$LINK4_BEFORE after=$LINK4_AFTER"
    FAILS+=("fixture4")
    BAD=$((BAD+1))
fi

echo "M10: $((N-BAD)) of $N foreign-binary guard fixtures behaved correctly (denominator: $N)"
[ "$BAD" -eq 0 ] || { printf 'M10: failed fixtures: %s\n' "${FAILS[*]}"; exit 1; }
exit 0
