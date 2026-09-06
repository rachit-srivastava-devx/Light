# Stitch audit: how much Fleet should stop hand-writing

## Executive finding

The ratio is wrong. `keel/fleet/src/` contains **8,181 lines of hand-written Rust** and
`keel/fleet/Cargo.toml` names **21 direct crates** (19 runtime and 2 development): about **390 Rust
lines per direct crate**. Dependency count is not a quality metric by itself, but here it is a useful
smell because the code is reimplementing facilities from dependencies it already has—most clearly
CLI grammar, help, and completion generation despite `clap` and `clap_complete` already being
dependencies.

The opposite conclusion would also be wrong: most of these 8,181 lines are not generic plumbing.
They encode Fleet's unusual trust boundary and refusal semantics. No candidate reviewed here owns
the fd-3-only worker channel, parent-side provenance stamping, immutable diff freeze, typed exit
codes, refusal receipts, non-zero denominators, or Fleet's hash-chained ledger. Replacing those with
an 80%-fit orchestration framework would remove the product's differentiator along with the code.

The plausible target is **820–1,170 Rust lines deleted (10–14%)**, not a wholesale rewrite:

| Module | Lines now | Verdict | Plausible Rust lines deleted |
|---|---:|---|---:|
| `main.rs` | 2,461 | PARTIAL | 300–450 |
| `ratchet.rs` | 1,628 | PARTIAL | 350–500 |
| `graph.rs` | 1,351 | PARTIAL | 170–220 |
| `console.rs` | 1,093 | KEEP | 0–40 |
| `mcp.rs` | 537 | KEEP | 0–20 |
| `agent.rs` | 378 | KEEP | 0 now |
| `lifecycle.rs` | 352 | KEEP | 0 now |
| `swarm.rs` | 231 | KEEP | 0–20 |
| `roles.rs` | 148 | KEEP | 0–10 |
| **Total** | **8,181** |  | **820–1,170** |

These estimates count deletions from the modules as measured, including embedded Rust tests. They
do not pretend that moving the same logic into a shell script is a saving. A substitution counts
only when a maintained component owns the behavior and the remaining Fleet adapter is materially
smaller.

## Module audit

### `main.rs` — 2,461 lines — PARTIAL

**What it actually does.** This is not merely `main`. It contains at least six systems:

1. A hand-written command grammar and dispatcher for roles, swarm, agents, run, oracle,
   adjudication, attestation, ledger, ratchet, console, graph, impact, MCP, completion, doctor,
   version, and help.
2. The run coordinator: start receipt, worker launch, submission validation, diff capture and
   freeze, independent verifier, blind-suite measurement, adequacy, rollback rehearsal,
   attestation, oracle adjudication, and end receipt.
3. The worker security boundary: sanitized environment, a Unix socket mapped to fd 3, process
   groups, deadlines, termination, and parent validation of the one response packet.
4. Oracle registration and the two-oracle/four-quadrant adjudicator.
5. The append-only BLAKE3 ledger, locking, schema checks, sequence/hash verification, and receipt
   lookup during attestation verification.
6. Environment diagnostics, hand-written help, and hand-written shell completions.

**Existing components.** `clap` derive already owns command/subcommand/option parsing, required
arguments, help, version, and validation. `clap_complete` already owns completion generation from
that same command model. Both are already compiled into Fleet. Narrower candidates such as
`tempfile`, `wait-timeout`/`command-group`, and in-toto data types could remove small utilities, but
none owns the Fleet workflow or trust boundary and none should be adopted without a separate
behavioral trial.

**Estimated saving and loss.** Moving only command grammar, help, version, and completion to a
single `clap` derive tree should delete **300–450 Rust lines**, after accounting for a thin adapter
that maps parse failures to Fleet's exit/receipt contract. What is lost if done naively is material:
`clap` normally exits on its own, uses its own error text and exit codes, and does not write a Fleet
receipt. It can also change stdout/stderr placement and completion behavior. Those are interface
changes, not cosmetic differences.

Do **not** swap the coordinator, fd-3 launcher, freeze, ledger, or attestation verifier for a generic
orchestrator. The repo's evidence says these exact boundaries are where superficially working tools
have been vacuous or misclassified failures. A library that launches a subprocess but leaks
`FLEET_STATE`, merges stdout with the submission channel, or reports exit 3 as an agent failure is
not an 80% solution; it is a regression.

**Verdict: PARTIAL.** Swap the CLI surface to `clap`/`clap_complete`; keep the workflow and
provenance kernel hand-written. This is the highest-confidence large deletion because the
dependencies are already present and the current comment explicitly admits completion is
hand-written only because parsing is hand-written.

