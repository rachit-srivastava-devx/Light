#!/usr/bin/env bash
# M1 — the mutants stage must stay opt-in, and its skip must stay visible.
# A merge silently reverted the FLEET_MUTANTS guard (D27) and nothing caught it: the gate still
# PASSED, it just took 24 minutes instead of 90 seconds. A gate slow enough to be bypassed will be
# bypassed. Nothing else in this tree asserts the property, so a green run said nothing about it.
#
# RETARGETED (fleet-verify relocation): `verify.sh`, where this guard used to live, was deleted --
# the gate table moved to `crates/fleet-verify/src/registry.rs`, driven by `fleet gate`. The
# `FLEET_MUTANTS` check itself now lives one layer out, in the real `ToolProbe` impl
# (`src/dispatch/verify_ports.rs`) rather than inside `fleet-verify`, which never reads its own
# environment (see that crate's `lib.rs`) -- IO stays injected, not inlined. Per the S1 fix (see
# `corpus/run.sh`), the scan root comes from `$FLEET_TARGET_REPO` (the real `--repo` target this
# script was invoked against), never from this script's own on-disk location.
set -u
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"

PROBE="$ROOT/src/dispatch/verify_ports.rs"
[ -r "$PROBE" ] || { echo "M1: verify_ports.rs unreadable at $PROBE"; exit 1; }
grep -q 'FLEET_MUTANTS' "$PROBE" || { echo "M1: mutants stage is no longer opt-in (FLEET_MUTANTS guard missing)"; exit 1; }

RENDERER="$ROOT/src/print/renderer.rs"
[ -r "$RENDERER" ] || { echo "M1: renderer.rs unreadable at $RENDERER"; exit 1; }
grep -q 'Outcome::Skip' "$RENDERER" || { echo "M1: the opt-out path no longer renders a visible SKIP outcome"; exit 1; }
grep -q '"SKIP"' "$RENDERER" || { echo "M1: the opt-out path no longer prints a visible SKIP label"; exit 1; }

# Integrity of THIS file is enforced externally by gates/corpus/MANIFEST.sha256 (D28) -- an in-file
# self-check cannot work: renaming the asserted string also renames the grep that looks for it.
exit 0
