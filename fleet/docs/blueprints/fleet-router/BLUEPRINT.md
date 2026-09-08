# BLUEPRINT — `fleet-router`

## 1. Header

- **Crate:** `fleet-router`
- **One-line purpose:** Given a role, a task class, and a snapshot of what's currently available,
  deterministically decide which adapter/model gets a task, staged through 6 auditable filters, or
  produce a typed refusal naming the stage and the fix — with zero IO of its own.
- **Build branch:** `extract` (MIGRATION-PLAN §3 row 4) — the decision core in
  `fleet/keel/fleet/src/route.rs` is a near-exact match. **Caveat (read §5/§notes): only part of
  `route.rs` is actually pure/IO-free; the extract must split the file, not lift it whole. See the
  "Divergence from MIGRATION-PLAN" note at the end of this file.**
- **Imports:** `fleet-types` (the `Role` enum and its tier/gate metadata — see divergence note;
  today `Role` lives in fleet's `roles.rs`, which MIGRATION-PLAN's row 1 evidence does not cite).
- **Imported by:** `fleet-govern` (owns quota/cooldown measurement — item 11, "route.rs failover" —
  and is the caller that assembles `RuntimeState` from live probes and calls `decide`),
  `fleet-worker` (reads `Decision.selected_adapter`/`resolved_model` to dispatch), `src/`
  (composition root — CLI arg parsing, JSON/human printing, receipt-writing on refusal).

## 2. Responsibility & non-goals

**Owns:** the committed candidate table (`ORDER`), the deterministic 6-stage filter pipeline
(explicit role → safety policy → local capability → availability/quota → verifier independence →
deterministic pick), and the typed `Decision`/`Refusal` shape that makes every stage's surviving
candidate set auditable. Given the same `RuntimeState`, `decide()` returns the same `Decision` on
every call, forever — this is the property every caller and every test leans on.

**Non-goals (the seam):**
- Does **not** probe which CLIs are installed or authenticated (`which codex`, `codex --version`,
  the `crew.adapters.capability.probe_adapter` python shell-out) — that's a capability-probing
  concern the caller (`fleet-govern`, today's `runtime()`/`installed_non_interactive`/
  `adapter_contract` in route.rs) owns and injects as an already-computed `RuntimeState.capable` set.
- Does **not** read token quota, reservations, or cooldowns from disk or environment
  (`FLEET_ROUTE_COOLDOWNS`, the meter state file) — that's `fleet-govern`/`fleet-store`'s job
  (today's `crate::meter::planning_snapshot_at` and `env::var` calls in route.rs's `runtime()`).