**Behavioral smoke test for the partial swap.** Before deleting the old parser, run both parsers
against a table containing every documented success form, every missing value, odd trailing
arguments, unknown commands/options, `--help`, `--version`, and all three completion shells. Compare
exit code, stdout, stderr, and ledger delta. In particular: a refused stateful command must append
exactly one valid refusal receipt; exit 3 must remain an environment fault; help/version/completion
must not create state; and a generated completion must expose every actual subcommand. Then run
`tests/acceptance/p0.sh` against the derive-based binary. Compilation alone proves none of this.

### `ratchet.rs` — 1,628 lines — PARTIAL

**What it actually does.** It parses seven fixed-schema quality metrics; represents rates as
millionths; validates and locks scorecards/marks; seeds immutable marks; refuses regressions;
advances strict improvements; computes mutation adequacy and Wilson intervals; records scorecards
and receipt-backed verdicts; and creates, signs, scopes, expires, and verifies narrow exceptions.
Roughly the last third of the file is adversarial tests for numeric parsing and exception scope.

**Existing components.** OPA/Rego through `conftest` already exists in this repository and is
already used for measured-nothing, attestation, and architecture policy. Rego is a good fit for the
pure decision portion: required metrics, higher/lower direction, actual-versus-mark comparison,
zero-denominator refusal, and whether an exception is a non-empty proper subset for the same
change and is not expired. It is not a fit for locks, atomic mark updates, BLAKE3 signing, receipt
append, fixed-point input parsing, or Wilson statistics. `cargo-machete` and `cargo-udeps` can
produce dependency-health inputs to the ratchet; they do not implement the ratchet.

**Estimated saving and loss.** Making Rego the owner of the decision matrix and replacing many
Rust branch tests with good/bad policy fixtures could delete **350–500 Rust lines**. The host must
still validate typed inputs, publish `{checked,total}`, map conftest absence to exit 3, append a
receipt on every refusal, hold the state lock, and apply an accepted mark update atomically.

The risk is unusually high. This repo has already had policies that parsed incorrectly, selected
the wrong namespace, examined zero rules, and accepted every bad fixture. Rego also makes it easy
to accidentally treat absent JSON values as benign or to reintroduce non-fixed numeric behavior.
Moving persistence or provenance into policy would save lines by destroying the boundary.

**Verdict: PARTIAL.** Move only deterministic policy-as-data into Rego, and only after a shadow
trial. Keep measurement, canonicalization/signing, state transitions, locking, typed exits, and
receipt publication in Rust.

**Behavioral smoke test for the partial swap.** Feed the current Rust decision function and the
candidate Rego policy the same matrix: each of seven metrics better/equal/worse; missing and wrong-
typed metrics; `checked=0`; `checked>total`; unknown, wildcard, duplicate, empty, all-metric,
wrong-change, expired, future, and signature-tampered exceptions. Require identical accept/refuse
results and named metric/value diagnostics. For the CLI arm, a bad scorecard must add exactly one
receipt containing the non-zero denominator; conftest absence must exit 3 without being labelled an
agent failure. Finally shadow both engines on real scorecards for a bounded period and require zero
divergences. A passing good fixture without the bad control is explicitly insufficient.

### `graph.rs` — 1,351 lines — PARTIAL

**What it actually does.** There are two different graphs in this file:

* Test-only Rust module reachability walks top-level functions from `main::dispatch` and fails if a
  shipped module has no reachable public entry point or if the denominator is zero.
* The production graph recursively scans Rust, Python, and Bash; parses definitions and calls with
  tree-sitter; assigns persistent symbol UUIDs; preserves aliases over Git renames; stores files,
  symbols, edges, aliases, coverage, and digests in SQLite; refuses stale/under-floor indices; and
  queries reverse dependencies to a bounded depth.

**Existing components.** `cargo-modules 0.27` is a concrete candidate for Rust module structure and
dependency/reachability checks. `cargo-machete 0.9` and `cargo-udeps 0.1.61` detect unused Cargo
dependencies. They are valuable architecture gates, but neither answers “which Rust/Python/Bash
functions depend on this symbol?”, preserves identity over file renames, or serves Fleet's JSON
impact contract. None is a replacement for the production cross-language index.

**Estimated saving and loss.** If `cargo-modules` can demonstrate dispatcher-to-module reachability
on this actual crate, it can replace approximately **170–220 lines** of test-only parser/traversal
code. The production index should remain. Replacing it with `cargo-modules` would lose Python and
Bash, symbol-level reverse edges, rename aliases, stale-tree detection, SQLite persistence, depth,
and coverage output—the majority of the feature. Adding machete/udeps as new gates saves **zero**
lines unless an existing, equivalent dependency check is removed; counting them as a graph rewrite
would be accounting fiction.

