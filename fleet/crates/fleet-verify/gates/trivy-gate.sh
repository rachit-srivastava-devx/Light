#!/usr/bin/env bash
# B11 / S1: secret-scanning gate. Called as a required `verify.sh` stage: `stage "trivy" required
# trivy ... bash bin/trivy-gate.sh`.
#
# This file itself did not exist anywhere in git history before B11 -- see bin/semgrep-gate.sh's
# header and docs/delta.d/B11.md for the full story: a previous session's own BACKLOG.md entry
# (S1) claimed this was wired, but the gate script was never `git add`ed, so `verify.sh`'s
# committed `stage "trivy" ...` line pointed at a file that did not exist in any fresh checkout.
#
# Scope: `trivy fs --scanners secret` only. `gitleaks` (a separate, already-real required stage)
# also does secret scanning; running trivy's independently-sourced rule set alongside it is
# defense in depth, not duplication -- the two use different regexes/entropy heuristics and have
# caught different classes of leak in other projects. Self-tested against real (fake) AWS/GitHub
# credentials before trusting it: 0 findings on this tree is a verified clean result, not an
# untested assumption -- see docs/delta.d/B11.md for the self-test transcript.
#
# `vuln`/`misconfig` scanning is DELIBERATELY NOT enabled here. `trivy fs --scanners vuln`
# downloads a multi-hundred-MB vulnerability DB bundle on first run; B11's investigation (2026-09-03)
# confirmed this DOES complete on a machine with normal internet access (unlike the S1-era note
# claiming it "stalled" -- that claim could not be verified either way since the S1 doc describing
# it was never committed, see docs/delta.d/B11.md), but a `verify.sh` stage that silently blocks
# for minutes downloading a DB on every contributor's first run -- and fails hard, un-skippably,
# for anyone genuinely offline -- is a materially different commitment than a fast, deterministic
# secret scan. That is real, separate scope (a cached/pre-fetched DB, a documented offline
# fallback, and its own false-positive review) and is left open, not silently dropped: tracked as
# a named follow-up, not claimed done here.
# NOTE (fleet-verify relocation): see semgrep-gate.sh's matching note -- ROOT is this script's
# own directory (the gates root), which is only a live fleet/ checkout when the caller supplies a
# gates-root override that points at one.
set -u
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

OUT="$(mktemp -t trivy-gate-out.XXXXXX)"
ERR="$(mktemp -t trivy-gate-err.XXXXXX)"
trap 'rm -f "$OUT" "$ERR"' EXIT

if ! trivy fs \
      --scanners secret \
      --skip-dirs keel/target \
      --skip-dirs tests/tools/b3oracle/target \
      --skip-dirs keel/mutants.out.old \
      --skip-dirs var \
      --format json --quiet \
      -o "$OUT" \
      . 2>"$ERR"; then
  echo "trivy-gate: trivy exited non-zero (scan error, not a findings verdict):" >&2
  cat "$ERR" >&2
  exit 1
fi

python3 - "$OUT" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
results = d.get("Results") or []
total = 0
findings = []
for r in results:
    secrets = r.get("Secrets") or []
    total += len(secrets)
    for s in secrets:
        findings.append(f"{r.get('Target')}:{s.get('StartLine')} {s.get('RuleID')} ({s.get('Severity')})")

print(f"trivy-gate: {total} secret findings across {len(results)} reported targets")

if len(results) == 0:
    print("trivy-gate: FAIL -- zero targets reported. This is the silent-no-op failure mode "
          "(measuring nothing is a failure, not a pass) -- trivy scanned nothing, which is "
          "not the same as trivy finding nothing.", file=sys.stderr)
    sys.exit(1)

if findings:
    print("trivy-gate: FAIL -- real secret findings:", file=sys.stderr)
    for f in findings:
        print(f"  {f}", file=sys.stderr)
    sys.exit(1)

print("trivy-gate: 0 open findings. Self-tested against fake-but-real-shaped AWS/GitHub "
      "credentials before this gate was trusted -- see docs/delta.d/B11.md.")
sys.exit(0)
PY
