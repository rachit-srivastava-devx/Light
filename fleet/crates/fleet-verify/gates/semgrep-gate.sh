#!/usr/bin/env bash
# B11 / S1: SAST gate. Called as a required `verify.sh` stage: `stage "semgrep" required semgrep
# ... bash bin/semgrep-gate.sh`.
#
# This file itself did not exist anywhere in git history before B11 -- a previous session's own
# BACKLOG.md entry (S1, "DONE, live session 2026-08-28") claimed this was wired and cited
# `docs/delta.d/S1-semgrep-trivy.md` as proof, and `verify.sh`'s `stage "semgrep" ...` line really
# was committed (`fd6f79e`) -- but the gate script and delta doc it references were never
# `git add`ed in that session, so the wiring was a dead reference in every fresh checkout/worktree
# from the moment it landed. See `docs/delta.d/B11.md` for the full audit. Do not let this happen
# again: if you edit this file, `git add` it in the SAME commit as any `verify.sh` change.
#
# Why not `--config=auto`: semgrep's `auto` composes a project-specific ruleset from semgrep.dev
# and can silently run a reduced/empty set when it cannot resolve that (a network blip would make
# this gate report clean instead of failing loud -- exactly the "a check cheaper to fake than to
# satisfy will be faked" trap, PRINCIPLES.md #1/#11). Pin explicit registry packs instead, and
# treat zero scanned files or a hard scan error as an environment failure, not a pass.
# NOTE (fleet-verify relocation, S1 fix): this copy lives at the gates root (materialized by
# fleet-verify, or a caller-supplied override) rather than under `bin/` inside a full checkout, so
# it cannot resolve the scan target from its own script path. `REPO` is the cwd this script was
# invoked with -- fleet's runner sets it to the `--repo` target (verify_runner_bounded.rs) -- so
# this scans the repo the caller actually named, never the gates root `$0` happens to live in.
set -u
REPO="${FLEET_TARGET_REPO:-$(pwd)}"

OUT="$(mktemp -t semgrep-gate-out.XXXXXX)"
ERR="$(mktemp -t semgrep-gate-err.XXXXXX)"
trap 'rm -f "$OUT" "$ERR"' EXIT

