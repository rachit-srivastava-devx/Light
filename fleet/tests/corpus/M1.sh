#!/usr/bin/env bash
# M1 — the mutants stage must stay opt-in, and its skip must stay visible.
# A merge silently reverted the FLEET_MUTANTS guard (D27) and nothing caught it: the gate still
# PASSED, it just took 24 minutes instead of 90 seconds. A gate slow enough to be bypassed will be
# bypassed. Nothing else in this tree asserts the property, so a green run said nothing about it.
set -u
V="$(dirname "$0")/../../verify.sh"
[ -r "$V" ] || { echo "M1: verify.sh unreadable at $V"; exit 1; }
grep -q 'FLEET_MUTANTS' "$V" || { echo "M1: mutants stage is no longer opt-in (FLEET_MUTANTS guard missing)"; exit 1; }
grep -q 'SKIPPED WITH A REASON' "$V" || { echo "M1: the opt-out path no longer publishes a visible skip"; exit 1; }

# Integrity of THIS file is enforced externally by tests/corpus/MANIFEST.sha256 (D28) -- an in-file
# self-check cannot work: renaming the asserted string also renames the grep that looks for it.
exit 0