- Does **not** write receipts/ledger entries on refusal — that's `fleet-store`'s job (today's
  `crate::append_receipt` call in route.rs's `command()`).
- Does **not** parse CLI args or print human/JSON output — that's the composition root's job
  (today's `command()`, `print_human()`, `invalid_args()` in route.rs).
- Does **not** call an LLM, spawn any process, or touch the filesystem or network under any
  circumstance. If a future change needs any of these inside this crate, that is itself a defect —
  push it to the caller instead.

## 3. Public API contract

```rust
//! Deterministic, side-effect-free role -> model routing decision.
//!
//! This crate answers exactly one question -- "given a role, a task class, and a snapshot of
//! what is currently available, which adapter/model gets this task, or why not" -- and answers it
//! identically every time for the same inputs. It performs no IO, spawns no process, reads no
//! clock, and reads no environment variable: every fact about the world (installed adapters,
//! quota remaining, cooldowns, the committed preference order) arrives through `RuntimeState`,
//! which the caller builds from its own probes.

use std::collections::{BTreeMap, BTreeSet};
use fleet_types::Role;

/// One entry in the committed, order-sensitive candidate table. Order changes are a policy
/// change and require review (see `ORDER`'s doc comment for the D50 history this preserves).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateSpec {
    /// Stable identity used in `Decision`, `RuntimeState.preference`, and receipts. Never renamed
    /// once shipped -- receipts and dashboards key on it.
    pub id: &'static str,
    /// The adapter family this candidate resolves through (`"claude"`, `"codex"`, `"freelane"`).
    /// Multiple candidates may share an adapter (e.g. opus/sonnet/haiku all resolve via "claude").
    pub adapter: &'static str,
    /// The model alias as requested of the adapter.
    pub requested: &'static str,
    /// The model identity actually used once resolved -- may differ from `requested` (aliases).
    pub resolved: &'static str,
    pub tier: Tier,
}

/// The tier a role maps to. `role_allows` is the only place tier <-> role is decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier {
    Lead,
    Worker,
    Cheap,
}

/// The committed candidate order and the deterministic tie-breaker. Stage 6 walks this table
/// top-to-bottom and picks the first id that both survived every prior filter AND appears in
/// `RuntimeState.preference` -- so changing this table's order changes routing outcomes;
/// changing it is a reviewed policy change, not a code change like any other.
///
/// `freelane` sits last deliberately: it is the keyless subprocess fallback, preferred only when
/// every authenticated CLI is unavailable or out of quota, never over them.
pub const ORDER: &[CandidateSpec] = &[
    CandidateSpec { id: "codex",    adapter: "codex",    requested: "codex-worker",    resolved: "codex",  tier: Tier::Worker },
    CandidateSpec { id: "sonnet",   adapter: "claude",   requested: "claude-sonnet",   resolved: "sonnet", tier: Tier::Worker },
    CandidateSpec { id: "opus",     adapter: "claude",   requested: "opus",            resolved: "opus",   tier: Tier::Lead },
    CandidateSpec { id: "haiku",    adapter: "claude",   requested: "haiku",           resolved: "haiku",  tier: Tier::Cheap },
    CandidateSpec { id: "freelane", adapter: "freelane", requested: "codestral-latest", resolved: "codestral-latest", tier: Tier::Worker },
];

/// Coarse task classification the safety-policy stage (stage 2) reasons over.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskClass {
    General,
    Implementation,
    HumanOnly,
}

/// A safety-gate check, mirroring fleet's `roles::Check`/`roles::evaluate`. Lifted into this
/// crate (not `fleet-types`) because today nothing outside routing consumes it -- see the
/// divergence note if a second consumer appears later, at which point it moves to `fleet-types`.
#[derive(Debug, Eq, PartialEq)]
pub struct RoleCheck<'a> {
    pub role: Role,
    pub diff_adds_code: bool,
    pub builder_model: Option<&'a str>,
    pub verifier_model: Option<&'a str>,
}

/// Why a `RoleCheck` failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoleRefusal {
    /// A Lead-role check saw a diff that adds code. Leads architect; they do not implement.
    LeadWroteCode,
    /// A Verifier's resolved model equals the Builder's resolved model -- a model cannot verify
    /// its own work.
    SelfVerified,
}

impl RoleRefusal {
    pub fn reason(self) -> &'static str {
        match self {
            Self::LeadWroteCode => "LEAD_WROTE_CODE",
            Self::SelfVerified => "SELF_VERIFIED",
        }
    }
}

/// Evaluate one role-gate check. Pure, total, never panics.
pub fn evaluate_role_check(check: &RoleCheck<'_>) -> Result<(), RoleRefusal> {
    if check.role == Role::Lead && check.diff_adds_code {
        return Err(RoleRefusal::LeadWroteCode);
    }
    if check.role == Role::Verifier {
        if let (Some(builder), Some(verifier)) = (check.builder_model, check.verifier_model) {
            if builder == verifier {
                return Err(RoleRefusal::SelfVerified);
            }
        }
    }
    Ok(())
}

/// Everything `decide` needs to know about the outside world, computed and owned by the caller.
/// This crate never mutates or re-derives any field -- it only filters `ORDER` against them.
#[derive(Clone, Debug)]
pub struct RuntimeState {
    /// Adapter families ("claude", "codex", "freelane", ...) confirmed installed AND
    /// non-interactively usable AND passing the builder/verifier capability contract for this
    /// call. Computed by the caller; this crate does not know how.
    pub capable: BTreeSet<&'static str>,
    /// Remaining token budget per adapter family. `None` = "no measured window" (treated as not
    /// usable, never as unlimited or as zero). Absent key = not measured at all.
    pub remaining: BTreeMap<String, Option<u64>>,
    /// Adapter families currently in a cooldown window (measured failure backoff).
    pub cooldown: BTreeSet<String>,
    /// Token budget this task is estimated to need; a candidate needs `remaining >= required_tokens`.
    pub required_tokens: u64,
    /// The committed preference order for the deterministic tie-breaker (stage 6). In production
    /// this is always `ORDER.iter().map(|c| c.id).collect()`; tests may shrink or reorder it to
    /// exercise the tie-breaker directly.
    pub preference: Vec<&'static str>,
}

/// One stage of the 6-stage pipeline, after filtering: how many candidates survived, out of the
/// total that started, and their ids -- the auditable trail.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Stage {
    pub number: usize,
    pub name: &'static str,
    pub checked: usize,
    pub total: usize,
    pub candidates: Vec<&'static str>,
}

/// Why routing refused to select anyone. Always names the stage that first emptied out, so the
/// operator's fix is specific ("stage 4: no lane has quota") not generic ("routing failed").
#[derive(Clone, Debug, serde::Serialize)]
pub struct Refusal {
    pub stage: usize,
    pub stage_name: &'static str,
    pub reason: String,
    pub fix: String,
}

/// The result of `decide`. `refusal.is_none()` iff a candidate was selected; the two are mutually
/// exclusive and jointly exhaustive -- never both `None`/`None` and never both `Some`/`Some`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Decision {
    pub role: &'static str,
    pub stages: Vec<Stage>,
    pub selected_adapter: Option<&'static str>,
    pub requested_model: Option<&'static str>,
    pub resolved_model: Option<&'static str>,
    pub decided_at_stage: Option<usize>,
    pub refusal: Option<Refusal>,
}

/// Run the full 6-stage pipeline once. Pure and total: never panics, never blocks, never
/// allocates unboundedly (bounded by `ORDER.len()`, a compile-time constant). Deterministic:
/// identical `(role, class, builder_resolved_model, runtime)` always yields an identical
/// `Decision`, including field order inside `stages`.
///
/// Stages, in order: (1) explicit role -- filter `ORDER` to the role's tier; empty if `role` is
/// `None` or the role's tier has no candidates. (2) safety policy -- refuse `TaskClass::HumanOnly`
/// outright; refuse a Lead role on `TaskClass::Implementation` via `evaluate_role_check`. (3) local
/// capability -- keep only candidates whose adapter is in `runtime.capable`. (4)
/// availability/quota -- drop candidates whose adapter is cooling down or lacks
/// `>= runtime.required_tokens` remaining (an unmeasured `None` window is never treated as
/// available). (5) verifier independence -- if `role == Verifier`, `builder_resolved_model` must
/// be `Some` and each surviving candidate's resolved model must pass `evaluate_role_check` against
/// it (mismatched-empty-set semantics: `None` builder model clears all candidates, it does not
/// skip the stage). (6) deterministic pick -- walk `runtime.preference` in order (which is itself
/// walked in `ORDER`'s order when built from `ORDER` directly) and take the first id both listed
/// there and still alive after stage 5.
pub fn decide(
    role: Option<Role>,
    class: TaskClass,
    builder_resolved_model: Option<&str>,
    runtime: &RuntimeState,
) -> Decision {
    unimplemented!("see fleet/keel/fleet/src/route.rs:136-278 `decide` for the reference body; \
                    port verbatim, replacing `roles::evaluate`/`Role`/`Check` with this crate's \
                    `evaluate_role_check`/`Role`/`RoleCheck`")
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `CandidateSpec` | All fields are `'static` data from the compiled-in `ORDER` table; never constructed at runtime by a caller. | A candidate that isn't part of the reviewed, committed policy table cannot enter the pipeline. |
| `Tier` | Exactly 3 variants, exhaustively matched in `role_allows` (private helper, not shown above — one `match` arm per `Role` variant, no wildcard `_ => ...` arm). | A role silently mapping to "no tier" or "every tier" — the compiler forces every `Role` to have an explicit tier mapping. |
| `RuntimeState.remaining: BTreeMap<String, Option<u64>>` | `None` and "key absent" are both distinct from `Some(0)` and both treated as "not usable" (never "unlimited"). | A lane with unmeasured quota being silently routed to as if it had infinite budget. |
| `Decision` | `refusal.is_some() == selected_adapter.is_none() == requested_model.is_none() == resolved_model.is_none() == decided_at_stage.is_none()`, always in lockstep — enforce this with a unit test, not just by convention (§9). | A caller reading a `Decision` and seeing a "selected" adapter alongside a live refusal, or a refusal-free `Decision` with no selection. |
| `Refusal` | `stage` is always the 1-based index of the first pipeline stage whose surviving-candidate set became empty (or, for stage 6, whose tie-breaker found nothing) — never a later stage once an earlier one already emptied. | An operator being told to fix stage 4 (quota) when the real block was stage 2 (safety policy) — the "first empty wins" rule in `decide` (`first_empty.get_or_insert(...)`, never overwritten) is exactly this. |

**Money/precision:** no money type in this crate. Token counts (`required_tokens`, `remaining`)
are `u64`, never float — a partial token is meaningless and must never be representable.

**Clock/RNG/IO injection points:** none — this crate touches none of the three. `decide` takes no
clock (routing has no time-dependent behavior) and no RNG (routing is deterministic by design, not
merely by convention — see §9's `deterministic_across_runs` test). If a future requirement needs
either, it is itself a change to this crate's non-goals and must be renegotiated, not silently added.

## 5. Reuse map

Source: `fleet/keel/fleet/src/route.rs` (687 lines, read in full 2026-09-08) and
`fleet/keel/fleet/src/roles.rs` (168 lines).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `route.rs:16-56` (`ORDER`) | The committed candidate table + D50 tie-breaker history comment. | yes | Make `CandidateSpec` fields `pub` (were private); keep the D50 comment verbatim — it is load-bearing institutional memory, not decoration. |
| `route.rs:58-63` (`Tier`) | 3-variant tier enum. | yes | `pub` instead of `pub(crate)`. |
| `route.rs:65-72` (`CandidateSpec`) | Candidate data struct. | yes | `pub` fields. |
| `route.rs:74-79` (`TaskClass`) | 3-variant task classification. | yes | `pub` instead of `pub(crate)`. |
| `route.rs:81-88` (`RuntimeState`) | Caller-supplied world snapshot. | yes | `pub` fields; this is already the exact seam the crate boundary should sit on — route.rs already separated "what decide() needs" from "how to compute it," it just did both in one file. |
| `route.rs:90-116` (`Stage`, `Decision`, `Refusal`) | Output shape, already `Serialize`. | yes | `pub` instead of `pub(crate)`; no logic change. |
| `route.rs:118-124` (`role_allows`) | Role→tier eligibility match. | yes | Keep private; only `decide` calls it. |
| `route.rs:126-134` (`stage`) | Builds one `Stage` record. | yes | Keep private helper. |
| `route.rs:136-278` (`decide`) | The full 6-stage pipeline — this is the crate's entire reason to exist. | yes, logic verbatim | Replace `roles::evaluate`/`Role`/`Check` calls with this crate's `evaluate_role_check`/`RoleCheck` (§3); replace `crate::meter::PlanningSnapshot` references (none directly in `decide`, only in the excluded `for_plan`) — none needed. |
| `roles.rs:1-70` (`Role` enum + `parse`/`name`/`bandwidth`/`allocation_fitness`/`owned_gate`/`may_write_code`) | Role identity + scheduling metadata (`bandwidth`, `allocation_fitness` are used by fleet's allocator, not by routing). | **no — belongs in `fleet-types`, not here** | `fleet-router` imports `Role` from `fleet-types`; do not re-lift the whole enum here. This is a gap in MIGRATION-PLAN's row 1 evidence (see divergence note). |
| `roles.rs:83-110` (`Refusal`, `evaluate`) | The two safety-gate rules (`LEAD_WROTE_CODE`, `SELF_VERIFIED`). | yes, renamed | Lift into this crate as `RoleRefusal`/`evaluate_role_check` (§3) — nothing outside routing calls this today, so it does not warrant living in `fleet-types` yet. Revisit if a second caller appears. |
| `route.rs:280-304` (`adapter_contract`) | Shells out to `python3 -c` importing `crew.adapters`, probing `builder_eligible`/`verifier_eligible`. | **no** | IO/subprocess — excluded per §2. Lives in the caller (`fleet-govern`) as capability-probing that produces `RuntimeState.capable`. |
| `route.rs:306-314` (`installed_non_interactive`) | Shells out to `<adapter> --version`. | **no** | Same as above — excluded, caller's job. |
| `route.rs:316-346` (`runtime`) | Assembles `RuntimeState` from `installed_non_interactive`, `adapter_contract`, `env::var("FLEET_ROUTE_COOLDOWNS")`, a freelane-script file-existence check, and the meter snapshot. | **no** | This is the exact function that must move to the caller — it is 100% IO, zero decision logic. Its *shape* (what fields it populates) becomes this crate's `RuntimeState` contract. |
| `route.rs:348-373` (`for_plan`, `for_plan_with_builder`) | Reads meter state from disk (`crate::meter::planning_snapshot_at`), builds `RuntimeState`, calls `decide`. | **no** | Composition/`fleet-govern` glue — calls into this crate's `decide`, isn't part of it. |
| `route.rs:375-400` (`print_human`) | Formats a `Decision` for terminal output. | **no** | Presentation — `src/` (composition root). |
| `route.rs:402-473` (`command`) | CLI arg parsing, dispatch, `crate::append_receipt` on refusal. | **no** | CLI + ledger-write — `src/` + `fleet-store`. |
| `route.rs:475-485` (`invalid_args`) | Usage message + receipt on bad args. | **no** | Same as above. |
| `route.rs:487-687` (`#[cfg(test)] mod tests`) | 4 tests: every-stage-names-its-empty-set, verifier bidirectional identity, lead/human-only refusals, determinism-across-runs. | as inspiration, not verbatim | Port the *assertions* into this crate's test plan (§9); the test bodies construct `RuntimeState`/`Role` inline exactly as this crate's tests should. |

## 6. Behavior spec

### `fn decide(role: Option<Role>, class: TaskClass, builder_resolved_model: Option<&str>, runtime: &RuntimeState) -> Decision`

| Input dimension | Behavior |
|---|---|
| empty | `role = None` → stage 1 filters `ORDER` with `.unwrap_or_default()`, yielding an empty candidate set immediately; refusal at stage 1, reason `"role has no eligible tier set"`. `runtime.capable`/`.remaining`/`.preference` empty → refusal at whichever stage first depends on them (3, 4, or 6 respectively) with that stage's named reason — never a panic or an empty `Decision` with no refusal. |
| null / `None` | `builder_resolved_model: None` while `role == Some(Role::Verifier)` → stage 5 clears all candidates (`candidates.clear()`), refusal `"no verifier with a resolved model distinct from the builder remains"` — a missing builder model is treated as "cannot verify," never as "skip the independence check." |
| wrong-type | Not reachable inside this crate — `role`/`class` are enums the caller must already have constructed correctly; `Role::parse`/CLI-string parsing happens in `fleet-types`/`src` before `decide` is ever called. No stringly-typed input crosses this boundary. |
| huge | `runtime.remaining`/`cooldown`/`preference` with thousands of entries: stage 3/4/6 are simple linear filters/lookups over `ORDER` (fixed at 5 entries today, bounded at compile time) — cost is `O(|ORDER| * cost of runtime lookups)`, not `O(runtime size)` beyond the `BTreeMap`/`BTreeSet` lookup cost; no unbounded allocation regardless of `runtime`'s size. `required_tokens = u64::MAX` → every candidate's `remaining >= required_tokens` check fails unless a lane reports `Some(u64::MAX)` too; refuses at stage 4, does not overflow (`checked_sub` already guards the underlying subtraction in the caller's `runtime()`, not here — this crate only compares, never subtracts). |
| negative | Not representable — `required_tokens: u64` and all token counts are unsigned; a caller cannot construct a negative token count. |
| duplicate | Two `CandidateSpec`s sharing an `id` in `ORDER` would break `Stage.candidates`/`Decision` uniqueness assumptions — this is a static policy-table defect, not a runtime input; guard it with a `const`-time or unit-test assertion (`ORDER` ids are pairwise distinct) rather than runtime logic, since `ORDER` is fixed and compiled in. |
| concurrent | `decide` takes `&RuntimeState` (shared, read-only) and returns an owned `Decision` — no interior mutability, no shared writable state, so concurrent calls from multiple threads with distinct or shared `RuntimeState` values are trivially safe and independent (see §11 thread-safety). |
| unicode / non-ASCII | `builder_resolved_model: Option<&str>` compared to `candidate.resolved` by `==` (byte/UTF-8 equality) — a unicode builder-model string that doesn't byte-match any candidate's `&'static str` simply never matches; no panic, no normalization performed (model identifiers are ASCII in practice; if that ever changes, equality is still well-defined, just not Unicode-normalized — documented here as a known limit, not a silent bug). |
| already-exists | Calling `decide` twice with identical inputs is not an error — it is required to be idempotent (§9 `deterministic_across_runs`). There is no "already decided" state to collide with. |
| partial-failure | `decide` cannot fail partially — it has no IO, so it either fully filters through all 6 stages and returns a complete `Decision`, or (impossible in practice, since the fn has no early `return` before line-for-line completing every stage) it never returns at all (this crate performs no panics, no `?`, no early-return paths — every stage always executes and appends its `Stage` record, verified by `stages.len() == 6` always holding). |

### `fn evaluate_role_check(check: &RoleCheck<'_>) -> Result<(), RoleRefusal>`

| Input dimension | Behavior |
|---|---|
| empty | `builder_model: None, verifier_model: None` on a `Verifier` check → the `if let (Some, Some)` guard fails to match, so the self-verification check is skipped and the fn returns `Ok(())` — a verifier check with no models named is never treated as an error by this fn alone (callers needing "must have both" enforce that separately, as `decide` stage 5 does via `candidates.clear()` on `None`). |
| null / `None` | Same as empty above. |
| wrong-type | n/a — `Role` is an enum, `bool`/`Option<&str>` are the only other params; no stringly-typed input. |
| huge | n/a — no collections, O(1) always. |
| negative | n/a — no numeric input. |
| duplicate | `builder_model == verifier_model` (same string, "duplicate" identity) on a `Verifier` check → `Err(RoleRefusal::SelfVerified)`, exactly the case this fn exists to catch. |
| concurrent | Pure value fn, no shared state — trivially safe. |
| unicode / non-ASCII | Byte-equality comparison, same as `decide`'s note above; a unicode model string compares correctly by `==`, just without normalization. |
| already-exists | n/a — no persisted state. |
| partial-failure | n/a — no IO, cannot fail partially; always returns `Ok`/`Err` synchronously. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `serde` | `1.0.229` | `Stage`/`Decision`/`Refusal` derive `Serialize` for the `--json` output and for receipts. |
| `serde_json` | `1.0.151` (dev-dependency only — nothing in `src/` builds or parses JSON; only the crate's own tests/doctests would assert on shape) | Not currently used by any test, but kept available as a dev-dependency for JSON-shape assertions; the composition root does the actual `to_string_pretty` call today (route.rs:456) — that call lives in `src/`, not here. |
| `fleet-types` | workspace-path dependency (`{ path = "../fleet-types" }`) | Supplies `Role`. |

No other crate is needed — this is deliberately the leanest crate in the roster (pure logic, no
async runtime, no serialization framework beyond `serde` derive, no error-handling crate: the two
error enums here are hand-rolled and total).

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** One responsibility per file; `lib.rs` is a thin hub.
> `decide()` alone is ~142 lines in route.rs (`route.rs:136-278`), so it MUST be split — the
> orchestration in `decide.rs`, the per-stage logic in `stage.rs`, the data types in `types.rs`.

```
crates/fleet-router/
  Cargo.toml
  src/
    lib.rs         # 21 — module decls + re-exports only
    table.rs       # 67 — ORDER, CandidateSpec, Tier, TaskClass (+ order_ids_are_pairwise_distinct unit test)
    role_check.rs  # 71 — RoleCheck, RoleRefusal, evaluate_role_check (+ its own unit tests)
    types.rs       # 60 — RuntimeState, Stage, Decision, Refusal (data only, derives serde)
    allow.rs       # 31 — role_allows() (+ role_allows_covers_every_role_exhaustively unit test)
    stage.rs       # 69 — stage() builder + stages 1/3/4 filters (role, capability, quota) and their refusals
    verify_gate.rs # 63 — stages 2/5: safety_refusal() and filter_verifier()/refusal_verifier()
    pick.rs        # 24 — stage 6: pick() deterministic tie-breaker + refusal_pick()
    decide.rs      # 70 — decide() orchestration wiring table + stage + verify_gate + pick
  tests/
    common/
      mod.rs              # 38 — shared RuntimeState fixtures + assert_mutually_exclusive()
    decide_happy.rs        # 51 — happy-path resolution per §9
    decide_refusals.rs     # 54 — refusal + verifier-independence cases per §9
    decide_mutants.rs      # 36 — every_reachable_candidate_is_reachable (D50 regression) per §9
```
> All source files are currently within the 80-line rule (largest is `role_check.rs` at 71). If any
> file above grows over 80 lines, split it (e.g. `stage.rs` → `stage_filter.rs` + `stage_score.rs`).
> The file-size gate (§10) is run before Opus review.

`Cargo.toml` (actual):
```toml
[package]
name = "fleet-router"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0.229", features = ["derive"] }
fleet-types = { path = "../fleet-types" }

[dev-dependencies]
serde_json = "1.0.151"
```

## 9. Test plan

**Unit tests** (colocated `#[cfg(test)] mod tests` in each source file):
- `order_ids_are_pairwise_distinct` (in `table.rs`) — asserts no two `ORDER` entries share `id`
  (guards the "duplicate" cell in §6 statically, at test time rather than at compile time).
- `role_allows_covers_every_role_exhaustively` (in `allow.rs`) — asserts `role_allows` compiles
  without a wildcard arm (enforced by the `match` itself — this test documents the intent by
  calling it for all `Role::ALL` variants and asserting each returns a bool without panicking).
- `lead_code_gate_covers_both_directions` (in `role_check.rs`) — `evaluate_role_check` on
  `(Lead, diff_adds_code: true)` is `Err(LeadWroteCode)`; on `(Lead, diff_adds_code: false)` is
  `Ok(())`.
- `verifier_self_check_covers_both_directions` (in `role_check.rs`) — same builder/verifier model
  → `Err(SelfVerified)`; distinct models → `Ok(())`.

**Integration tests** (`tests/decide_happy.rs`, `tests/decide_refusals.rs`, `tests/decide_mutants.rs`,
sharing fixtures from `tests/common/mod.rs`, calling only the public API):
- `every_filter_stage_names_its_empty_set` (in `decide_refusals.rs`) — drive `decide` into a
  refusal at each of stages 1/2/3/4/5/6 in turn (no role; Lead+Implementation; empty `capable`;
  empty `remaining`; single-adapter Verifier with no distinct model; empty `preference`) and assert
  `refusal.unwrap().stage` matches.
- `decision_refusal_and_selection_are_mutually_exclusive` (in `decide_refusals.rs`, via the shared
  `assert_mutually_exclusive` helper) — for a table of representative `RuntimeState`s (empty, full)
  and every `Role` variant, assert `decision.refusal.is_some() == decision.selected_adapter.is_none()`
  (and likewise for `requested_model`/`resolved_model`/`decided_at_stage`) always holds — the
  invariant from §4.
- `verifier_compares_resolved_identity_in_both_directions` (in `decide_happy.rs`) — a codex
  builder resolves the verifier to sonnet; a `"codex-worker"` builder (codex's *requested* alias,
  not its resolved identity) resolves it to codex (i.e. the comparison is against `resolved`, not
  `requested`).
- `lead_implementation_refuses_and_lead_general_routes_to_opus` (in `decide_happy.rs`).
- `deterministic_across_runs_and_both_availability_directions` (in `decide_happy.rs`) — 20
  repeated calls with identical input yield an identical `resolved_model`; cooling down codex
  shifts the pick to sonnet; the two are independently verified (not just "it changed").

**Mutation-testing targets** (`cargo mutants -p fleet-router`):
- Flipping `>=` to `>` in the stage-4 quota check (`remaining >= runtime.required_tokens`) must be
  killed by `every_filter_stage_names_its_empty_set`'s stage-4 case (an exact-match quota boundary).
- Flipping `first_empty.get_or_insert(...)` to unconditional overwrite (so a later stage's refusal
  clobbers an earlier one) must be killed by the dedicated `first_refusal_wins_over_later_stages`
  test (in `decide_refusals.rs`): construct a `RuntimeState` that empties both stage 3 and stage 4,
  assert `refusal.stage == 3`.
- Deleting the `stages.push(stage(6, ...))` call (so `decision_refusal_and_selection_are_mutually_exclusive`'s `stages.len() == 6` check) must be killed by asserting `stages.len() == 6` in every integration test, not just once.
- Swapping `ORDER`'s ordering (moving `freelane` before `codex`) must be killed by
  `deterministic_across_runs_and_both_availability_directions`'s assertion that the full-runtime
  pick is specifically `"codex"`.

**Reachability regression** (`tests/decide_mutants.rs`, given the D50 history of a silently
unreachable candidate — built as a plain `for` loop over `ORDER`, not a `proptest` generator; this
crate takes no `proptest` dependency):
- `every_reachable_candidate_is_reachable`: for each `id` in `ORDER`, construct a `RuntimeState`
  making exactly that candidate's adapter capable/quota'd/uncooled and every other adapter absent,
  and assert `decide` for a role whose tier contains that candidate selects it (both
  `selected_adapter` and `resolved_model`). This is the direct, generalized regression test for
  D50 (`freelane` being unconditionally unreachable) — it fails loudly the moment any future
  `ORDER` entry becomes structurally unreachable, at N = 5 cases today (one per current entry),
  growing automatically as `ORDER` grows.

## 10. Verification recipe

```bash
cd crates/fleet-router
cargo test -p fleet-router --all-targets
cargo clippy -p fleet-router --all-targets -- -D warnings
cargo mutants -p fleet-router
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration tests pass, 0 skipped — publish as `<passed>/<total>` (e.g.
`13/13`, not "tests pass"). Clippy: 0 warnings. Mutants: every target named in §9 caught; publish
`<caught>/<total mutants>` — this crate is small and pure enough that the floor should be **100%
of viable mutants caught**, not a partial-credit percentage; any survivor gets a new test, not a
lowered floor.

## 11. L8 checklist

- [x] Every fallible path returns a typed error enum (`RoleRefusal`) — `decide` itself is
      infallible by design (refusal is a data field, not an error), which is the correct shape for
      a pure decision fn: "no candidate survived" is an expected outcome, not an exceptional one.
- [x] Clock/RNG/IO are injected — trivially, by having none: `RuntimeState` is the sole input,
      supplied by the caller; this crate reads no ambient state at all.
- [x] Thread-safety documented: `decide`/`evaluate_role_check` take `&`-refs and return owned
      values with no interior mutability — `Send + Sync` for free, safe to call concurrently from
      any number of threads with any `RuntimeState` values, shared or distinct.
- [x] No float used for money, tokens, or any precision-sensitive count — `required_tokens`/
      `remaining` are `u64` throughout.
- [ ] No self-grading — verification runs `cargo mutants`, not just the crate's own unit tests;
      denominator published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10, template) — restate
      the real numbers in the PR once the crate is built (mark done then).
- [x] Tests that touch the filesystem: none needed — this crate has no filesystem access to test
      against; if a future test needs a temp file it must use `tempdir()`, never the repo tree.
- [x] Every non-goal in §2 is absent from the code — no subprocess spawn, no `env::var`, no
      `fs::`/`std::io` call anywhere in `crates/fleet-router/src/`; enforce with a clippy/grep
      check in CI if desired (`grep -rn 'Command::new\|env::var\|std::fs' crates/fleet-router/src/` must return nothing).
- [ ] No source file exceeds 80 lines — §8 splits `decide()`/`stage()`/types into separate ≤80-line
      files; verified by the §10 `wc -l ... awk '$1>80'` gate before Opus review.

## 12. Definition of Done

`fleet-router` is DONE when: §10's three commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught) run from `crates/fleet-router/`; every unchecked box
in §11 is checked with its real numbers; `registry/services/REGISTRY.md` (or `features/REGISTRY.md`,
per C1/L2 — router is infrastructure, likely `services/`) lists the crate; and Opus has
re-derived the 6-stage pipeline from this blueprint alone (without re-reading route.rs), reproduced
the "first-empty-wins" mutation by hand, and driven one real `decide()` call end-to-end confirming
`selected_adapter`/`refusal` are mutually exclusive as claimed in §4.

---

## Divergence from MIGRATION-PLAN (for Opus)

MIGRATION-PLAN §3 row 4 describes `fleet-router` as reuse from `route.rs — near-exact match, pure
decision`. Having read the full 687-line file, two things diverge from that framing:

1. **`route.rs` is not pure end-to-end — only `decide()` (and its private helpers `role_allows`,
   `stage`) are.** The other ~60% of the file (`adapter_contract`, `installed_non_interactive`,
   `runtime`, `for_plan`, `for_plan_with_builder`, `print_human`, `command`, `invalid_args`) is
   subprocess-spawning capability probing, disk/env reads, CLI parsing, and ledger writes. "Near-
   exact match" is true of the ~230 lines this blueprint actually extracts (`decide` + its types);
   it is not true of the file as a whole. The blueprint above treats this as the crate boundary
   itself (§2 non-goals, §5's "no" rows) rather than a problem — but it means **the extraction is a
   file split, not a file move**, and the IO half needs a real home: this blueprint assigns it to
   `fleet-govern` (quota/cooldown, matching roster item 11's own note "route.rs failover") plus
   `src/` (CLI/printing) plus `fleet-store` (receipt-write-on-refusal). MIGRATION-PLAN doesn't
   currently say this explicitly anywhere — worth a line in §3's row 4 and row 11.

2. **`Role` (and the safety-gate `Check`/`evaluate`) live in `fleet/keel/fleet/src/roles.rs`, a
   168-line file MIGRATION-PLAN's roster never cites.** Row 1 (`fleet-types`) cites only
   `agent.rs` and `lifecycle.rs` as evidence, but `decide()`'s stage 1 (`role_allows`) and stage 2
   (safety policy) are both entirely dependent on `roles.rs`'s `Role` enum and `Check`/`evaluate`.
   This blueprint resolves it by splitting `roles.rs` itself: `Role` (plus `parse`/`name`/
   `bandwidth`/`allocation_fitness`/`owned_gate`/`may_write_code`, which look scheduling-relevant
   beyond routing) goes to `fleet-types`; the two routing-specific safety rules (`LEAD_WROTE_CODE`,
   `SELF_VERIFIED`) are re-homed into `fleet-router` itself as `RoleRefusal`/`evaluate_role_check`,
   since nothing else in the roster currently calls them. **`roles.rs` should be added to row 1's
   evidence column** so whoever builds `fleet-types` doesn't miss it — right now it's an
   undocumented dependency two crates deep.