# Registry packs: p/security-audit (cross-language secrets/injection/crypto smells, and the
# yaml.github-actions + bash.lang.security + package_managers.dependabot rules that caught real
# findings below), p/secrets (dedicated secret-pattern rules; gitleaks already covers most of this
# but the rule sets are not identical), p/python (crew/'s adapters), p/rust (this repo's actual
# language -- registry coverage is thin, see the exclude-rule block below for what it does find).
# `p/bash` is not a real registry pack (404) -- bash coverage comes bundled in p/security-audit.
#
# 2026-09-03 B11 first real run, full findings reviewed by hand (not skimmed):
#   - 10x yaml.github-actions.security.github-actions-mutable-action-tag: REAL. Fixed for real by
#     pinning actions/checkout, actions/cache, anchore/sbom-action/download-syft and
#     softprops/action-gh-release to commit SHAs in ci.yml/release.yml/supply-chain.yml.
#     dtolnay/rust-toolchain@stable kept unpinned with an inline `nosemgrep` + reason: `@stable` IS
#     that action's documented, intentional moving reference (pinning it would freeze the Rust
#     *version* tested, not just the action's own code).
#   - 2x package_managers.dependabot.dependabot-missing-cooldown: REAL. Fixed for real by adding
#     `cooldown: {default-days: 3}` to both .github/dependabot.yml entries.
#   - 2x bash.lang.security.ifs-tampering (bin/freelane.sh:212,253): reviewed, TRUE false
#     positive -- `IFS=,; echo "${arr[*]}"` inside `$(...)` scopes IFS to that subshell only, it
#     never leaks to the caller. Suppressed with an inline `nosemgrep` + reason at each site, not a
#     blanket flag, so a *different* IFS misuse elsewhere still gets caught.
#   - 32x rust.lang.security.{unsafe-usage,temp-dir,current-exe,args}: reviewed line-by-line (every
#     hit read in context, not sampled):
#       * unsafe-usage (18, all in main.rs): every one is a raw libc FFI call this CLI's fd-3
#         supervised-worker architecture needs and has no safe-Rust equivalent for --
#         mkdtemp/socketpair/send/gmtime_r/mem::zeroed<libc::tm>. The rule is a blanket "contains
#         `unsafe`" grep; it cannot see that these are already the safety boundary, not a violation
#         of it. clippy (a required, separate `verify.sh` stage) is what actually checks these
#         blocks for soundness.
#       * temp-dir (10, meter.rs/ratchet.rs/worktree.rs/repl.rs/swarm.rs): every hit is inside
#         `#[cfg(test)]`/`mod tests`, and every path is disambiguated (pid, an AtomicU64 counter, or
#         real `mktemp -d`) -- the TOCTOU/predictable-path concern this rule targets does not apply
#         to per-test-run scratch dirs that are never shared as an attack surface. Production code's
#         own temp dir (`mktemp_dir()` in main.rs) already uses libc::mkdtemp, not env::temp_dir(),
#         and correctly does NOT trigger this rule.
#       * current-exe (3): main.rs:2782 and repl.rs:571 are the same fd-3 re-exec architecture
#         (deliberately trusts its own binary path); ratchet.rs:1719 is test-only.
#       * args (1, main.rs:38): `env::args()` at the CLI's own top-level entry point -- the
#         panic-on-invalid-UTF8 this rule warns about is the correct fail-fast behaviour for a
#         clap-based CLI, not a vulnerability.
#     All four rust rule IDs are excluded below with this comment as the record. Re-review this
#     block whenever new `unsafe`/`temp_dir()`/`current_exe()`/`args()` call sites are added --
#     an exclude-rule this broad is exactly the kind PRINCIPLES.md warns to re-audit periodically,
#     not trust forever.
if ! semgrep \
      --config p/security-audit \
      --config p/secrets \
      --config p/python \
      --config p/rust \
      --exclude-rule rust.lang.security.unsafe-usage.unsafe-usage \
      --exclude-rule rust.lang.security.temp-dir.temp-dir \
      --exclude-rule rust.lang.security.current-exe.current-exe \
      --exclude-rule rust.lang.security.args.args \
      --exclude '*/target/*' \
      --exclude '.git/*' \
      --exclude 'var/*' \
      --exclude 'tmp/*' \
      --exclude '**/__pycache__/*' \
      --exclude 'keel/mutants.out.old/*' \
      --json --quiet \
      "$REPO" >"$OUT" 2>"$ERR"; then
  echo "semgrep-gate: semgrep exited non-zero (scan error, not a findings verdict):" >&2
  cat "$ERR" >&2
  exit 1
fi

python3 - "$OUT" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
paths = d.get("paths", {})
scanned = len(paths.get("scanned", []))
results = d.get("results", [])
errors = [e for e in d.get("errors", []) if e.get("type") != "PartialParsing"]
partial = [e for e in d.get("errors", []) if e.get("type") == "PartialParsing"]

print(f"semgrep-gate: {scanned} files scanned, {len(results)} findings, "
      f"{len(errors)} hard errors, {len(partial)} partial-parse warnings "
      f"(bash parser limits on complex scripts, not missed code -- see docs/delta.d/B11.md)")

if scanned == 0:
    print("semgrep-gate: FAIL -- zero files scanned. This is the silent-no-op failure mode "
          "(registry unreachable, or every path excluded) -- treat as broken, not clean.",
          file=sys.stderr)
    sys.exit(1)

if errors:
    print("semgrep-gate: FAIL -- hard parse/scan errors (excluding known bash-parser partial "
          "parses):", file=sys.stderr)
    for e in errors:
        print(f"  {e}", file=sys.stderr)
    sys.exit(1)

if results:
    print("semgrep-gate: FAIL -- new findings with no reviewed exclusion. Either fix the finding, "
          "or add a justified `nosemgrep: <rule-id> -- <reason>` comment at the site (see "
          "bin/freelane.sh for the pattern) -- do not widen the --exclude-rule list in this script "
          "without adding the same kind of dated, line-referenced review recorded above it:",
          file=sys.stderr)
    for r in results:
        print(f"  {r['check_id']} {r['path']}:{r['start']['line']}", file=sys.stderr)
    sys.exit(1)

print("semgrep-gate: 0 open findings. See docs/delta.d/B11.md for the full reviewed history of "
      "every finding this gate has ever produced.")
sys.exit(0)
PY
