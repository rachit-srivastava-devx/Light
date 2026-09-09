#!/usr/bin/env bash
# M2 — build artifacts must not live inside the repo.
# A 4.9GB CARGO_TARGET_DIR at the repo root made 24 of 97 detectors walk it; the corpus stage
# stopped returning at all (D30). Python's rglob walks the whole tree BEFORE the path filter runs,
# so a string exclusion does not prevent the traversal -- only keeping the artifacts out does.
set -u
ROOT="${FLEET_TARGET_REPO:-$(pwd)}"
N=$(cd "$ROOT" && python3 -c "import pathlib;print(sum(1 for _ in pathlib.Path('.').rglob('*')))" 2>/dev/null || echo 0)
[ "$N" -gt 0 ] || { echo "M2: could not count files -- measuring nothing is a failure"; exit 1; }
if [ "$N" -gt 15000 ]; then
  echo "M2: $N files in the tree (>15000). Build artifacts are almost certainly inside the repo;"
  echo "    set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30)."
  exit 1
fi
exit 0