**Verdict: PARTIAL.** Trial `cargo-modules` only for the narrow Rust module-reachability gate. Keep
the production graph until a real cross-language code-index CLI reproduces its contract. Do not
adopt machete/udeps under the banner of replacing impact analysis.

**Behavioral smoke test for the partial swap.** On a disposable copy, run the candidate against the
current good tree and require a non-zero published denominator. Then add a syntactically valid
module with a public function but no path from dispatch and require failure naming that module;
also run an empty-crate fixture and require failure, not `0 of 0` success. Compare the candidate's
reachable-module set with the current gate. If `cargo-modules` only prints the module tree and
cannot prove dispatch reachability, abandon the swap rather than weakening the assertion.

### `console.rs` — 1,093 lines — KEEP

**What it actually does.** It is a read-only three-view TUI over Fleet's heterogeneous state. It
loads ledger rows, transcripts, scorecards, submissions, attestations, and artifacts; joins them
into tasks and agents; tracks provenance for each displayed value; refuses fabricated denominators
and overflow; states absences rather than converting them to zero; renders fleet/task/agent views;
and handles keyboard navigation and terminal restoration. Snapshot tests cover three widths.

**Existing components.** `ratatui` and `crossterm` already do the generic terminal work; the module
is already stitched on them. `serde_json` does parsing. No named candidate understands Fleet's
receipt/transcript join, source labels, absence semantics, or bounded verdicts. A generic dashboard
could render JSON, but would not reproduce this operator contract without moving roughly the same
code into queries/templates.

**Estimated saving and loss.** At most **0–40 lines** of local navigation or formatting helpers
might be absorbed by another widget crate. That is below the cost of another dependency. Replacing
the TUI with a generic JSON viewer would lose provenance labels, explicit reasons for absence,
task/agent correlation, denominator validation, and stable narrow-width behavior.

**Verdict: KEEP.** This is application-specific presentation built on existing UI libraries, not a
hand-written terminal framework. The larger concern is schema drift from parsing many files as
loose `Value`s; a library swap does not solve that.

### `mcp.rs` — 537 lines — KEEP

**What it actually does.** It serves an MCP endpoint over stdio; derives six tool schemas; derives a
tool manifest from a `<prefix>/**` lease; enforces lexical and canonical path confinement including
symlink escape checks; performs read/write/list; exposes impact and lesson metadata; reads ledger
tails; rejects unmanifested tools; and wraps every outcome in a Fleet receipt and MCP result.

**Existing components.** `rmcp`, `schemars`, and `tokio` already own protocol routing, schemas,
transport, and runtime. This module is therefore substantially stitched already. A standard MCP
filesystem server can provide allowed-root file operations, but it does not own Fleet's lease
manifest, ledger-backed outcome receipts, typed exits, or Fleet tools. Putting one behind a proxy
would likely add at least as much adapter code as it removes.

**Estimated saving and loss.** A path-capability crate might remove **0–20 lines** after adapters.
A wholesale filesystem-server swap could appear to remove 150–250 lines but would lose receipt-on-
refusal, manifest enforcement, tool-specific result envelopes, ledger access, or the single-process
local model. That deletion is not credible unless the replacement demonstrates all of them.

**Verdict: KEEP.** The generic MCP machinery is already delegated. Keep the small policy server
that remains. Also note that `impact` and `lesson_recall` currently return capability metadata, not
the underlying result; that is a product completeness issue, not evidence for a protocol rewrite.

### `agent.rs` — 378 lines — KEEP

**What it actually does.** It loads and validates `agents.toml`; exposes registry metadata; loads,
validates, and persists per-agent scorecards with checked arithmetic and explicit denominators; and
defines a compile-time assignment flow (`Idle` through adjudication/credit/fault/amendment) using
consuming typestate transitions.

The adversarial finding is that the assignment typestate is not used by the production dispatcher.
Production uses the registry/listing path, while the assignment flow currently supplies a type
model and compile-time example. There is also a second, richer typestate in `lifecycle.rs`. That
duplication should be resolved before optimizing either implementation.

**Existing components.** `typestate 0.9.0-rc2` could generate marker states and transitions.
`serde`/`toml` already own data decoding. `statig` and `sm` are runtime state-machine approaches and
do not match the compile-time consuming API required by the acceptance contract.

