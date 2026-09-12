#!/usr/bin/env bash
# M3 — the shipped binary must not carry a path from a deleted worktree.
# A shared CARGO_TARGET_DIR across git worktrees caches the FIRST builder's artifact, and
# CARGO_MANIFEST_DIR is baked in at compile time. When that worktree is removed the binary keeps
# pointing at a path that no longer exists, and every registry read fails at RUNTIME with exit 3
# while the build stays green (D34). Nothing else in this tree asserts the property.
#
# NOTE: a bare `.worktrees/` substring is not itself evidence of D34 -- since S3, fleet's own
# worktree.rs legitimately embeds the literal `.worktrees/{name}` format string (a relative git
# argument, DOT-prefixed, not a baked absolute path) and would always false-positive here. D34's
# actual signature is an ABSOLUTE compile-time CARGO_MANIFEST_DIR value that resolved to a worktree
# checkout: it contains `/worktrees/` (SLASH-prefixed) with `keel/fleet` later in that same string
# -- match that specific shape, not the bare relative literal.
#
# NOTE (B22, found by mutation-testing M13.sh against this exact check): the regex used to be
# anchored `^...$` to the whole `strings` line. That is silently wrong for a Rust binary --
# adjacent `&str` literals get laid back-to-back in rodata with NO null terminator between them
# (Rust strings are not C strings), so `strings` often reports one "line" that glues several
# unrelated constants together with no separator right after `/keel/fleet` (e.g.
# `.../keel/fleet::f::{{closure}}fleet::console::tests...`). The trailing `$` then never matches
# and this detector silently passed a genuinely stale binary -- reproduced directly by building a
# real stale release binary via `git worktree add`/`remove` and running this exact check against
# it (docs/delta.d/B22.md has the transcript). Dropping the anchors and matching the
# `/worktrees/...keel/fleet` shape as a substring is enough on its own to exclude the dot-prefixed
# relative literal above, without needing line boundaries at all.
set -u
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
BIN="${FLEET_BIN:-${CARGO_TARGET_DIR:-$ROOT/keel/target}/debug/fleet}"
[ -x "$BIN" ] || exit 77   # not built here: not mechanisable in this context
HITS=$(strings "$BIN" 2>/dev/null | grep -cE '/worktrees/.*/keel/fleet' || true)
if [ "${HITS:-0}" -gt 0 ]; then
  echo "M3: the binary embeds $HITS absolute CARGO_MANIFEST_DIR path(s) under a worktrees/ dir -- built in a worktree that may be deleted."
  echo "    Fix: cargo clean -p fleet, then rebuild from the main tree."
  exit 1
fi
exit 0
