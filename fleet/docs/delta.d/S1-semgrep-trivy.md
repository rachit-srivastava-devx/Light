# S1 (partial) / B11 item 1 — semgrep + trivy wired, live session 2026-08-28

Expected: `semgrep --config=auto` and `trivy fs` wired into `verify.sh` per B11's acceptance bar,
with the real finding count and false-positive count published.

Actual:

- **semgrep**: `--config=auto` needs a live semgrep.dev connection to resolve a ruleset — a bare
  `--config=auto --quiet` run returned exit 0 with zero output, which is indistinguishable from "no
  rules ran" (confirmed: `--metrics=off --config=auto` errors outright, "cannot create auto config
  when metrics are off"). Switched to pinned rulesets (`p/rust`, `p/secrets`,
  `p/security-audit`) for a reproducible, auditable finding set.
- First real run against `keel/fleet/src`: **31 findings, 4 rule classes, 0 in `p/secrets`**.
  All 31 reviewed by hand:
  - `unsafe-usage` (17) — fires unconditionally on any `unsafe` block; this is a systems tool
    (socketpair/setsid/dup2/kill/pre_exec for real process-group control per AGENTS.md hard rule
    5). False positive for this codebase's *existing, reasoned-about* unsafe use.
  - `temp-dir` (10) — `env::temp_dir()` for scratch/history/build paths, not security-sensitive
    files. False positive.
  - `current-exe` (3) — `env::current_exe()` for fleet's own documented self-reexec pattern, not a
    privilege decision. False positive.
  - `args` (1) — `env::args()` vs `args_os()` panic-safety nit on the CLI's own argv. False
    positive in the security sense (not attacker-controlled).
  - **False-positive rate: 31/31 (100%) for this first-pass tuning.** Published honestly, not
    hidden — matches B11's own warning that a new gate's first run is mostly false positives.
  - Excluded all 4 via `--exclude-rule` in `bin/semgrep-gate.sh`, with the rationale inline.
    Re-confirmed 0 findings after exclusion. **This is a first tuning pass, not a permanent
    exemption** — the script says so and should be periodically re-run without the excludes.
- **trivy**: `--scanners vuln` (needs the vulnerability DB) and `--scanners misconfig` (needs a
  checks bundle) both stalled 4+ minutes with zero progress downloading from `mirror.gcr.io` in
  this session's environment — a real network constraint, not a code problem. Did not force it
  through with a short timeout that would silently no-op on a stall (that recreates the
  measured-nothing trap `policy/measured_nothing.rego` already refuses). Wired **only**
  `--scanners secret` (self-contained, no external DB, ~16s, 0 findings on this tree) in
  `bin/trivy-gate.sh`; vuln/misconfig left as a named TODO in that script for when the
  registry is reachable in a given environment.
- Attempted a positive control (plant a fake secret, confirm the gate catches it). The canonical
  AWS example key did NOT trip it — expected, that key is commonly allowlisted by secret scanners
  specifically because it appears in so much documentation, not a defect. A second attempt with a
  synthetic (non-real) credential-shaped string was **blocked by this repo's own
  `fleet/hooks/injection-guard.sh` PreToolUse hook** before the command even ran — a different,
  real layer already refusing secret-shaped literals in shell commands. Did not attempt to work
  around that guard; its refusal is itself informative (this class of risk already has a second
  line of defense) and constructing a workaround to defeat a safety hook would be the wrong move
  regardless of the testing goal.

What changed: `bin/semgrep-gate.sh` (new), `bin/trivy-gate.sh` (new), `verify.sh` (+2 stages).
Full `verify.sh`: **17 passed, 1 failed (corpus, B13, pre-existing), 1 skipped (mutants, opt-in) of
19 stages** — both new stages green.

What's left of B11: coverage measurement (`tarpaulin`/`llvm-cov`, absent entirely) and
`bin/perf-gate.sh` (wired or deleted, still neither) are untouched — out of scope for this pass.
What's left of S1: this closes only the "semgrep+trivy" piece. The router→`swarm dispatch` wiring
and the model-tier-into-the-Python-adapter piece are still open.
