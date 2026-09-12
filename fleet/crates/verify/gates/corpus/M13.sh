#!/usr/bin/env bash
# M13 — the cached TEST binary must not carry a path from a deleted worktree (B22).
# M3 mechanised D34 for the shipped release binary ($FLEET_BIN / debug/fleet), but `cargo test`
# compiles a SEPARATE artifact -- its own hashed executable under debug/deps/fleet-<hash> -- with
# CARGO_MANIFEST_DIR baked in at ITS compile time. Landing the wave-3 (B8/B9/B12/B14) merge hit
# this directly: after the four build worktrees those items were developed in were removed (the
# same cleanup pattern as S3), the next `verify.sh` run's `unit tests` and `swarm` acceptance
# stages both failed with `cannot load agent registry (exit 3)` -- 8 unit-test panics plus a real
# acceptance failure, every one tracing to `agent.rs::load_default()` / `skills.rs::load_default()`
# failing to find `agents.toml`/`skills.toml` via `env!("CARGO_MANIFEST_DIR")`. The shared external
# CARGO_TARGET_DIR had cached a test binary compiled while `cargo test` ran inside one of those
# worktrees, baking an absolute `.claude/worktrees/<name>/keel/fleet` path in -- D34's exact
# mechanism, just on the artifact M3 never looks at. Once the worktree was deleted, the cached
# test binary's baked path pointed nowhere, and every registry load failed at runtime while the
# BUILD stayed green (nothing recompiles debug/deps/fleet-<hash> unless its inputs changed, so a
# stale one keeps getting reused across `cargo test` invocations from a fresh checkout). Fixed that
# occurrence with `cargo clean -p fleet` + rebuild (the same remedy M3 already prints); this
# detector mechanises the catch so the next occurrence is a named exit 1, not a confusing exit 3
# hunt with no link back to "a worktree was recently deleted."
#
# Reuses M3's false-positive lesson verbatim: a bare `.worktrees/` substring is not evidence
# (fleet's own worktree.rs legitimately embeds the relative literal `.worktrees/{name}` as a git
# argument format string, DOT-prefixed, never SLASH-prefixed) -- only an ABSOLUTE compile-time
# CARGO_MANIFEST_DIR value containing `/worktrees/` (slash-prefixed) with `keel/fleet` later in
# that same string is D34's actual signature. See M3.sh's header for the full story.
#
# NOTE ON A REAL BUG FOUND WHILE MUTATION-TESTING THIS DETECTOR (B22): M3.sh's own regex,
# `^/.*/worktrees/.*/keel/fleet$`, anchors both ends of the LINE. That is silently wrong for a
# Rust binary: rustc/LLVM lay adjacent `&str` literals back-to-back in rodata with NO null
# terminator between them (Rust strings are not C strings), so `strings` frequently reports one
# "line" that is several unrelated string constants glued together, e.g.
# `...keel/fleet::f::{{closure}}fleet::console::tests...` with no separator after `/keel/fleet`.
# The trailing `$` then never matches and the detector silently passes a genuinely stale binary --
# reproduced directly against both a `cargo test` deps/ binary AND a plain `cargo build` release
# binary built the same way, so M3.sh carries this exact false-negative too, not just this
# detector's first draft (a proxy is not the property -- confirmed by actually building a stale
# binary and running the check, not by reading the regex and reasoning about it). This detector
# therefore does NOT reuse M3's regex verbatim: it drops the anchors and matches the
# `/worktrees/...keel/fleet` shape as a substring, which is sufficient on its own to exclude the
# dot-prefixed relative literal (see paragraph above) without needing line boundaries at all.
#
# `cargo test`'s own binary is a hashed filename (`fleet-<hash>`) under debug/deps/, not a fixed
# path -- and a deps/ dir commonly holds SEVERAL stale ones left over from earlier builds (cargo
# does not clean superseded hashes on its own). Any one of them being stale is the bug: check
# every candidate under this test-binary naming convention, not only the newest.
set -u
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/keel/target}"
DEPS="$TARGET_DIR/debug/deps"

[ -d "$DEPS" ] || exit 77   # never ran `cargo test` here: not mechanisable in this context

# Candidate test binaries: executable regular files named fleet-<hash> with no extension --
# excludes fleet-<hash>.d (dep-info, text, not executable), and libfleet-*.rlib / *.rmeta (never
# executable either, but excluded by name defensively in case perm bits ever lie on some fs).
CANDIDATES=()
while IFS= read -r -d '' f; do
  case "$f" in
    *.d|*.rlib|*.rmeta) continue ;;
  esac
  CANDIDATES+=("$f")
done < <(find "$DEPS" -maxdepth 1 -type f -perm -u+x -name 'fleet-*' -print0 2>/dev/null)

[ "${#CANDIDATES[@]}" -gt 0 ] || exit 77   # no compiled test binary found: not mechanisable here

TOTAL_HITS=0
STALE_BINS=()
for BIN in "${CANDIDATES[@]}"; do
  HITS=$(strings "$BIN" 2>/dev/null | grep -cE '/worktrees/.*/keel/fleet' || true)
  HITS="${HITS:-0}"
  if [ "$HITS" -gt 0 ]; then
    TOTAL_HITS=$((TOTAL_HITS + HITS))
    STALE_BINS+=("$BIN ($HITS hit(s))")
  fi
done

echo "M13: checked ${#CANDIDATES[@]} cached test binary(ies) under $DEPS (denominator: ${#CANDIDATES[@]})"
if [ "${#STALE_BINS[@]}" -gt 0 ]; then
  echo "M13: ${#STALE_BINS[@]} cached test binary(ies) embed an absolute CARGO_MANIFEST_DIR path under a worktrees/ dir -- built while a worktree that may since be deleted was in use:"
  for b in "${STALE_BINS[@]}"; do echo "    - $b"; done
  echo "    Fix: cargo clean -p fleet, then rebuild from the main tree."
  exit 1
fi
exit 0
