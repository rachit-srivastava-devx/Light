#!/usr/bin/env bash
# B11 item 3: coverage measurement. Before this, `cargo-mutants`'s mutation score was the ONLY
# proxy for "is this code tested", and it answers a different question than coverage does (a
# mutant can be caught by an assertion that happens to run near the mutated line without the test
# suite ever having exercised most of the codebase; coverage answers "did any test touch this
# line at all", which mutation testing does not directly measure).
#
# ADVISORY, not a pass/fail gate: BACKLOG.md's own instruction is "do not set a threshold you
# cannot defend." This repo has never measured coverage before today, so there is no historical
# baseline, no agreed floor, and no owner who has looked at which specific 0%-covered code is
# acceptable (CLI arg-parsing boilerplate, say) versus a real gap. Publishing the number honestly,
# every run, is the whole job here; picking an arbitrary "must be >= 70%" line before anyone has
# reviewed what's under/over it would be exactly the invented-threshold PRINCIPLES.md warns
# against. `verify.sh` runs this as `advisory` (see the `stage()` helper: advisory failures are
# reported but never flip the gate red) for that same reason -- flip it to `required` (or a
# `required`-with-a-real-percentage-check) only after a human has reviewed a real report and
# picked a number they can defend.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

OUT="$(mktemp -t coverage-report.XXXXXX)"
trap 'rm -f "$OUT"' EXIT

if ! cargo llvm-cov --manifest-path keel/Cargo.toml --summary-only --json --output-path "$OUT" 2>&1; then
  echo "coverage-report: cargo-llvm-cov run failed -- see output above" >&2
  exit 1
fi

python3 - "$OUT" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
totals = d["data"][0]["totals"]

def pct(key):
    v = totals.get(key, {})
    covered = v.get("covered", 0)
    count = v.get("count", 0)
    percent = v.get("percent", 0.0)
    return covered, count, percent

for label, key in (("lines", "lines"), ("functions", "functions"), ("regions", "regions")):
    covered, count, percent = pct(key)
    print(f"coverage-report: {label}: {covered}/{count} ({percent:.2f}%)")

print("coverage-report: no pass/fail threshold is set (B11: 'do not set a threshold you cannot "
      "defend') -- this is a published denominator for a human to review, not a gate.")
PY