**Estimated saving and loss.** A proc-macro DSL might delete **80–120 lines** of transition
boilerplate, but would add a release-candidate macro dependency and could obscure or fail the
required `PhantomData`, `Task<S>`, consuming-transition, and compile-fail evidence. It would not
replace registry validation, scorecards, fixed counts, or persistence. Because the flow is not
wired into runtime, those savings are not counted as plausible now.

**Verdict: KEEP.** First decide whether this assignment machine should be merged with or removed in
favor of `lifecycle.rs` under the lead-authored contract. Do not adopt a macro to polish detached
code. If it becomes the runtime model, reevaluate `typestate` only with compile-fail and generated-
API inspection.

### `lifecycle.rs` — 352 lines — KEEP

**What it actually does.** It defines 15 sealed lifecycle state types, a non-empty `TaskId`, typed
refusals, human-approval evidence that only the crate can mint, a narrow receipt-ledger capability,
and consuming transitions that refuse empty evidence and append a receipt before advancing. Tests
exercise the full 14-edge path and failure without advancement. External compile-fail fixtures
prove illegal edges, constructed states, non-human approval, and reuse of consumed tasks fail to
compile.

The production CLI does not currently drive this `Task<S>`; it is a library contract enforced by
tests. That gap matters more than its line count.

**Existing components.** `typestate 0.9.0-rc2` is the closest candidate for generating typestate
boilerplate. `statig 0.4.1` is a hierarchical runtime state machine and `sm 0.9` is also oriented
toward runtime state transitions; either would sacrifice the compile-time impossibility guarantee
unless wrapped in another typestate layer, erasing the saving.

**Estimated saving and loss.** `typestate` might remove **70–110 lines** of marker/impl boilerplate.
It cannot own human approval, required non-empty evidence, receipt-before-transition, or the sealed
ledger capability. The lead-authored acceptance suite also explicitly inspects source shape and
compile-fail behavior, so generated internals are not automatically equivalent. Savings are not
counted until the runtime-wiring gap is resolved.

**Verdict: KEEP.** The manual version is small, explicit, and aligned to a security-relevant type
contract. `statig`/`sm` are bad trades here. A future typestate-macro trial must prove all existing
compile-fail fixtures still fail and the legal 14-edge path writes exactly 14 receipts before it can
be considered.

### `swarm.rs` — 231 lines — KEEP

**What it actually does.** It seeds one JSON status record per registry agent, reloads persistent
per-agent status, validates non-empty identity/role/task and one of 15 lifecycle names, rejects zero
or one agent, rejects duplicates, and reports environment faults separately from invariant faults.
Its tests prove seeding, persistence, zero-agent refusal, and the “one agent is not a swarm” rule.

**Existing components.** `serde_json` already owns serialization. A validation crate could express
some field predicates, and a state-machine crate could type a live state, but neither owns atomic
per-agent seeding, registry coupling, the two-agent minimum, or Fleet's error taxonomy. `statig` and
`sm` would be especially misleading because this file stores snapshots; it does not execute the
lifecycle graph.

**Estimated saving and loss.** A validator derive might remove **0–20 lines** while adding a
dependency and still needing custom error mapping. A generic actor/swarm framework would add
network/runtime assumptions explicitly outside this local product's scope and lose the simple
inspectable JSON state.

**Verdict: KEEP.** It is a small persistence adapter around already-used serde, with product rules
dominating the code.

### `roles.rs` — 148 lines — KEEP

**What it actually does.** It defines five roles, their owned gates and implementation-write
permission; prints the role matrix; and enforces two separation rules: a lead cannot add code and a
verifier cannot use the builder's model. Unit tests cover both accept and refuse arms.

**Existing components.** Rego/conftest could express the two Boolean rules, and the existing
architecture policy is a possible home. It would not replace typed role parsing or the printed
role catalog. Calling an external policy engine for two comparisons also introduces an environment
failure mode into a currently deterministic operation.

**Estimated saving and loss.** Moving only `evaluate` to Rego removes perhaps **20–35 Rust lines**
and adds input assembly, process invocation, result mapping, and fixtures—probably a net increase.
It risks splitting the role catalog from its policy and must still append the refusal receipt in
`main.rs`.

**Verdict: KEEP.** Policy-as-data is valuable when a policy is substantial or changes independently.
At this size, the Rust table is the clearer single source of truth. If role policy grows, move the
whole catalog and rules together rather than only two `if` statements.

## Recommended order by lines saved per hour of risk

These are risk-adjusted trial estimates, not delivery promises. “Hours” includes fixture creation,
behavior comparison, and rollback—not just typing the replacement.

