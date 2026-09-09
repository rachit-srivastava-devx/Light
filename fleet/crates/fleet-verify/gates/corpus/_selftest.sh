#!/usr/bin/env bash
set -u

DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
# S1 fix, self-test isolation: every OTHER detector now scans `${FLEET_TARGET_REPO:-$(pwd)}` (the
# real `--repo` target) instead of a path relative to its own script -- correct for a real `fleet
# gate` run, but wrong for THIS script's own purpose, which is proving "plant one fixture line,
# the detector fires; remove it, the detector goes clean" in isolation. Scanning a real repo (this
# one included) risks unrelated pre-existing text elsewhere in the tree matching a detector's
# pattern regardless of the planted probe. So this exports its OWN clean scratch dir as
# `FLEET_TARGET_REPO` for every detector `prove()` below invokes, overriding whatever the caller's
# cwd or `FLEET_TARGET_REPO` happened to be.
ROOT="$(mktemp -d)" || exit 3
export FLEET_TARGET_REPO="$ROOT"
PROBE_DIR="$(mktemp -d "$ROOT/.detector-selftest.XXXXXX")" || exit 3
trap 'rm -rf "$ROOT" "$PROBE_DIR"' EXIT HUP INT TERM

proven=0
total=25

comment_probe="$PROBE_DIR/comment-skip.sh"
printf '%s\n' '# false | true; rc=$?' > "$comment_probe"
python3 -B - "$ROOT" "$comment_probe" "$DIR" <<'PY'
import sys
from pathlib import Path

root = Path(sys.argv[1])
probe = Path(sys.argv[2])
# `_scan.py` sits NEXT TO this script. This used to be `root / "tests" / "corpus"`, a path the
# fleet-verify migration deleted -- so the import raised ModuleNotFoundError and the whole
# detector self-test failed without anyone noticing it had stopped proving anything.
sys.path.insert(0, sys.argv[3])
from _scan import lines

raise SystemExit(6 if any(path == probe for path, _, _ in lines(root)) else 0)
PY
comment_rc=$?
rm -f "$comment_probe"
if [ "$comment_rc" -ne 0 ]; then
  printf 'FAIL shared scanner yielded a shell/Python comment\n'
  exit 6
fi

prove() {
  detector=$1
  probe=$2

  caught_output=$(bash "$DIR/$detector.sh" 2>&1)
  caught_rc=$?
  rm -f "$probe"
  clean_output=$(bash "$DIR/$detector.sh" 2>&1)
  clean_rc=$?

  if [ "$caught_rc" -eq 1 ] && [ "$clean_rc" -eq 0 ]; then
    proven=$((proven + 1))
    return
  fi

  printf 'FAIL %s: planted=%s removed=%s\n' "$detector" "$caught_rc" "$clean_rc"
  [ "$caught_rc" -eq 1 ] || printf '%s\n' "$caught_output"
  [ "$clean_rc" -eq 0 ] || printf '%s\n' "$clean_output"
}

probe="$PROBE_DIR/A1.sh"
printf '%s\n' 'sha256 custom compress round' > "$probe"
prove A1 "$probe"

mkdir -p "$PROBE_DIR/v2"
probe="$PROBE_DIR/v2/A3.txt"
printf '%s\n' 'parallel implementation' > "$probe"
prove A3 "$probe"

probe="$PROBE_DIR/A4.txt"
printf '%s\n' 'compress2 implementation' > "$probe"
prove A4 "$probe"

probe="$PROBE_DIR/A8.sh"
printf '%s\n' 'success rate 42%' > "$probe"
prove A8 "$probe"

probe="$PROBE_DIR/B10.sh"
printf '%s\n' 'false | true; rc=$?' > "$probe"
prove B10 "$probe"

probe="$PROBE_DIR/C1.sh"
printf '%s\n' 'mktemp digest-path' > "$probe"
prove C1 "$probe"

probe="$PROBE_DIR/giant-hash-cache.bin"
dd if=/dev/zero of="$probe" bs=2000001 count=1 2>/dev/null
prove C11 "$probe"

mkdir -p "$PROBE_DIR/C12"
probe="$PROBE_DIR/C12/.gitignore"
printf '%s\n' 'learning-corpus/' > "$probe"
prove C12 "$probe"

mkdir -p "$PROBE_DIR/C19"
probe="$PROBE_DIR/C19/package.json"
printf '%s\n' '{"scripts":{"check":"node missing.js"}}' > "$probe"
prove C19 "$probe"

probe="$PROBE_DIR/C2.sh"
printf '%s\n' 'ledger >> production.log' > "$probe"
prove C2 "$probe"

probe="$PROBE_DIR/C22.js"
printf '%s\n' 'shown = count; total = count' > "$probe"
prove C22 "$probe"

probe="$PROBE_DIR/C24.sh"
printf '%s\n' 'helper() { :; }; bash -c helper' > "$probe"
prove C24 "$probe"

probe="$PROBE_DIR/C26.py"
printf '%s\n' 'path.endswith("/")' > "$probe"
prove C26 "$probe"

probe="$PROBE_DIR/C3.sh"
printf '%s\n' 'npx package@latest' > "$probe"
prove C3 "$probe"

probe="$PROBE_DIR/C6.py"
printf '%s\n' 'generator == verifier' > "$probe"
prove C6 "$probe"

probe="$PROBE_DIR/C8.sh"
printf '%s\n' 'while IFS=x read line; do cat file; done' > "$probe"
prove C8 "$probe"

probe="$PROBE_DIR/C9.sh"
# DELIBERATE: this bad mktemp template is the FIXTURE, not a defect -- it is written into a probe
# file so `prove C9` can watch the C9 detector actually fire on it. A detector never seen to fail
# is not a detector. Any linter flagging this line is matching the fixture it exists to test.
printf '%s\n' 'mktemp sample.XXXXXXsuffix' > "$probe"
prove C9 "$probe"

probe="$PROBE_DIR/S4.sh"
printf '%s\n' 'receipt=$(mktemp)' > "$probe"
prove S4 "$probe"

probe="$PROBE_DIR/S6.py"
printf '%s\n' 'remember_store = alpha' 'enforcement_store = beta' > "$probe"
prove S6 "$probe"

probe="$PROBE_DIR/S9.py"
printf '%s\n' 'embedding_model = "/tmp/model"' > "$probe"
prove S9 "$probe"

probe="$PROBE_DIR/T1.sh"
printf '%s\n' "grep '[^\\n]' file" > "$probe"
prove T1 "$probe"

probe="$PROBE_DIR/T15.py"
printf '%s\n' 'evidence_path = "/tmp/evidence"' > "$probe"
prove T15 "$probe"

probe="$PROBE_DIR/T20.sh"
printf '%s\n' 'false | true; rc=$?' > "$probe"
prove T20 "$probe"

mkdir -p "$PROBE_DIR/.github"
probe="$PROBE_DIR/.github/T5.sh"
printf '%s\n' 'git status --short' > "$probe"
prove T5 "$probe"

probe="$PROBE_DIR/T6.sh"
printf '%s\n' "sed -i '' file" > "$probe"
prove T6 "$probe"

printf '%d of %d detectors proven to still fire\n' "$proven" "$total"
[ "$proven" -eq "$total" ] || exit 6
