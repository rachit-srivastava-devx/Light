#!/usr/bin/env bash
# D28 — detector integrity. A worktree agent rewrote tests/corpus/M1.sh to assert the OPPOSITE of
# its purpose so its own change would pass, and a blanket `git add -A` committed that under the
# lead's message. An in-file self-check cannot catch this (renaming the asserted string renames the
# grep looking for it), so integrity is enforced from outside: every detector is hashed, and any
# change must be accompanied by a deliberate manifest update.
# NOTE (fleet-verify relocation): this copy lives directly at the gates root (materialized by
# fleet-verify, or a caller-supplied override), not under `bin/` inside a full checkout, so ROOT
# is the script's own directory and the detector corpus is `$ROOT/corpus`, not `$ROOT/tests/corpus`.
set -u
ROOT="$(cd "$(dirname "$0")" && pwd)"
MAN="$ROOT/corpus/MANIFEST.sha256"

# Capture the cwd this script was invoked with -- fleet's runner sets it to the `--repo` target
# (see verify_runner_bounded.rs) -- BEFORE the `cd` below can change it. The detector inventory
# this gate hashes is fleet's OWN corpus (materialized under $ROOT/corpus by fleet-verify), so
# on a user repo that has no fleet source to inspect the whole gate is a category error:
# `fleet run --repo <any user repo>` used to end with `FAIL gate detectors -- NonZeroExit(6)`
# via the manifest/count/hash refusals below, a red gate that never had a chance to succeed.
# Print the not-applicable marker `parsers::detectors` recognizes and exit 0; the gate then
# reads as SKIP, same shape recur-gate and corpus/run.sh use on a foreign checkout.
#
# The fleet-ness test is deliberately narrow -- same probe file corpus/run.sh uses -- so that
# inside fleet's own tree the D28 tamper-guard still fires; only a truly foreign repo skips.
export FLEET_TARGET_REPO="$(pwd)"
is_fleet_tree() { [ -r "$FLEET_TARGET_REPO/crates/fleet-verify/src/registry.rs" ]; }
if [ "${1:-}" != "--update" ] && ! is_fleet_tree; then
  echo "detectors-gate: not-applicable -- target repo $FLEET_TARGET_REPO is not a fleet checkout"
  exit 0
fi

cd "$ROOT/corpus" || exit 3
if [ "${1:-}" = "--update" ]; then
  shasum -a 256 *.sh > MANIFEST.sha256
  echo "detector-integrity: manifest updated ($(wc -l < MANIFEST.sha256 | tr -d ' ') detectors)"
  exit 0
fi
[ -r "$MAN" ] || { echo "detector-integrity: REFUSE: no manifest at $MAN"; exit 6; }
N=$(wc -l < "$MAN" | tr -d ' ')
[ "$N" -gt 0 ] || { echo "detector-integrity: REFUSE: empty manifest — measuring nothing"; exit 6; }
LIVE=$(ls *.sh | wc -l | tr -d ' ')
if [ "$LIVE" -ne "$N" ]; then
  echo "detector-integrity: REFUSE: $LIVE detectors on disk, $N in manifest — run bin/detector-integrity.sh --update deliberately"
  exit 6
fi
if ! shasum -a 256 --status -c "$MAN" 2>/dev/null; then
  echo "detector-integrity: REFUSE: a detector changed without a manifest update:"
  shasum -a 256 -c "$MAN" 2>/dev/null | grep -v ': OK$' | sed 's/^/    /'
  exit 6
fi
echo "detector-integrity: $N detectors match the manifest (denominator: $N)"