| Order | Trial | Expected deletion | Risk effort | Lines saved per risk-hour | Adoption condition |
|---:|---|---:|---:|---:|---|
| 1 | `cargo-modules` for **test-only** module reachability | 170–220 | 6–10 h | 17–37 | Must fail an unreachable-module bad control and an empty denominator; otherwise abandon |
| 2 | `clap` derive + `clap_complete` for CLI surface | 300–450 | 16–24 h | 13–28 | Full good/bad CLI matrix preserves exits, streams, and receipt deltas |
| 3 | Rego/conftest for ratchet decision policy | 350–500 | 40–60 h | 6–13 | Zero divergence in fixture matrix and shadow scorecards; bad controls mandatory |

The first trial is deliberately disposable: `cargo-modules` may expose structure without proving
reachability from `dispatch`. If so its deletion is **zero**, and it should not be defended or wired
in. The second should migrate leaf subcommands incrementally so failures identify one grammar edge.
The third goes last because this repository has direct evidence that a policy engine can load zero
effective rules and still look green.

`cargo-machete` and `cargo-udeps` are worthwhile separate dependency-hygiene experiments, but they
are not in this ranking: neither replaces current code, so their lines-saved numerator is zero.
Likewise, do not spend time trialing `statig` or `sm` for the current lifecycle; they solve the wrong
kind of state machine.

## Honest deletion ceiling and hand-written floor

The credible deletion ceiling is **1,170 of 8,181 lines**. A central planning number is about
**1,000 lines**, leaving roughly **7,181**. The honest floor under the candidates reviewed is
therefore approximately **7,000 hand-written Rust lines** (conservatively **6,900–7,360** after the
estimated swaps), not hundreds.

Of that retained floor, at least **about 4,000 lines** directly implement or test Fleet-specific
trust/refusal behavior that no named replacement owns:

* worker isolation and the fd-3-only submission channel, including sanitized environment and
  parent-owned timestamp/actor/model fields;
* process-group deadlines and typed separation of environment fault, invariant violation, refusal,
  and verification mismatch;
* immutable artifact freeze, blind-suite isolation measurements, rollback rehearsal, two-oracle
  independence, and attestation recomputation;
* lock-serialized receipt append, canonical BLAKE3 chain verification, refusal-before-exit, and
  non-zero `{checked,total}` verdicts;
* narrow, expiring ratchet exceptions and atomic mark advancement;
* lease-confined MCP paths with a receipt for success and refusal;
* compile-time lifecycle edges, human-only approvals, per-agent persistence, and role separation;
* the console's rule that absence remains absence and never becomes a fabricated zero.

The remaining retained lines are custom cross-language impact indexing, operator presentation, and
adapters for those invariants. They are less sacred, but none of the concrete candidates replaces
them today. The correct USP interpretation is therefore: **stitch commodity mechanisms aggressively,
but keep the small Fleet-specific policy and provenance kernel explicit.** Today the commodity
surface is too hand-written; after the three proven partial swaps, the large remaining Rust body is
evidence of product specificity rather than failure to search crates.io.


---

## Correction, 2026-08-24 — the `main.rs` estimate was wrong, and the swap was rejected

This audit estimated **300–450 lines** deletable from `main.rs` by moving the hand-rolled command
grammar to `clap` derive. The swap was implemented and measured:

    before 3,096 lines · after 3,095 lines · NET DELETED: 1
    diff: 445 removed, 444 declarative/adapter lines added

**The estimate was wrong by roughly two orders of magnitude.** A complete `clap` derive grammar for
this many subcommands, with the exit-code mapping the public contract requires (clap exits 2 on a
bad flag; fleet's contract is 7), is very nearly as long as the parsing it replaces.

**Merge rejected.** The stated goal was least self-written code measured in lines deleted, and 889
lines of churn through the dispatch path — the most safety-critical file in the tree — is a bad
trade for one line. The tree is green and there is higher-value work.

**The lane did exactly what was asked and reported honestly**, including that its own result was
"substantially below STITCH's estimate". That report is why this correction exists; a lane that had
quietly declared success would have bought us the churn.

**What this says about the rest of the audit.** The remaining estimates — `ratchet.rs` 350–500,
`graph.rs` 170–220 — were produced the same way and should be treated as unvalidated until one is
measured. The audit's *verdicts* (which modules are PARTIAL vs KEEP, and that no library owns fd-3
provenance, the freeze, or the ledger chain) still stand; its *line estimates* now have one
data point and it was off by 400x.

Verified before rejecting: build clean, clippy clean, swarm 36/36, exit-code contract held
(`fleet run --nonsense-flag` → rc 7, not clap's 2), and p0 33/34 with the single failure being an
unbuilt `b3oracle` in the worktree — B4 correctly refusing to pass when it cannot measure.
