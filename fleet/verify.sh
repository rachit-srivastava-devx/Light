#!/usr/bin/env bash

# D22: PATH normalisation. git exports a REDUCED PATH to hooks, so `verify.sh` run as .githooks/
# pre-commit could not see cargo/uv/conftest even though an interactive shell could. The gate then
# failed on a normal machine for an environment reason and reported it as a product failure.
# This is a mac-only tool (see TARGET.md), so the standard Homebrew and cargo locations are added
# explicitly rather than inherited by luck.
for d in /opt/homebrew/bin /usr/local/bin "$HOME/.cargo/bin" "$HOME/.local/bin"; do
  case ":$PATH:" in *":$d:"*) ;; *) [ -d "$d" ] && PATH="$d:$PATH" ;; esac
done
export PATH

# D23: strip inherited git context. git runs pre-commit hooks with GIT_INDEX_FILE set (relative,
# `.git/index`). Stages here create throwaway repos and run `git add`/`git commit` inside them; an
# inherited index path redirects those writes. Unset before any stage runs, so `verify.sh` behaves
# identically whether invoked by hand or as .githooks/pre-commit.
unset GIT_DIR GIT_INDEX_FILE GIT_WORK_TREE GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES
# THE GATE WALL. Run before any claim of done. Exit codes are typed (AGENTS.md rule 7):
#   0 ok | 3 environment fault | 6 invariant/quality violation
# NOTE: never `$?` after a pipe (E1/S11). Every stage captures status directly.
set -u
ROOT="$(cd "$(dirname "$0")" && pwd)"; cd "$ROOT"
PASS=0; FAIL=0; SKIP=0; ENV_FAIL=0
LOG="$ROOT/var/verify.log"; mkdir -p "$ROOT/var"; : > "$LOG"
tool_available(){
  case "$1" in
    cargo-fmt) command -v cargo >/dev/null 2>&1 && cargo fmt --version >/dev/null 2>&1 ;;
    cargo-clippy) command -v cargo >/dev/null 2>&1 && cargo clippy --version >/dev/null 2>&1 ;;
    cargo-deny) command -v cargo-deny >/dev/null 2>&1 ;;
    cargo-audit) command -v cargo-audit >/dev/null 2>&1 ;;
    cargo-llvm-cov) command -v cargo >/dev/null 2>&1 && cargo llvm-cov --version >/dev/null 2>&1 ;;
    *) command -v "$1" >/dev/null 2>&1 ;;
  esac
}
stage(){ # stage <name> <required|advisory> <probe> <reason> <cmd...>
  local name="$1" req="$2" probe="$3" reason="$4"; shift 4
  if ! tool_available "$probe"; then
    printf '  SKIPPED WITH A REASON %-18s (%s)\n' "$name" "$reason"; SKIP=$((SKIP+1))
    [ "$req" = required ] && ENV_FAIL=$((ENV_FAIL+1))
    return
  fi
  printf '  .... %-26s' "$name"
  if "$@" >>"$LOG" 2>&1; then printf '\r  ok   %-26s\n' "$name"; PASS=$((PASS+1))
  else printf '\r  FAIL %-26s (see var/verify.log)\n' "$name"; FAIL=$((FAIL+1)); fi
}
echo "== fleet verify =="
stage "fmt"            required cargo-fmt   "cargo fmt unavailable" cargo fmt --manifest-path keel/Cargo.toml --all -- --check
stage "clippy -D warn" required cargo-clippy "cargo clippy unavailable" cargo clippy --manifest-path keel/Cargo.toml --all-targets -- -D warnings
stage "unit tests"     required cargo       "cargo unavailable" cargo test --manifest-path keel/Cargo.toml --quiet
stage "acceptance builds" required cargo   "cargo unavailable" bash -c 'cargo build --manifest-path keel/Cargo.toml --quiet && cargo build --manifest-path tests/tools/b3oracle/Cargo.toml --quiet'
stage "cargo-deny"     required cargo-deny   "cargo-deny unavailable" cargo deny --manifest-path keel/Cargo.toml check
stage "cargo-audit"     required cargo-audit  "cargo-audit unavailable" bash -c "cd keel && cargo audit --quiet"
stage "secrets"        required gitleaks      "gitleaks unavailable" gitleaks detect --no-banner --redact -s .
# M2: cargo now builds into CARGO_TARGET_DIR (repo-external, per AGENTS.md). The acceptance suite
# locates the binary via $FLEET_BIN / $B3ORACLE_BIN; export them so the suite finds the binary
# wherever the shared cache actually built it, defaulting to the legacy in-repo path. This makes
# verify.sh robust whether or not CARGO_TARGET_DIR is set in the invoking shell, so the pre-commit
# hook (.githooks/pre-commit -> exec verify.sh) gets the same correct behaviour with no extra export.
export FLEET_BIN="${CARGO_TARGET_DIR:-$ROOT/keel/target}/debug/fleet"
export B3ORACLE_BIN="${B3ORACLE_TARGET_DIR:-${CARGO_TARGET_DIR:-$ROOT/tests/tools/b3oracle/target}}/debug/b3oracle"
stage "acceptance"     required bash         "bash unavailable" bash tests/acceptance/p0.sh
# F02: three lld.v1 mirrors (TS/orb, Python/orb, Rust/fleet), one shared fixture corpus, one
# comparator -- proves the three hand-mirrored validators actually agree (F02 lane contract §7.4).
# Needs orb's node_modules + relay-py .venv established (see FLEET-LEARNINGS.md's S0 entries).
stage "lld-crosslang"  required bash         "bash unavailable" bash tests/acceptance/lld-crosslang.sh
# F06: the lld-ready gate's own guard (both directions: bad fixture refused, control accepted --
# 03 §2.4's "disabled at startup with a banner, not silently trusted") and its Rust<->TS
# comparator (a second, independent proof beyond F06's own unit tests -- F06 lane contract §2.3).
stage "lld-ready-selftest"  required bash    "bash unavailable" bash tests/acceptance/lld-ready-selftest.sh
stage "lld-ready-crosslang" required bash    "bash unavailable" bash tests/acceptance/lld-ready-crosslang.sh
# The swarm contract is RED on purpose. Blueprint 06 (per-agent strict SDLC) and requirements 4/8
# are the product; everything green above them is the substrate they would run on. A green wall over
# a missing product is exactly the failure this repo exists to catch, so the gap is a failing stage
# and not a TODO in a document.
stage "readme"         required bash "bash unavailable" bash tests/acceptance/readme.sh
stage "swarm"          required bash         "bash unavailable" bash tests/acceptance/swarm.sh
# B14: `fleet status --json` cold-start contract (empty store: exit 0, but the JSON and human
# render must say so unambiguously). See docs/delta.d/B14.md.
stage "status"         required bash         "bash unavailable" bash tests/acceptance/status.sh
stage "policy"          required conftest    "conftest unavailable" bash policy/run.sh
# keel-gate::recur (11-THREE-MEMORY-LAYERS.md §4): evaluates promoted lesson signatures against
# the diff, not the whole tree. One signature today (E1, memory/lessons/E1-pipe-exit-code.json),
# status Candidate — self-authored fixture, proven both directions (bin/recur-gate.sh --selftest)
# plus a real positive/negative control, but not yet independently reviewed for promotion to
# Enforced. Wired as a real (blocking) stage anyway: the CHECK is proven; only the LESSON's
# provenance is unreviewed, and those are different claims.
stage "recur"           required bash        "bash unavailable" bash bin/recur-gate.sh
# SAST (B11 item 1 / S1). Pinned rulesets + a tuned exclude-list — see bin/semgrep-gate.sh for the
# full rationale (31 findings on first run, all reviewed as false positives for this codebase's
# legitimate systems-programming patterns).
stage "semgrep"          required semgrep     "semgrep unavailable" bash bin/semgrep-gate.sh
# Secret scanning (B11 item 1 / S1) — SCOPED to trivy's secret scanner only; see bin/trivy-gate.sh
# for why vuln+misconfig (a DB download on first run) are a deliberately separate, open piece of
# work rather than silently folded into this stage.
stage "trivy"            required trivy       "trivy unavailable" bash bin/trivy-gate.sh
# B11 item 3: coverage has no threshold (BACKLOG.md: "do not set a threshold you cannot defend") —
# advisory only, publishes a real denominator (region/line/function %) via cargo-llvm-cov. See
# bin/coverage-report.sh and docs/delta.d/B11.md for why this is advisory, not a pass/fail gate.
stage "coverage" advisory cargo-llvm-cov "cargo-llvm-cov unavailable" bash bin/coverage-report.sh
if [ "${FLEET_MUTANTS:-0}" = "1" ]; then
  stage "mutants"       advisory cargo-mutants "cargo-mutants unavailable" bash bin/mutants-gate.sh
else
  printf '  SKIPPED WITH A REASON %-18s (%s)\n' "mutants" "set FLEET_MUTANTS=1 - full pass ~24min"
  SKIP=$((SKIP+1))
fi
stage "attest-smoke" advisory witness "witness/cosign unavailable" bash bin/witness-smoke.sh
stage "pytest" required uv "uv unavailable" bash -c 'cd crew && uv run pytest -q'  # D20: NOT in the gate before.
# 10/10 was green while crew/tests carried a red test. A suite the gate never runs is the same
# defect as the vacuous policy (D19): a check that cannot fail. Publish it or do not claim it.

stage "detectors"      required bash "bash unavailable" bash bin/detector-integrity.sh
stage "corpus"         required bash         "bash unavailable" bash tests/corpus/run.sh
echo "-- $PASS passed, $FAIL failed, $SKIP skipped (denominator: $((PASS+FAIL+SKIP)) stages) --"
[ "$ENV_FAIL" -eq 0 ] || exit 3
[ "$FAIL" -eq 0 ] || exit 6
