#!/usr/bin/env bash
# M10 — install.sh must never SILENTLY destroy a binary or symlink it did not put there (B9).
# On the author's own machine ~/.local/bin/fleet was a symlink into a DIFFERENT, unrelated `fleet`
# repository. A plain `./install.sh` would have overwritten it, and `--uninstall` would have deleted
# it, with no warning at all -- destroying someone else's tool and workflow. `foreign_bin()` guards
# both paths; this mechanises the four directions it must get right, each in its own throwaway
# fixture dir so no real machine state (including the real $HOME) is ever touched.
#
# Owner decision (2026-09-09): `fleet` on PATH must ALWAYS be this CLI (a stale symlink to a
# predecessor repo was shadowing every build of this one). install.sh no longer REFUSES on a
# foreign binary at the install path -- it DISPLACES it to `<path>.displaced-by-fleet-rs` (created
# once, never clobbered by a later install) and installs over it. Fixture 3 below asserts that new
# contract: nothing is silently destroyed, it is preserved at a documented path instead. The
# `--uninstall` direction (fixture 4) is UNCHANGED by that decision -- it still refuses outright,
# because uninstalling is destructive with no displacement to fall back on.
#
# Fixture 3 nests BIN_DIR *inside* the fixture's own ROOT_DIR on purpose: that exact layout is what
# exposed a real bug during this detector's own construction (see docs/delta.d/B9.md) -- the guard's
# path-prefix comparison was written to apply only to symlinks, but ran unconditionally, so a plain
# foreign file sitting under a BIN_DIR nested inside ROOT_DIR was silently classified as "ours" and
# clobbered. Fixed in install.sh; this fixture pins the fix down.
set -u
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
command -v cargo >/dev/null 2>&1 || exit 77
command -v rustc >/dev/null 2>&1 || exit 77
[ "$(uname -s)" = "Darwin" ] || exit 77   # install.sh itself refuses off-Darwin

N=4
BAD=0
FAILS=()

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# A minimal, fast-building stand-in for the real workspace: it only needs to answer `--help`
# with the exact marker string install.sh's foreign_bin() looks for, and `completions zsh` with
# anything (install.sh pipes that straight to a file). Using this instead of the real ~300+ crate
# workspace keeps this detector on the order of a few seconds instead of a full release build.
#
# This MUST live directly at $WORK (not a subdirectory): install.sh computes its own ROOT_DIR as
# `dirname "$0"`, and $WORK is where the copy of install.sh below is placed, so `$WORK/Cargo.toml`
# is exactly the manifest path install.sh's `cargo build --manifest-path "$ROOT_DIR/Cargo.toml"`
# will look for. A fake crate nested one level down (e.g. `$WORK/keel/`) leaves that manifest path
# empty and every fixture that builds fails with "manifest path ... does not exist" for reasons that
# have nothing to do with the foreign-binary guard under test.
mkdir -p "$WORK/src"
cat > "$WORK/Cargo.toml" <<'EOF'
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
cat > "$WORK/src/main.rs" <<'EOF'
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

# --- Fixture 3: a foreign file at the install path is DISPLACED, never silently destroyed ------
# BIN_DIR is deliberately nested inside $WORK (this fixture's ROOT_DIR) -- see header comment.
# New contract (2026-09-09): `fleet` on PATH must always be this CLI, so install.sh no longer
# refuses -- it moves the foreign file to `<path>.displaced-by-fleet-rs` (once; never clobbered by
# a later install) and installs over it. This asserts every part of that: `--check` reports the
# displacement without performing it, a real install performs it and preserves the original bytes,
# and a second run does not clobber the existing backup with a newer foreign file.
F3OK=1
B3="$WORK/f3-bin"; S3="$WORK/f3-state"
mkdir -p "$B3"
printf '#!/bin/sh\necho "not the real fleet, v1"\n' > "$B3/fleet"
chmod +x "$B3/fleet"
SUM3_ORIG="$(shasum -a 256 "$B3/fleet" | awk '{print $1}')"

# 3a: --check must report the displacement and must NOT mutate anything.
OUT3CHECK="$(run_install "$B3" "$S3" --check)"; RC3CHECK=$?
SUM3_AFTER_CHECK="$(shasum -a 256 "$B3/fleet" | awk '{print $1}')"
BACKUP3="$B3/fleet.displaced-by-fleet-rs"
if [ "$RC3CHECK" -ne 0 ] || [ "$SUM3_ORIG" != "$SUM3_AFTER_CHECK" ] || [ -e "$BACKUP3" ] \
    || ! printf '%s' "$OUT3CHECK" | grep -q 'displaced-by-fleet-rs'; then
    echo "M10: fixture3a (--check on foreign file) expected exit 0, file untouched, no backup created, and a displacement report; got exit=$RC3CHECK before=$SUM3_ORIG after=$SUM3_AFTER_CHECK backup_exists=$([ -e "$BACKUP3" ] && echo yes || echo no)"
    F3OK=0
fi

# 3b: a real install must move the ORIGINAL foreign file to the backup path, byte-for-byte, and
# install our binary at the target path.
OUT3="$(run_install "$B3" "$S3")"; RC3=$?
SUM3_BACKUP="$([ -e "$BACKUP3" ] && shasum -a 256 "$BACKUP3" | awk '{print $1}' || echo MISSING)"
HELP3="$("$B3/fleet" --help 2>/dev/null || true)"
if [ "$RC3" -ne 0 ] || [ "$SUM3_BACKUP" != "$SUM3_ORIG" ] || [ ! -x "$B3/fleet" ] \
    || ! printf '%s' "$HELP3" | grep -q 'frozen, attested change'; then
    echo "M10: fixture3b (real install on foreign file) expected exit 0, original preserved at $BACKUP3 ($SUM3_ORIG), and our binary installed; got exit=$RC3 backup_sum=$SUM3_BACKUP installed=$([ -x "$B3/fleet" ] && echo yes || echo no)"
    F3OK=0
fi

# 3c: a foreign file reappearing at the target path (e.g. something else wrote over our binary)
# must be removed on the next install WITHOUT clobbering the backup already holding the ORIGINAL
# foreign file -- the backup is "once", not "most recent".
printf '#!/bin/sh\necho "not the real fleet, v2"\n' > "$B3/fleet"
chmod +x "$B3/fleet"
OUT3SECOND="$(run_install "$B3" "$S3")"; RC3SECOND=$?
SUM3_BACKUP_AFTER="$([ -e "$BACKUP3" ] && shasum -a 256 "$BACKUP3" | awk '{print $1}' || echo MISSING)"
HELP3SECOND="$("$B3/fleet" --help 2>/dev/null || true)"
if [ "$RC3SECOND" -ne 0 ] || [ "$SUM3_BACKUP_AFTER" != "$SUM3_ORIG" ] \
    || ! printf '%s' "$HELP3SECOND" | grep -q 'frozen, attested change'; then
    echo "M10: fixture3c (second install over a NEW foreign file) expected exit 0, backup still holding the original ($SUM3_ORIG), and our binary reinstalled; got exit=$RC3SECOND backup_sum=$SUM3_BACKUP_AFTER"
    F3OK=0
fi

if [ "$F3OK" -ne 1 ]; then
    FAILS+=("fixture3")
    BAD=$((BAD+1))
fi

# --- Fixture 4: --uninstall on a foreign symlink must refuse and leave it alive ----------------
# Unchanged by the 2026-09-09 contract change: uninstalling is destructive (rm, not a move-aside),
# so there is no safe displacement to fall back on -- refusing outright is still the only option.
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
