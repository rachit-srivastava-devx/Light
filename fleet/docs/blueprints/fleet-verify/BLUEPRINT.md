# BLUEPRINT — `fleet-verify`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-verify`
- **One-line purpose:** Run the committed set of shell verification gates, classify each one as a
  typed `Pass`/`Fail`/`Skip` with a published numerator/denominator, and refuse to call a gate a
  `Pass` when it measured nothing — the re-run-every-check gate wall, orchestrated instead of
  scripted, with zero subprocess-spawning of its own.
- **Build branch:** `extract` (MIGRATION-PLAN §3 row 8) — `fleet/verify.sh`'s `stage()`/
  `tool_available()` orchestration pattern (232 lines) and the four `bin/*-gate.sh` scripts' own
  "published denominator" discipline are a near-exact match for the *shape* this crate types. The
  24 individual checks (fmt, clippy, cargo-deny, semgrep, trivy, mutants, ...) themselves stay
  exactly as they are today — bash, python, external binaries — this crate does not reimplement a
  single one of them. See §2's non-goals and the design-decision note at the end of this file.
- **Imports:** `fleet-types` (`ExitCode`, for the aggregate report's process exit code — see §3).
- **Imported by:** `src/` (composition root — the `fleet verify` CLI subcommand builds the real
  `ToolProbe`/`ProcessRunner` implementations, supplies the committed `GateSpec` table, calls
  `run_all`, prints the report, and returns its `ExitCode` as the process exit status). `none yet`
  among sibling crates. The git pre-commit hook (`.githooks/pre-commit`) today execs `verify.sh`
  directly and is unaffected by this crate until `src/` re-points it at the compiled binary — that
  re-pointing is `src/`'s job (P5 "integrate" in MIGRATION-PLAN §4), not this crate's.

## 2. Responsibility & non-goals

**Owns:** the typed shape a verification run reports in — one `GateSpec` (name, requirement level,
required tool, invocation) per committed check, one `Verdict` (`Pass(Denominator)` /
`Fail{reason, denominator}` / `Skip{reason, was_required}`) per gate after running it, and the

**As built**, this crate additionally owns resolving *where a gate script's bytes actually are* —
`spec.rs`'s `GateCommand::Script` variant plus the `gates/` module (§7's note, §8's file layout):
every wrapped script is embedded into the binary at compile time and, by default, materialized to a
temp directory on first use (executable bit set), so a `GateSpec` no longer needs a `bin/...`-
relative path that only resolves when the process's cwd happens to be a live fleet checkout. A
caller may still override this with a real on-disk gates root (`GatesRoot::from_override`), mirroring
this crate's existing `ToolProbe`/`ProcessRunner` injected-port style. This is new scope beyond this
blueprint's original "the caller resolves `command`'s literal argv, this crate never builds it"
framing — flagged here rather than silently left undocumented.

Restating the original "owns" list, unchanged: the
**"a gate that measured nothing FAILED" invariant**: even when a wrapped shell script exits `0`,
if its own stdout does not publish a nonzero `total` in its denominator, this crate reclassifies
that gate as `Fail`, uniformly, for every gate, whether or not that gate's own script remembered to
guard against reporting `0/0` itself (today only `semgrep-gate.sh`/`trivy-gate.sh`/
`recur-gate.sh`/`corpus/run.sh`/`policy/run.sh`/`detector-integrity.sh` do this themselves, in six
slightly different ways — `mutants-gate.sh`, plain `cargo test`, and others do not). This crate also
owns aggregating every gate's verdict into one `Report` with a typed overall `ExitCode`
(`Ok`/`Env`/`Invariant`, mirroring `verify.sh:125-126`'s `exit 3`/`exit 6` tail).

**Non-goals (the seam):**
- Does **not** reimplement `cargo fmt`/`clippy`/`cargo-deny`/`cargo-audit`/`gitleaks`/`semgrep`/
  `trivy`/`cargo-mutants`/`conftest`/`cargo-llvm-cov`/`witness` in Rust. Every one of those stays a
  wrapped external tool or shell script, invoked exactly as `verify.sh` invokes it today. Per the
  brief's design question: recommend **(a) orchestrate as subprocesses behind a typed interface**,
  not (b) reimplement — rewriting `trivy`'s CVE/secret database logic, `semgrep`'s multi-language
  parser, or `cargo-mutants`' mutation engine in Rust would be months of duplicated, worse-tested
  work for zero behavior change; the actual defect class this migration is fixing (S1's dead
  `git add`-forgotten gate scripts, D19's vacuous-policy pattern, the untyped "just parse `$?`"
  wiring) lives entirely in the *orchestration* layer, not inside any one tool.
- Does **not** spawn a subprocess itself. Every gate's actual invocation goes through the caller-
  supplied `ProcessRunner` (§3) — this crate never calls `std::process::Command::new` in its own
  logic, only in its own tests' fake implementations of the trait.
- Does **not** probe `command -v`/`--version` itself either — that goes through the caller-supplied
  `ToolProbe` (§3), for the same reason: this crate types the *decision* ("required tool absent →
  Skip, and Skip on a required gate → the run is an environment fault"), not the mechanism that
  answers "is cargo-deny on PATH."
- Does **not** decide *what* the 24 gates are supposed to check, beyond their committed name/
  requirement/command — writing a new check (a new mutation floor, a new semgrep rule) is a change
  to the wrapped script or to `bin/*-gate.sh`, not to this crate.
- Does **not** write to the ledger/receipt chain (`fleet-store`'s job) or print human/JSON output
  (`src/`'s job) — this crate returns a `Report` value; formatting and persisting it are the
  caller's concern, exactly as `fleet-router`'s `decide()` returns a `Decision` and leaves printing
  to `src/`.
- Does **not** read the environment, a clock, or an RNG. `run_all`'s only inputs are the committed
  `GateSpec` table and the two injected trait objects.

## 3. Public API contract

```rust
//! Typed orchestration of fleet's shell verification gates.
//!
//! This crate answers one question -- "given the committed set of verification gates, what did
//! each one actually prove" -- and answers it the same way regardless of which 24 (or 30, or 12)
//! shell scripts are wired in. It spawns no subprocess and probes no tool itself: both are injected
//! via `ToolProbe`/`ProcessRunner` so the whole orchestration logic is unit-testable without ever
//! shelling out. The one non-negotiable rule this crate enforces that no individual gate script is
//! trusted to enforce on its own: a gate that reports it measured zero of anything is a `Fail`,
//! never a silent `Pass`, no matter what exit code its wrapped script returned.

use fleet_types::ExitCode;

// =====================================================================================
// A. Requirement level and tool probing -- verify.sh:35-45 `stage`/`tool_available`
// =====================================================================================

/// Whether a gate missing its required tool makes the whole run an environment fault (`Required`,
/// `verify.sh:125` `exit 3`) or is merely noted and skipped (`Advisory`, e.g. `coverage`/
/// `attest-smoke`, `verify.sh:110,117`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Requirement {
    Required,
    Advisory,
}

/// The external tool a gate needs on `PATH` before it can even attempt to run. Mirrors
/// `verify.sh:25-34`'s `tool_available` match arms; `Named` covers every tool that is checked by a
/// plain `command -v` today (`gitleaks`, `conftest`, `semgrep`, `trivy`, `witness`, `uv`, `bash`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeTool {
    Cargo,
    CargoFmt,
    CargoClippy,
    CargoDeny,
    CargoAudit,
    CargoLlvmCov,
    CargoMutants,
    Named(&'static str),
}

/// Injected IO boundary #1: "is this tool usable on this machine right now." The real
/// implementation (owned by `src/`) shells `command -v`/`<tool> --version`, exactly as
/// `verify.sh:25-34` does; tests inject a fixed `BTreeSet<ProbeTool>` of "available" tools instead.
pub trait ToolProbe {
    fn available(&self, tool: ProbeTool) -> bool;
}

// =====================================================================================
// B. Subprocess execution -- injected, never called directly by this crate
// =====================================================================================

/// The raw result of running one gate's command line. `exit_code` is the wrapped script's own
/// process exit status (today inconsistent across gates -- `mutants-gate.sh`/`recur-gate.sh`/
/// `policy/run.sh` use fleet's `0/3/6` taxonomy, `semgrep-gate.sh`/`trivy-gate.sh` use Python's
/// bare `sys.exit(1)`, `corpus/run.sh` uses `0/1/5/6` -- this crate does not normalize that
/// (non-goal), it only asks "was it zero").
#[derive(Clone, Debug)]
pub struct ProcessOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Injected IO boundary #2: actually run a gate's argv. The real implementation (`src/`) wraps
/// `std::process::Command`; tests inject scripted `ProcessOutput`s per command, so every
/// orchestration rule below is proven without a real shell, a real `cargo`, or a real filesystem.
pub trait ProcessRunner {
    fn run(&self, command: &[&str]) -> ProcessOutput;
}

// =====================================================================================
// C. The published denominator -- the crate's core invariant
// =====================================================================================

/// A published numerator/total pair (`caught/total`, `passed/checked`, `scanned-files/0-findings`,
/// ...). Deliberately cannot be constructed with `total == 0` -- see `new`'s contract. This is the
/// direct typed encoding of `mutants-gate.sh:32-37`'s bash-level "a numerator of 0 [total] can
/// never be violated by any score" refusal, generalized to every gate instead of hand-written once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Denominator {
    numerator: u64,
    total: u64,
}

/// `Denominator::new` was asked to construct a `0/0` (or `N/0`) denominator.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("a denominator with total=0 asserts nothing and cannot be published as a Pass")]
pub struct ZeroDenominator;

impl Denominator {
    /// `total == 0` is refused unconditionally -- there is no such thing as a valid "0 things were
    /// measured" denominator in this crate, matching `trivy-gate.sh:63-67`'s / `semgrep-gate.sh:
    /// 104-108`'s own "zero scanned is the silent-no-op failure mode, not a pass" language, made a
    /// type-level fact instead of a per-script python `if scanned == 0` reviewers must remember to
    /// keep writing on every new gate.
    pub fn new(numerator: u64, total: u64) -> Result<Self, ZeroDenominator> { unimplemented!() }
    pub fn numerator(self) -> u64 { unimplemented!() }
    pub fn total(self) -> u64 { unimplemented!() }
}

/// What a gate's stdout parser found, before orchestration applies the zero-total rule. Distinct
/// from `Denominator` itself: `Unparseable` (no recognizable marker in stdout at all) and
/// `Counted(0, 0)` (a marker was found and it says zero) are different failure *causes* that a
/// report should distinguish for a human, even though both end up `Fail` (see `classify` in §6).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DenominatorResult {
    Counted(u64, u64),
    Unparseable,
}

// =====================================================================================
// D. One committed gate -- the data table mirroring verify.sh's stage list
// =====================================================================================

/// One gate's identity and policy, as committed data -- mirrors one `stage "name" req probe reason
/// cmd...` line in `verify.sh` (e.g. `verify.sh:47` for `fmt`, `:111` for `mutants`). `argv[0]` is
/// the command name, the rest its arguments; this crate never builds the argv from parts at
/// runtime, it only ever hands the caller's `ProcessRunner` the exact committed slice.
#[derive(Clone, Copy)]
pub struct GateSpec {
    /// Stable identity used in `GateResult`/`Report` and in receipts a caller writes downstream.
    /// Never renamed once shipped. Matches `verify.sh`'s stage name column verbatim (`"fmt"`,
    /// `"clippy -D warn"`, `"mutants"`, ...).
    pub id: &'static str,
    pub requirement: Requirement,
    pub probe: ProbeTool,
    pub command: &'static [&'static str],
    /// Extracts this gate's published denominator from its own stdout/stderr. A fn pointer (not a
    /// closure) so `GateSpec` stays `Copy` and the committed table (§5's `registry.rs`) is a plain
    /// `const` array, exactly like `fleet-router`'s `ORDER`.
    pub parse_denominator: fn(stdout: &str, stderr: &str) -> DenominatorResult,
}

// =====================================================================================
// E. Verdict and report
// =====================================================================================

/// Why a gate's stdout could not be trusted as a `Pass` even though the wrapped process exited 0.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailReason {
    /// The wrapped process exited nonzero; the raw code is kept for diagnostics, not normalized
    /// (see `ProcessOutput`'s doc comment -- different gates use different nonzero conventions).
    NonZeroExit(i32),
    /// Exit was 0, but `parse_denominator` returned `Counted(_, 0)` -- the gate measured nothing.
    MeasuredNothing,
    /// Exit was 0, but `parse_denominator` returned `Unparseable` -- the gate's stdout carried no
    /// recognizable denominator marker at all (a stronger red flag than `MeasuredNothing`: the
    /// gate's own output format may have silently changed and this crate can no longer tell whether
    /// it measured anything).
    Unparseable,
}

/// One gate's outcome after orchestration. Never constructed by a `GateSpec`'s own
/// `parse_denominator` -- only `classify` (§6) builds one, which is what makes the
/// "measured nothing is Fail" rule impossible for an individual gate to route around.
#[derive(Clone, Debug)]
pub enum Verdict {
    Pass(Denominator),
    Fail { reason: FailReason, denominator: Option<Denominator> },
    Skip { reason: String, was_required: bool },
}

#[derive(Clone, Debug)]
pub struct GateResult {
    pub id: &'static str,
    pub verdict: Verdict,
}

/// The full run's outcome. `exit_code()` mirrors `verify.sh:125-126`'s tail: any required gate
/// skipped (missing tool) outranks a plain failure and reports `ExitCode::Env`; any gate `Fail`
/// (with every required tool present) reports `ExitCode::Invariant`; otherwise `ExitCode::Ok`.
#[derive(Clone, Debug)]
pub struct Report {
    pub results: Vec<GateResult>,
}

impl Report {
    pub fn passed(&self) -> usize { unimplemented!() }
    pub fn failed(&self) -> usize { unimplemented!() }
    pub fn skipped(&self) -> usize { unimplemented!() }
    /// Count of `Skip{was_required: true, ..}` -- these are `verify.sh`'s `ENV_FAIL` counter.
    pub fn env_faults(&self) -> usize { unimplemented!() }
    /// `ExitCode::Env` if `env_faults() > 0`; else `ExitCode::Invariant` if `failed() > 0`; else
    /// `ExitCode::Ok`. Matches `verify.sh:125-126`'s `[ "$ENV_FAIL" -eq 0 ] || exit 3` /
    /// `[ "$FAIL" -eq 0 ] || exit 6` ordering exactly (env-fault is checked, and wins, first).
    pub fn exit_code(&self) -> ExitCode { unimplemented!() }
}

// =====================================================================================
// F. Orchestration -- the only two fns that touch the injected traits
// =====================================================================================

/// Run one gate to completion: probe → (skip | run → classify). Never panics; every branch below
/// produces a `GateResult`, there is no path that returns nothing.
pub fn run_gate(spec: &GateSpec, probe: &dyn ToolProbe, runner: &dyn ProcessRunner) -> GateResult {
    unimplemented!(
        "1) if !probe.available(spec.probe): GateResult{{id, verdict: Skip{{reason: format!(\"{{}} \
         unavailable\", ...), was_required: spec.requirement == Requirement::Required}}}}. \
         2) else: let out = runner.run(spec.command); classify(spec, &out) -- see classify() below."
    )
}

/// Run every committed gate in order, in a fresh `Report`. Order is preserved from `specs` (callers
/// that want `verify.sh`'s exact stage order pass the committed table in that order) but gates are
/// independent -- nothing here assumes gate N's outcome affects gate N+1's inputs, matching
/// `verify.sh`'s own stages (each one is a standalone `stage(...)` call, none reads a prior stage's
/// result).
pub fn run_all(specs: &[GateSpec], probe: &dyn ToolProbe, runner: &dyn ProcessRunner) -> Report {
    unimplemented!("Report{{ results: specs.iter().map(|s| run_gate(s, probe, runner)).collect() }}")
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `Denominator` | `total > 0` always — enforced at the single constructor `new`, never bypassed by a field-literal (fields are private). | A gate reporting `Pass` while its own published total is `0` — the type that carries "I measured N of M" cannot exist with `M == 0`. |
| `Verdict::Fail{ reason, denominator }` vs `Verdict::Pass(Denominator)` | A `Pass` always carries a valid (`total > 0`) `Denominator`; a `Fail` may carry `None` (`Unparseable`) or `Some` (e.g. a `NonZeroExit` gate that still printed a partial count). `Skip` carries neither. | A caller reading `report.results` and finding a `Pass` variant that lacks any published evidence — the variant itself cannot be constructed that way (see `classify`, §6). |
| `GateSpec` | `command` is always `'static` data from the caller's committed table, never assembled from user/environment input at call time. | A gate's argv being influenced by anything other than the reviewed, committed table — no argument injection surface exists inside this crate. |
| `Report.exit_code()` | Env-fault check strictly outranks invariant-fail check, matching `verify.sh:125-126`'s literal ordering (`ENV_FAIL` checked before `FAIL`). | A run with one missing required tool AND one real invariant failure being reported as `ExitCode::Invariant` (masking the environment problem) instead of `ExitCode::Env` — the ordering is fixed in `exit_code()`'s body, not left to call-site discretion. |

**Money/precision:** no money type in this crate. `Denominator`'s two fields are `u64`, never
float — a mutation kill *rate* or a coverage *percent* is a downstream presentation concern
(`bin/coverage-report.sh` itself already treats coverage as advisory-only, non-gating, per its own
comment and `verify.sh:107-110`); this crate only ever carries the raw integer counts a gate
published, never a derived ratio.

**Clock/RNG/IO injection points:** `ToolProbe` (tool-availability check) and `ProcessRunner`
(subprocess spawn) are the crate's only two IO boundaries, both trait objects supplied by the
caller — `run_gate`/`run_all` never call `std::process::Command`, `command -v`, or read any
environment variable directly. No clock, no RNG anywhere in this crate.

## 5. Reuse map

Source read in full: `fleet/verify.sh` (232 lines), `fleet/bin/mutants-gate.sh` (123 lines),
`fleet/bin/semgrep-gate.sh` (130 lines), `fleet/bin/trivy-gate.sh` (78 lines),
`fleet/bin/recur-gate.sh` (279 lines), `fleet/bin/detector-integrity.sh` (29 lines),
`fleet/policy/run.sh`, `fleet/bin/coverage-report.sh`, `fleet/bin/witness-smoke.sh` (tails).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `verify.sh:25-34` (`tool_available`) | `case` dispatch mapping a probe name to a `command -v`/`--version` check. | logic yes, location no | Becomes the *contract* of the caller's real `ToolProbe` impl (owned by `src/`, not this crate) — this crate only defines the `ProbeTool` enum + trait; the actual `command -v` shelling stays bash-adjacent glue in `src/`, matching this crate's zero-subprocess non-goal. |
| `verify.sh:35-45` (`stage`) | Probe → run → PASS/FAIL/SKIPPED-WITH-A-REASON, updates 4 counters (`PASS`/`FAIL`/`SKIP`/`ENV_FAIL`). | yes, retyped | This is `run_gate` (§3 F) — same branch structure (probe first, then run), but returns a typed `GateResult` instead of incrementing shared bash counters; `Report`'s `passed()`/`failed()`/`skipped()`/`env_faults()` (§3 E) are those same four counters computed from `Vec<GateResult>` instead of mutated in place. |
| `verify.sh:46-124` (the `stage "name" ... cmd` call list, 24 lines) | The committed list of gates + their required/advisory flag + probe + literal command. | yes, retyped as data | Becomes the `GateSpec` table (a `registry.rs` module, one `const GATES: &[GateSpec]` — not shown in §3's contract body since it is *data*, not API shape, but every one of the 24 lines maps to exactly one `GateSpec{ id, requirement, probe, command, parse_denominator }` entry; e.g. `verify.sh:47` (`fmt`, required, `cargo-fmt` probe) and `:111` (`mutants`, advisory, `cargo-mutants` probe) are two such entries). |
| `verify.sh:111-116` (`FLEET_MUTANTS` env-var gate on whether to even attempt `mutants`) | Bash `if` around the `stage "mutants" ...` call, printing a `SKIPPED WITH A REASON` line directly (not via `stage`'s own probe path) when the env var is unset. | **no, reshaped** | This crate reads no environment variable (non-goal, §2) — whether the `mutants` `GateSpec` is included in the `specs` slice passed to `run_all` at all is the caller's (`src/`'s) decision, made by reading `FLEET_MUTANTS` itself before calling this crate. |
| `verify.sh:124` (final tally line, `"-- $PASS passed, $FAIL failed, $SKIP skipped (denominator: ...) --"`) | Human-readable summary print. | **no** | Presentation — `src/`'s job, built from `Report`'s public accessor methods (§3 E), same split as `fleet-router`'s `print_human`. |
| `verify.sh:125-126` (`exit 3`/`exit 6` tail) | Maps `ENV_FAIL`/`FAIL` counters to the process exit code. | yes, retyped | `Report::exit_code()` (§3 E), using `fleet-types::ExitCode` instead of a bare `exit N`. |
| `mutants-gate.sh:32-37` ("a numerator of 0 ... asserts nothing ... REFUSE") | Bash-level refusal when the *floor* (not the *measured* score) is uncalibrated at `0`. | inspiration, not lifted verbatim | This crate's `Denominator::new` refusing `total == 0` is a distinct, more general rule — it refuses an uncalibrated/empty *measurement*, not an uncalibrated *floor*; `mutants-gate.sh`'s own floor-file check (lines 15-51) stays exactly as-is inside the still-bash `mutants-gate.sh`, this crate does not re-implement it. |
| `mutants-gate.sh:106-111` (`caught`/`total` printf + `total -eq 0` check) | Publishes `mutants: caught=%d total=%d floor=%s`; separately refuses if `total == 0`. | yes, as a parser | `parse_denominator` for the `mutants` `GateSpec`: regex/`str::find` for `"caught="`/`" total="` in stdout → `DenominatorResult::Counted(caught, total)`. The bash script's own `total -eq 0` check (line 113) becomes redundant-but-harmless once this crate's `classify` also enforces it — belt and suspenders, not a conflict. |
| `semgrep-gate.sh:100-108` (`f"{scanned} files scanned, {len(results)} findings, ..."` + `scanned == 0` check) | Publishes a scanned-file count; separately fails on zero. | yes, as a parser | `parse_denominator` for `semgrep`: parses `"<N> files scanned"` → `Counted(len(results) findings not the numerator here — see note, N, )`. Concretely: numerator = 0-findings-is-good is inverted from "caught/total" gates, so this parser reports `Counted(scanned - findings, scanned)` (files-clean over files-scanned) rather than findings-over-scanned, keeping "higher numerator is better" consistent across every `GateSpec` in the table — a deliberate normalization decision, called out for Opus review (see divergence note). |
| `trivy-gate.sh:61-67` (`f"{total} secret findings across {len(results)} reported targets"` + `len(results) == 0` check) | Same shape as semgrep. | yes, as a parser | `parse_denominator` for `trivy`: `Counted(reported_targets - total_findings, reported_targets)`, same normalization as semgrep above. |
| `recur-gate.sh:146` (`"recur-gate: checked=%d flagged=%d (signatures=1: E1)"`) | Publishes checked/flagged counts from the awk scan. | yes, as a parser | `parse_denominator` for `recur`: `Counted(checked - flagged, checked)`. |
| `detector-integrity.sh:29` (`"$N detectors match the manifest (denominator: $N)"`) | Publishes a plain `N/N` on success (no partial-pass shape — either every detector matches or the script already `exit 6`'d earlier). | yes, as a parser | `parse_denominator` for `detectors`: `Counted(N, N)` parsed from the trailing `"(denominator: N)"` marker; the script's own earlier `exit 6` paths (empty manifest, count mismatch, hash mismatch) are already `NonZeroExit` cases this crate's `classify` catches independently of parsing stdout at all. |
| `policy/run.sh` (`"-- $P passed, $F failed (denominator: $((P+F)) policies) --"` + `zero policies checked` refusal) | Publishes pass/fail counts across `.rego` policies. | yes, as a parser | `parse_denominator` for `policy`: `Counted(P, P+F)`. |
| `tests/corpus/run.sh` (`"DENOMINATOR checked=%d total=%d ..."` line + `checked -gt 0` refusal, exit codes `0/1/5/6`) | Publishes a multi-field denominator line; already refuses `checked == 0`. | yes, as a parser | `parse_denominator` for `corpus`: `Counted(checked - caught, total)` (or an even richer per-field `Denominator` if a future revision widens the type — flagged in divergence note, this crate's `Denominator` today is a flat 2-tuple, `corpus`'s line has 7 fields). |
| `cargo test`'s own stdout (`"test result: ok. N passed; 0 failed; ..."`, standard `libtest` format) | No fleet script wraps this today — `verify.sh:49` calls `cargo test` directly with no denominator-publishing wrapper at all. | **no direct fleet source — greenfield parser** | `parse_denominator` for `unit tests`/`acceptance builds`: regex over libtest's own fixed `"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed"` line → `Counted(passed, passed + failed)`. This is new work this crate adds that `verify.sh` itself never had (today a `cargo test` that ran and reported `0 passed; 0 failed` — an empty test binary — would still print `stage "unit tests" ... ok` in `verify.sh`, exactly the vacuous-pass gap this crate's core invariant closes). |

## 6. Behavior spec

### `fn run_gate(spec: &GateSpec, probe: &dyn ToolProbe, runner: &dyn ProcessRunner) -> GateResult`

| Input dimension | Behavior |
|---|---|
| empty | `spec.command == &[]`: `probe.available` is still checked first (a `GateSpec` with an empty command but a real `probe` value still gets a Skip-or-run decision); if it runs, `runner.run(&[])` is the injected fake's problem to define — this crate never inspects `command`'s length itself, it only ever forwards the slice. |
| null / `None` | n/a — every field of `GateSpec` is a required, non-`Option` value; there is no "gate with a missing command" representable at this crate's boundary (a caller who wants an optional gate simply omits it from the `specs` slice passed to `run_all`). |
| wrong-type | n/a — no stringly-typed/erased input crosses this fn's boundary; `ProcessOutput.exit_code` is always `i32`, `stdout`/`stderr` always `String` (a non-UTF-8 subprocess output is the `ProcessRunner` impl's problem to lossy-convert before handing this crate a `ProcessOutput` — documented as the injected impl's contract, not re-validated here). |
| huge | A 50MB `stdout` string: `parse_denominator` fn pointers are expected to be `O(stdout.len())` scans (regex/`str::find`), never quadratic or unbounded-recursive — this crate does not itself impose a size cap, but a `GateSpec`'s parser is reviewed data (§5), not arbitrary code, so this is a per-parser correctness concern flagged in code review, not a runtime check. |
| negative | `ProcessOutput.exit_code: i32` can be negative (a signal-terminated process on Unix reports as negative via some `ProcessRunner` conventions) — `classify` treats any nonzero (`!= 0`, not `> 0`) as `NonZeroExit`, so a negative code is handled identically to a positive one, never silently treated as success. |
| duplicate | Calling `run_gate` twice with the same `spec` and a `ProcessRunner` that returns the same `ProcessOutput` both times yields two identical `GateResult`s — idempotent by construction (no shared mutable state anywhere in this fn). |
| concurrent | `run_gate` takes `&GateSpec` (shared) and `&dyn ToolProbe`/`&dyn ProcessRunner` (shared trait objects) and returns an owned `GateResult` — safe to call from multiple threads for different `GateSpec`s concurrently, *provided* the caller's `ProcessRunner`/`ToolProbe` impls are themselves `Sync` (a fact about the caller's impl, not this crate — see §11 thread-safety). |
| unicode / non-ASCII | A gate's stdout containing non-ASCII text (e.g. a file path with accented characters in a `trivy`/`semgrep` finding): `parse_denominator`'s marker search is a plain substring/regex scan over the `String`, which is already valid UTF-8 by Rust's `String` invariant — no separate handling needed, no panic possible from non-ASCII content alone. |
| already-exists | n/a — `run_gate` has no persisted state to collide with; running it any number of times never accumulates anything beyond its one return value. |
| partial-failure | The wrapped subprocess crashing mid-write (a `ProcessRunner` impl that returns a truncated `stdout`): looks identical to `Unparseable` if the truncation cut off the denominator marker, or to a genuine (possibly wrong) `Counted` if the truncation happened after the marker — this crate cannot distinguish "truncated" from "the script always prints the marker early" and does not try; that distinction, if ever needed, belongs to a more capable `ProcessRunner` (e.g. one that also reports whether the child was killed) supplying a richer `ProcessOutput`, a future extension not built here. |

### `fn classify(spec: &GateSpec, out: &ProcessOutput) -> Verdict` (private helper referenced above; the mechanism that enforces the crate's one invariant)

| Input dimension | Behavior |
|---|---|
| empty | `out.stdout == ""`: `spec.parse_denominator("", "")` is expected to return `Unparseable` (no marker in empty text) → `Fail{reason: Unparseable, denominator: None}`, even if `out.exit_code == 0`. |
| null / `None` | n/a — `ProcessOutput`'s fields are always-present `String`/`i32`, never `Option`. |
| wrong-type | n/a — no type erasure at this boundary. |
| huge | Covered under `run_gate`'s huge row above (parser cost is a per-`GateSpec` concern). |
| negative | `out.exit_code < 0` → `NonZeroExit(out.exit_code)`, same as any other nonzero value (see `run_gate`'s negative row). |
| duplicate | n/a — pure fn of its two inputs, no identity concept. |
| concurrent | Pure fn, no shared state — trivially safe. |
| unicode / non-ASCII | Same as `run_gate`'s row — no special handling needed beyond valid UTF-8. |
| already-exists | n/a — no persisted state. |
| partial-failure | `out.exit_code == 0` but `parse_denominator` returns `Counted(n, 0)` for any `n` (including `n > 0`, a malformed-but-technically-parseable line) → `Fail{reason: MeasuredNothing, denominator: None}` — `Denominator::new(n, 0)` is refused (§4), so this branch can never smuggle a zero-total `Denominator` into a `Pass` by constructing one first and hoping nobody checks; the type itself makes it unrepresentable. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace-path (`{ path = "../fleet-types" }`) | Supplies `ExitCode` for `Report::exit_code()`. |
| `thiserror` | `2.0.20` (matches `fleet/keel/Cargo.lock`'s already-resolved version) | `ZeroDenominator`'s typed error, and (as built) `gates::GateAssetError` (§8's `gates/` module). |
| `tempfile` | `3.27.0` (as built; matches `fleet/keel/Cargo.lock`) | **New dependency beyond this section's original projection.** `gates::materialize` (§8) writes the crate's embedded gate scripts out to a fresh temp directory (with the executable bit set) so an installed binary run from any cwd can still find and run them — see the `gates/` module note below. |
| `include_dir` | `0.7.4` (as built) | **New dependency beyond this section's original projection.** `gates::embed` (§8) embeds the `crates/fleet-verify/gates/` script tree into the binary at compile time, so `GateCommand::Script` variants resolve without depending on the process's cwd being a live fleet checkout. |

As built, `parse_denominator` implementations use plain `str::find`/`split`-based digit extraction
(`digits.rs`'s `after`/`before` helpers) — no `regex` dependency was needed, confirming this
section's original preference.

**As-built API addition not covered by §3's contract above:** `GateSpec.command` is no longer a bare
`&'static [&'static str]` — it is a `GateCommand` enum (`spec.rs`) with two variants, `OnPath(&'static
[&'static str])` (run directly via `$PATH`, e.g. `cargo test`) and `Script { relative: &'static str,
args: &'static [&'static str] }` (a gate script resolved at run time against a `GatesRoot`, §8's
`gates/` module, instead of a `bin/...`-relative literal that only worked when the process's cwd
happened to be a fleet checkout). `run_gate`/`run_all` (§3 F) both gained a fourth parameter,
`gates: &GatesRoot`, used only to resolve `Script` variants — `run_gate` now returns `Skip` (not a
panic or a silent empty argv) when a required script asset is unavailable. This is a real, load-
bearing signature change from this file's original `pub fn run_gate(spec: &GateSpec, probe: &dyn
ToolProbe, runner: &dyn ProcessRunner) -> GateResult` / `run_all(specs, probe, runner)` — see
`crates/fleet-verify/src/orchestrate.rs` for the exact real signatures, and `crates/fleet-verify/src/
gates/` for `GatesRoot`/`GateAssetError`. This crate still spawns no subprocess itself (§2's
non-goal): `gates::materialize` writes embedded *script* bytes to disk, it never executes them.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** `lib.rs` is a thin hub; the trait/type/data/logic
> split below keeps every file single-responsibility.

> **As built**, this tree gained a `digits.rs` helper module and a whole `gates/` submodule not in
> this section's original projection — the as-built `GateCommand::Script` variant (§7's note) needs
> a run-time-resolvable, location-independent way to find a gate script's bytes, which `gates/`
> provides (compile-time embedding + first-use materialization to a temp dir, or a caller-supplied
> override root). Tests gained `gates_root.rs` and `report_semantics.rs` to match. The list below is
> the real `find crates/fleet-verify -name '*.rs'` output:

```
crates/fleet-verify/
  Cargo.toml
  src/
    lib.rs              # module decls + re-exports only
    requirement.rs      # Requirement, ProbeTool
    ports.rs            # ToolProbe, ProcessRunner, ProcessOutput (the two injected boundaries)
    denominator.rs      # Denominator, ZeroDenominator, DenominatorResult
    digits.rs           # NEW (as built): after()/before() plain substring digit-extraction helpers shared by parsers.rs -- no regex needed
    spec.rs             # GateSpec, GateCommand (OnPath | Script{relative, args} -- see §7's note)
    verdict.rs           # FailReason, Verdict, GateResult
    report.rs             # Report + passed/failed/skipped/env_faults/exit_code
    classify.rs            # classify() (private; the invariant-enforcing fn)
    orchestrate.rs           # run_gate, run_all (public entry points; both now take a &GatesRoot, §7)
    parsers.rs                # one small fn per gate family (caught/total, scanned/findings,
                               # checked/flagged, libtest passed/failed, N/N), using digits.rs
    registry.rs                # const GATES: &[GateSpec] = &[...]; the committed gate table
    gates/                      # NEW (as built), not in this blueprint's original scope -- see §7's note
      mod.rs                    # re-exports: GateAssetError, GatesRoot
      embed.rs                  # compile-time embedding of crates/fleet-verify/gates/ (include_dir)
      error.rs                  # GateAssetError
      materialize.rs             # first-use extraction of an embedded script to a temp dir, executable bit set
      root.rs                    # GatesRoot: resolves a Script's `relative` path against the materialized dir or a caller override
  tests/
    denominator_invariant.rs     # Denominator::new rejects total==0; accepts total>0 incl. numerator==0
    classify_rules.rs            # every classify() branch in §6's table, via fake ProcessOutput values
    orchestration_probe_skip.rs  # run_gate/run_all with a fake ToolProbe: required-missing -> env
                                  # fault counted; advisory-missing -> skip, no env fault
    registry_integrity.rs        # GATES ids pairwise distinct; every id has a non-empty command
    parsers_table.rs             # one fixture per real gate script's stdout (copied verbatim
                                  # from §5's citations) -> asserts the exact Counted(n, total)
    gates_root.rs                # NEW (as built): GatesRoot resolution -- materialized-embed path vs. override path, GateAssetError cases
    report_semantics.rs          # NEW (as built): Report accessor/exit_code semantics, split out of/alongside classify_rules.rs
    support/mod.rs                # shared test fixtures
```

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-verify"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-types = { path = "../fleet-types" }
thiserror = "2.0.20"
tempfile = "3.27.0"
include_dir = "0.7.4"

[dev-dependencies]
# none required beyond std — fakes for ToolProbe/ProcessRunner are plain structs in tests/.
```
(as built — `tempfile`/`include_dir` are real dependencies now; see §7's note above.)

## 9. Test plan

**Unit tests** (in each module's `#[cfg(test)]`):
- `denominator_rejects_zero_total_regardless_of_numerator` — `Denominator::new(0, 0)` and
  `Denominator::new(7, 0)` both `Err(ZeroDenominator)`; `Denominator::new(0, 7)` is `Ok` (a
  legitimate "0 of 7 passed" *failure* is still a valid, informative denominator — it is only
  `total == 0` that asserts nothing, never `numerator == 0`).
- `report_exit_code_ranks_env_fault_over_invariant_fail` — a `Report` with one `Skip{was_required:
  true}` and one `Fail{..}` result reports `ExitCode::Env`, never `ExitCode::Invariant` (the §4
  ordering invariant, tested directly rather than only asserted in a doc comment).
- `report_counts_match_verify_sh_semantics` — a hand-built `Report` of 3 pass / 2 fail / 1 required-
  skip / 1 advisory-skip asserts `passed()==3, failed()==2, skipped()==2, env_faults()==1`.

**Integration tests** (calling only `run_gate`/`run_all`, with fake `ToolProbe`/`ProcessRunner`):
- `required_tool_missing_produces_env_fault_skip` — a `GateSpec` with `requirement: Required`
  whose probe reports unavailable → `Verdict::Skip{was_required: true, ..}`, and the process is
  never invoked (assert the fake `ProcessRunner` recorded zero calls).
- `advisory_tool_missing_produces_plain_skip` — same shape with `Requirement::Advisory` →
  `Skip{was_required: false, ..}`.
- `zero_exit_with_zero_total_is_fail_not_pass` — a fake `ProcessRunner` returning `exit_code: 0,
  stdout: "caught=0 total=0"` for a `GateSpec` whose `parse_denominator` extracts that pair →
  `Verdict::Fail{reason: MeasuredNothing, ..}`, **not** `Pass` — this is the single most important
  test in the suite; it is the direct proof of the crate's whole reason to exist.
- `zero_exit_with_unrecognized_stdout_is_fail_unparseable` — `exit_code: 0`, `stdout: "unexpected
  garbage"` → `Fail{reason: Unparseable, ..}`.
- `nonzero_exit_is_always_fail_even_with_a_valid_looking_denominator` — `exit_code: 1, stdout:
  "caught=40 total=40"` → `Fail{reason: NonZeroExit(1), denominator: Some(_)}` — a script that
  printed a perfect-looking denominator but still exited nonzero must never be read as a `Pass`.
- `run_all_preserves_spec_order_and_runs_every_gate_independently` — 5 `GateSpec`s where gate 2
  fails: assert `report.results.len() == 5` and gates 3-5 still ran (their fake `ProcessRunner`
  calls were recorded) — one gate's failure never short-circuits the rest, matching `verify.sh`'s
  own behavior of running every `stage(...)` call regardless of earlier ones' outcomes.

**Mutation-testing targets (`cargo mutants -p fleet-verify`):**
- Flipping `total == 0` to `total <= 0` (a no-op for `u64` but a plausible refactor mistake if
  `Denominator` were ever changed to a signed type) or deleting the `total == 0` check entirely in
  `Denominator::new` must be killed by `denominator_rejects_zero_total_regardless_of_numerator`.
- Swapping the env-fault/invariant-fail precedence in `Report::exit_code()` (checking `failed() >
  0` before `env_faults() > 0`) must be killed by `report_exit_code_ranks_env_fault_over_invariant_fail`.
- Changing `classify`'s `MeasuredNothing` branch to fall through to `Pass` when `exit_code == 0` and
  `Counted(_, 0)` must be killed by `zero_exit_with_zero_total_is_fail_not_pass` — the load-bearing
  test named above.

**Property tests:** not applicable — this crate's domain (a fixed enum classification over a small,
committed `GateSpec` table) has no natural algebraic invariant beyond the ones already covered as
named unit/integration tests above; a property test here would just be a fuzzer over
`ProcessOutput` fields, which the explicit boundary-value tests already cover more legibly.

## 10. Verification recipe

```bash
cd crates/fleet-verify
cargo test -p fleet-verify --all-targets
cargo clippy -p fleet-verify --all-targets -- -D warnings
cargo mutants -p fleet-verify
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration tests pass, 0 skipped — publish as `<passed>/<total>` (this crate
is small and pure enough that, like `fleet-router`, the floor should be every named mutation target
in §9 caught, published as `<caught>/<total mutants>`, not a partial-credit percentage). Clippy: 0
warnings. File-size gate: no output.

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`ZeroDenominator`) — `run_gate`/`run_all`/
      `classify` are themselves infallible by design (a bad gate outcome is a `Verdict::Fail` data
      value, not a Rust `Err`), the correct shape for an orchestrator whose whole job is to observe
      and classify failure, never to propagate it as its own error.
- [ ] Clock/RNG/IO are injected — trivially, by having none directly: `ToolProbe`/`ProcessRunner`
      are the sole IO-shaped inputs, both caller-supplied trait objects.
- [ ] Thread-safety documented: every public fn takes `&`-refs (`&GateSpec`, `&dyn ToolProbe`, `&dyn
      ProcessRunner`) and returns owned values with no interior mutability in this crate's own types
      — `Send + Sync` for the crate's own types is automatic; whether concurrent `run_gate` calls
      are safe additionally depends on the caller's `ToolProbe`/`ProcessRunner` impl being `Sync`,
      which is that impl's contract to document, not this crate's to enforce.
- [ ] No float used anywhere — `Denominator`'s two fields are `u64`; no percentage/rate type exists
      in this crate's public API.
- [ ] No self-grading — verification runs `cargo mutants`, not just this crate's own unit tests;
      denominator published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10) — restate the real
      numbers in the PR once the crate is built.
- [ ] Tests that touch the filesystem: none needed — every test drives fakes in memory; if a future
      test needs a temp file it must use `tempfile::tempdir()` (already in `fleet/keel/Cargo.lock`
      at `3.27.0`), never the repo tree.
- [ ] Every non-goal in §2 is absent from the code — no `std::process::Command` call, no `env::var`,
      no `command -v` shelling anywhere in `crates/fleet-verify/src/`; enforce with
      `grep -rn 'Command::new\|env::var' crates/fleet-verify/src/` returning nothing.
- [ ] No source file exceeds 80 lines — §8's split verified by the §10 `wc -l ... awk '$1>80'` gate
      before Opus review.

## 12. Definition of Done

`fleet-verify` is DONE when: §10's four commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught, file-size gate silent) run from `crates/fleet-verify/`;
every unchecked box in §11 is checked with its real numbers; `registry/services/REGISTRY.md` (this
is core CI/quality infrastructure, not a product feature, so `services/` per C1/L2) lists the crate;
and Opus has independently re-derived the "measured nothing is Fail" invariant from this blueprint
alone (without re-reading `verify.sh`), reproduced the `MeasuredNothing`-vs-`Pass` mutation by hand
against `zero_exit_with_zero_total_is_fail_not_pass`, and driven one real `run_all` call — with a
`ProcessRunner` fake wired to the *actual* recorded stdout of a real `mutants-gate.sh`/`semgrep-
gate.sh` run (not a synthetic fixture) — confirming the parsed `Denominator` matches what that real
run published.

---

## Divergence from MIGRATION-PLAN / notes for Opus

1. **Design decision, as asked in the brief:** orchestrate (option a), do not reimplement (option
   b). Justification is in §2's first non-goal bullet; the short version is that every one of the
   24 gates' actual defect-finding logic (semgrep's rulesets, trivy's DB, cargo-mutants' mutation
   engine, gitleaks' entropy heuristics) is mature, external, and correctly scoped already — the
   documented failure modes this migration is chasing (S1's un-committed gate script, D19's vacuous
   policy, an inconsistent ad hoc `$?`-check per script) are all orchestration-layer defects, which
   is exactly the layer this crate is scoped to fix. Reimplementing any of the 24 tools would trade
   a real, working detector for a worse one at large cost, with no defect-class addressed.

2. **The four `bin/*-gate.sh` scripts already self-check "measured nothing" in six subtly different
   ways** (`mutants-gate.sh`'s floor-vs-score two-part check, `semgrep`/`trivy`'s `scanned == 0`/
   `len(results) == 0`, `recur-gate.sh`'s implicit "no signature ever fires on an empty diff" via
   `checked`, `detector-integrity.sh`'s manifest-count match, `corpus/run.sh`'s `checked -gt 0`,
   `policy/run.sh`'s `zero policies checked`). This blueprint's `Denominator::new` rule is a *second*,
   independent, uniform enforcement of the same idea at the orchestration layer — deliberately
   redundant with the scripts' own checks, not a replacement for them, because a future new gate
   script that forgets to add its own check (exactly S1's failure pattern — the wiring existed, the
   file wasn't committed) is still caught here. Opus should confirm this redundancy is intentional
   and worth the (small) duplication, not flag it as scope creep.

3. **The `semgrep`/`trivy`/`recur`/`corpus` parsers in §5 normalize "lower is better" (findings,
   flagged mutants) into "higher numerator is better" (clean-of-total) so every `GateSpec` in the
   committed table shares one `Denominator` reading direction.** This is a judgment call made in
   this blueprint, not dictated by MIGRATION-PLAN or the brief — an alternative design would give
   `Denominator` a `direction: HigherIsBetter | LowerIsBetter` field instead of normalizing in the
   parser. Flagging this explicitly for Opus to pick: normalize-in-parser (this blueprint's choice,
   simpler `Denominator` type, parser must remember to invert) vs. tag-the-direction (more type
   surface, no parser-side inversion to get wrong). Either is implementable from this blueprint with
   a one-line change to `Denominator`'s shape if Opus prefers the latter.

4. **`corpus/run.sh` publishes 7 named fields** (`checked`/`total`/`excluded`/`caught`/
   `timeout_contention`/`timeout_confirmed`/`timeout_persistent`), not a plain 2-tuple. §5's parser
   collapses this into one `Denominator` (`checked - caught` over `total`) for uniformity with every
   other gate, which loses the timeout-vs-clean-failure distinction `corpus/run.sh`'s own exit codes
   (`0`/`1`/`5`/`6`) already preserve at the `NonZeroExit` level (a timeout-only run exits `5`, still
   correctly `Fail` via `classify`, just without the richer field breakdown surfaced in the
   `Denominator` itself). If a future consumer needs the full 7-field breakdown surfaced in typed
   form rather than only in the raw `stdout`/`stderr` strings already carried on `Fail`'s
   diagnostic path, that is a `Denominator` shape change, not a gap in this blueprint's coverage of
   `corpus`'s existing behavior.
