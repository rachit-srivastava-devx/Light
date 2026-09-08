# BLUEPRINT — `fleet-govern`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-govern`
- **One-line purpose:** Own budget admission, real token metering, and quota-aware failover for
  multi-day fleet runs — reserve tokens atomically before a lane is spawned, persist exact usage,
  and pick or escalate away from an over-budget/cooling-down lane, with every fact about the outside
  world (files, clocks, capability probes) arriving through an injected port.
- **Build branch:** `refactor+unify` (MIGRATION-PLAN §3 row 11) — `fleet/keel/fleet/src/meter.rs`
  (687 lines, read in full 2026-09-08) supplies the persistent-state format and most of the data
  model; `route.rs`'s `runtime()` (`route.rs:316-346`) supplies the failover/`RuntimeState`-assembly
  shape; `registry/features/budget/budget.sh` + `registry/lib/receipt.sh:invariant_i2`
  (`receipt.sh:305-309`) supply the admission invariant. **None of the three sources has the atomic
  admission this crate must add** — see the "Divergence" note at the end: `meter.rs::reserve`
  (`meter.rs:138-160`) has the exact fan-out race this crate exists to close, and `budget.sh` is a
  stateless one-shot invariant check, not a reservation.
- **Imports:** `fleet-types` (`Tokens`, `Role`, `LaneId`, `EmptyIdentifier` — the last consumed by
  `LoopError::BadAdapter` when `AutonomousRun::tick` turns a router-selected adapter name into a
  `LaneId`), `fleet-router` (`RuntimeState`, `TaskClass`, `Decision`, `decide`) — a named
  sibling→sibling edge per MIGRATION-PLAN §3's DAG note ("sibling crates connect only through named
  minimal edges"); this crate is the caller that assembles `RuntimeState` from live probes and calls
  `fleet_router::decide` (per `fleet-router`'s own §1 "Imported by" line).
- **Imported by:** `fleet-worker` (calls `admit` before spawning a lane's process, `settle` after it
  exits, and reads `next_provider`'s `Decision` to know which adapter/model to invoke), `src/`
  (composition root — wires the meter state path, the tokenizer, and the capability-probe port at
  startup, and prints/receipts a refusal).
  > **(post-build)** The built crate also ships a multi-day autonomous-loop driver
  > (`AutonomousRun`/`tick`/`complete_unit` in `loop_run.rs`/`loop_complete.rs`, plus
  > `LoopStore`/`FileLoopStore`/`LoopPlan`/`LoopProgress`/`UnitId`/`TickOutcome`/`LoopError`) that
  > this section and §3 did not originally describe — it composes this crate's own `next_provider`/
  > `admit`/`escalate` into a resumable `tick(now)` state machine. See the new §3 subsection below
  > and the divergence note at the end of this file.

## 2. Responsibility & non-goals

**Owns:** the measured-token ledger (one row per lane: configured window, used, live reservations),
the atomic admit/settle protocol that makes "spawn N children, all believing they have budget" the
overspend it should never be, real token-count estimation for admission (never a `chars/4` guess),
the cooldown clock a failed/quota-exhausted adapter sits in, assembling `fleet_router::RuntimeState`
from that ledger plus an injected capability probe and calling `fleet_router::decide` for a routing
pick, and the throttle→downgrade→cached→pause escalation ladder a long-running dispatch consults
when spend approaches its budget.

**Non-goals (the seam):**
- Does **not** decide *which* candidate wins among those available — that comparison (the 6-stage
  filter pipeline, the tie-breaker over `ORDER`) is `fleet-router::decide`'s job; this crate only
  builds the `RuntimeState` `decide` reads and forwards its `Decision`.
- Does **not** probe whether an adapter CLI is installed or authenticated — the actual `which
  codex`/`codex --version`/`python3 -c "...probe_adapter..."` subprocess calls
  (`route.rs:280-314`'s `adapter_contract`/`installed_non_interactive`) are `fleet-worker`'s job;
  this crate only consumes an already-computed capability set through an injected
  `CapabilityProbe` port (§4) — it never shells out itself.
- Does **not** run the LLM call or read tokens off an adapter's response — `fleet-worker` reports
  the actual token count to `settle` after the call completes; this crate never touches a network
  socket or a subprocess.
- Does **not** write ledger receipts (`Receipt`/blake3 chain) — that is `fleet-store`'s job
  (`main.rs::append_receipt`, MIGRATION-PLAN row 2); a refusal from this crate is a typed return
  value the caller may choose to receipt.
- Does **not** parse CLI args or print human/JSON output — composition-root (`src/`) work, same
  seam `fleet-router`'s blueprint already drew for `route.rs::command`/`print_human`.
- Does **not** read `SystemTime::now()`/an RNG/an env var ambiently anywhere in its logic — every
  such fact is a parameter (§4).

## 3. Public API contract

```rust
//! Budget admission, token metering, and quota-aware failover.
//!
//! This crate answers three questions with zero ambient IO: "may this lane spend N more tokens
//! right now, and if so, hold them" (`admit`/`settle`), "given what's currently capable, quota'd,
//! and cooling down, which candidate should `fleet-router` pick" (`next_provider`), and "spend is
//! approaching budget — what should the caller do about it" (`escalate`). Every fact about the
//! outside world (the meter file, the wall clock, which adapters are installed) arrives through an
//! injected port; this crate never calls `SystemTime::now()`, never spawns a process, and never
//! reads an environment variable inside its own logic.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};

use fleet_router::{Decision, RuntimeState, TaskClass};
use fleet_types::{LaneId, Role, Tokens, TokensOverflow};

// ---------------------------------------------------------------------------------------------
// Ledger state — one row per lane, mirroring meter-v1.tsv's shape (meter.rs:17-30)
// ---------------------------------------------------------------------------------------------

/// One lane's measured state. `window`/`used` are `None` exactly when unmeasured — never coerced
/// to zero (the invariant `meter.rs`'s module doc names explicitly, preserved verbatim here).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaneState {
    pub window: Option<Tokens>,
    pub used: Option<Tokens>,
    pub reservations: Vec<Reservation>,
    pub resolved_model: Option<String>,
    /// Set once an observation with an unknown token count lands (`meter.rs:372-376`'s
    /// `unknown_observed`) — once true, `used` can never again be treated as exact.
    pub unknown_observed: bool,
}

/// A held claim on a lane's budget between `admit` and `settle`. Carries its own id (unlike
/// `meter.rs:21`'s bare `Vec<u64>`, which cannot tell two reservations of the same size apart —
/// see §5) so `settle` can find and remove exactly the reservation it is closing out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReservationId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reservation {
    pub id: ReservationId,
    pub lane: LaneId,
    pub estimated: Tokens,
}

/// Why `admit` refused to reserve. Never a bare `Option`/`bool` — every refusal names the exact
/// unmeasured/insufficient fact so the caller's escalation ladder (`escalate`) has something to
/// act on.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AdmitError {
    #[error("lane {0:?} is not configured")]
    UnknownLane(String),
    #[error("lane {0:?} has no configured window (unmeasured, not unlimited)")]
    WindowUnknown(String),
    #[error("lane {0:?} usage is unmeasured (unknown_observed)")]
    UsedUnknown(String),
    #[error("lane {lane:?} would exceed its window: requested {requested:?} > remaining {remaining:?}")]
    InsufficientBudget { lane: String, requested: Tokens, remaining: Tokens },
    #[error("token arithmetic overflowed while reserving")]
    Overflow(#[from] TokensOverflow),
    #[error("meter store IO failed: {0}")]
    Store(#[from] MeterIoError),
}

/// Why `settle` could not close out a reservation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SettleError {
    #[error("reservation {0:?} is not open on any known lane")]
    UnknownReservation(ReservationId),
    #[error("token arithmetic overflowed while settling")]
    Overflow(#[from] TokensOverflow),
    #[error("meter store IO failed: {0}")]
    Store(#[from] MeterIoError),
}

/// The meter file could not be read, decoded, or durably published.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct MeterIoError(pub String);

// ---------------------------------------------------------------------------------------------
// The injected persistence port — the atomic admit/settle boundary
// ---------------------------------------------------------------------------------------------

/// The sole IO boundary this crate touches. `with_lane_locked` is the entire atomicity contract:
/// the implementation must hold an exclusive lock across the read-check-mutate-publish sequence
/// so two concurrent callers (the fan-out race — see §5's divergence note) can never both observe
/// the same `remaining` and both admit past it. `FileMeterStore` (in `store_file.rs`) is the real
/// implementation, built on an OS advisory file lock; tests use an in-memory `Mutex<BTreeMap<...>>`
/// implementation instead — never the repo tree or `$HOME` (§11).
pub trait MeterStore: Send + Sync {
    /// Run `f` with exclusive access to `lane`'s current `LaneState` (absent lanes get
    /// `LaneState::default()`-shaped `None`s, never fabricated as configured-but-empty), publish
    /// whatever `f` leaves behind durably before returning, and return `f`'s result. `f` itself
    /// must be total and side-effect-free besides mutating `state` — it may run more than once
    /// under contention in a lock-free implementation (none is provided here; `FileMeterStore`
    /// never re-runs `f`, this is documented for anyone swapping the implementation).
    fn with_lane_locked<T>(
        &self,
        lane: &LaneId,
        f: &mut dyn FnMut(&mut Option<LaneState>) -> T,
    ) -> Result<T, MeterIoError>;

    /// A snapshot read of every configured lane's `remaining = window - used` (`None` propagates
    /// through `checked_sub` per the "unknown, never zero" rule), used only to build
    /// `RuntimeState.remaining` for `next_provider` — never used to decide admission (that always
    /// goes through `with_lane_locked`, which alone is race-free).
    fn snapshot_remaining(&self) -> Result<BTreeMap<String, Option<Tokens>>, MeterIoError>;
}

// ---------------------------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------------------------

/// Reserve `cost_est` tokens on `lane`, atomically, before the caller spawns the child that will
/// spend them — this is the fan-out race fix (§5, §6): the reservation happens under
/// `store`'s lock, so no second concurrent `admit` call on the same lane can observe
/// pre-reservation `remaining` and also succeed past the true budget.
pub fn admit(store: &dyn MeterStore, lane: &LaneId, cost_est: Tokens) -> Result<Reservation, AdmitError>;

// ---------------------------------------------------------------------------------------------
// Settlement
// ---------------------------------------------------------------------------------------------

/// Close out `reservation`, replacing its estimate with the real `actual` token count the caller
/// measured from the completed call. `used` moves by `actual - estimated` (may be negative in
/// effect, i.e. `used` can shrink back toward the estimate's slack) via `checked_add`/`checked_sub`
/// — never re-derived from scratch, so a concurrent lane's own usage is never clobbered.
pub fn settle(store: &dyn MeterStore, reservation: Reservation, actual: Tokens) -> Result<(), SettleError>;

// ---------------------------------------------------------------------------------------------
// Real token estimation — never chars/4
// ---------------------------------------------------------------------------------------------

/// A loaded tokenizer. `TiktokenTokenizer` (in `tokenizer.rs`) is the real implementation,
/// wrapping `tiktoken-rs`'s `cl100k_base`/`o200k_base` encoding; tests may inject a fixed-count
/// fake. This is this crate's only non-IO injection point besides the clock (§4) — it is pure
/// (same text always yields the same count) but is still a port, not called directly, so a
/// tokenizer-version bump is a caller decision, not a hidden crate constant.
pub trait Tokenizer: Send + Sync {
    fn count(&self, text: &str) -> Tokens;
}

/// Convenience wrapper: `estimate = tokenizer.count(text)`, exposed so callers building a
/// `cost_est` for `admit` never hand-roll a `len() / 4` heuristic.
pub fn estimate(tokenizer: &dyn Tokenizer, text: &str) -> Tokens { tokenizer.count(text) }

// ---------------------------------------------------------------------------------------------
// Cooldown — the clock-injected half of route.rs's runtime() (route.rs:316-346)
// ---------------------------------------------------------------------------------------------

/// Tracks which adapters are in a measured failure-backoff window. Replaces `route.rs:324-330`'s
/// `env::var("FLEET_ROUTE_COOLDOWNS")` (an ambient, unstructured, caller-set string) with real
/// state this crate owns and can expire on its own clock.
pub trait CooldownStore: Send + Sync {
    fn start(&self, adapter: &str, at: SystemTime, duration: Duration) -> Result<(), MeterIoError>;
    fn is_cooling_down(&self, adapter: &str, now: SystemTime) -> Result<bool, MeterIoError>;
}

// ---------------------------------------------------------------------------------------------
// Failover — assembling RuntimeState and calling fleet-router
// ---------------------------------------------------------------------------------------------

/// Everything this crate must be told to build one `RuntimeState`. Mirrors `route.rs:316`'s
/// `runtime()` signature, minus the subprocess probing it used to do inline (`adapter_contract`/
/// `installed_non_interactive`, `route.rs:280-314`) — that now arrives as `capable`, already
/// computed by `fleet-worker`.
pub struct FailoverInputs<'a> {
    pub role: Option<Role>,
    pub class: TaskClass,
    pub builder_resolved_model: Option<&'a str>,
    pub capable: BTreeSet<&'static str>,
    pub required_tokens: Tokens,
    pub now: SystemTime,
}

/// Build a `RuntimeState` from `store`/`cooldowns`/`inputs` and hand it to `fleet_router::decide`.
/// This is `route.rs:348-373`'s `for_plan`/`for_plan_with_builder`, re-homed: the meter-snapshot
/// read and the cooldown check move here, the decision logic stays in `fleet-router`.
pub fn next_provider(
    store: &dyn MeterStore,
    cooldowns: &dyn CooldownStore,
    inputs: &FailoverInputs<'_>,
) -> Result<Decision, MeterIoError>;

// ---------------------------------------------------------------------------------------------
// Escalation ladder
// ---------------------------------------------------------------------------------------------

/// Ascending token-count thresholds on a lane's *window* (never a float ratio — §4) at which the
/// caller should take the next, more drastic action. Each threshold must be `<=` the next; `escalate`
/// does not enforce this at construction (a plain data struct), but §9 tests the ladder's ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EscalationPolicy {
    pub throttle_at: Tokens,
    pub downgrade_at: Tokens,
    pub cached_at: Tokens,
    pub pause_at: Tokens,
}

/// What the caller should do next, in ascending severity. Never taken by this crate itself — it
/// has no subprocess, cache, or sleep of its own; the caller applies the recommendation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Escalation {
    Continue,
    Throttle { delay: Duration },
    Downgrade,
    UseCached,
    Pause,
}

/// Compare `used` (a lane's `LaneState.used`, or the max across the lanes a caller cares about)
/// against `policy`'s thresholds and return the single most severe action that applies. Pure,
/// total, no IO — `delay` inside `Throttle` is a fixed, policy-supplied backoff, never computed
/// from a clock read here.
pub fn escalate(used: Tokens, policy: &EscalationPolicy, throttle_delay: Duration) -> Escalation;

// ---------------------------------------------------------------------------------------------
// (post-build addition, not in the original contract) Multi-day autonomous loop
// ---------------------------------------------------------------------------------------------
// The built crate adds a resumable driver on top of admit/settle/next_provider/escalate: a
// `LoopPlan` (a fixed, ordered `Vec<UnitId>` of work units plus routing/budget parameters) and a
// `LoopProgress` (just `completed: Vec<UnitId>`) persisted behind an injected `LoopStore` port
// (`FileLoopStore` is the real file-backed implementation, same lock+tmp-rename pattern as
// `FileMeterStore`/`FileCooldownStore`). `AutonomousRun::tick(now, capable)` calls
// `next_provider` to pick a provider, `escalate` to check the ladder, and `admit` to reserve
// budget for the next not-yet-completed unit, returning a `TickOutcome` (`Advanced { unit,
// decision, reservation }` / `Paused { until }` / `Exhausted { until }` / `Done`) — it never
// marks a unit done itself. `AutonomousRun::complete_unit(unit, reservation, actual)` is the
// other half: it `settle`s the reservation and then persists the unit as done via `LoopStore`,
// in that order, so a crash between `tick` and `complete_unit` never loses or double-counts
// progress and a restart days later resumes from `LoopStore::load` alone (never anything held in
// memory). Every fallible path collapses into one typed `LoopError`. See §5/§8/the divergence
// note at the end of this file for what is and isn't yet reflected elsewhere in this blueprint.

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `LaneState` | `window`/`used: Option<Tokens>` — absence means unmeasured, never coerced to `Some(0)`; mirrors `meter.rs`'s module-level rule verbatim. | A lane with an unmeasured window being scheduled against as if it had zero (refused) or infinite (overspent) budget. |
| `Reservation`/`ReservationId` | Every reservation carries a unique, store-assigned id; `settle` addresses a reservation by id, never by scanning for a matching token count. | Two same-sized concurrent reservations on one lane being indistinguishable at settlement time — today's `meter.rs:21`'s bare `Vec<u64>` cannot tell them apart; this type can. |
| `MeterStore::with_lane_locked` | The entire read-check-mutate-publish sequence for one lane runs under one exclusive lock acquisition — no `admit`/`settle` call ever observes a `LaneState` that a concurrent call is mid-mutation on. | The fan-out race this crate exists to close: two children admitted concurrently both reading `remaining = 500` before either's reservation is visible, both being granted 400, for 800 total against a 500 window. |
| `Tokens` (from `fleet-types`) | `checked_add`/`checked_sub` only; overflow is a typed `TokensOverflow`, never a wrap. | A reservation or settlement silently wrapping past `u64::MAX` and appearing to *free* budget. |
| `EscalationPolicy` | Four ascending `Tokens` thresholds, compared by `>=`, never by a derived float ratio. | A budget-ratio computed in floating point drifting from the integer ledger it's supposed to describe (e.g. `used as f64 / window as f64` rounding two different lanes' near-identical states to the same or different ladder rungs inconsistently). |
| `CooldownStore` | `is_cooling_down` takes `now: SystemTime` as a parameter; the store itself never calls `SystemTime::now()`. | A cooldown check silently using a different "now" than the caller's own clock, making cooldown expiry untestable and non-deterministic in CI. |

**Money/precision:** no money type in this crate; every count is `fleet_types::Tokens` (`u64`
minor-units), never `f32`/`f64`. `escalate`'s comparisons are integer `Tokens` vs `Tokens`, not a
computed ratio (see table row above).

**Clock/RNG/IO injection points:** `CooldownStore::is_cooling_down`/`start` take `now`/`at` as
parameters — no `SystemTime::now()` call anywhere in this crate's own logic (the caller reads the
clock once and passes it in, exactly like `fleet-router`'s "caller computes, crate consumes"
pattern). No RNG anywhere. IO is exactly two ports: `MeterStore` (the meter file — real
implementation in `store_file.rs`, holding an OS advisory lock for the critical section) and
`CooldownStore` (a small persisted table, same shape); `Tokenizer` is a pure but still-injected
port (no ambient global tokenizer). Nothing else — no subprocess, no network, no other file.

## 5. Reuse map

Source read in full: `fleet/keel/fleet/src/meter.rs` (687 lines, 2026-09-08),
`fleet/keel/fleet/src/route.rs` lines 280-473 (2026-09-08),
`fleet/registry-reference/registry/features/budget/budget.sh` (46 lines),
`fleet/registry-reference/registry/lib/receipt.sh:290-313` (`invariant_i2`, `invariant_i3`), and
`fleet/registry-reference/registry/lib/chain-rs/src/lib.rs:422-437` (`invariant_i2`'s Rust twin).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `meter.rs:17-24` (`Lane`) | `{window, used, reservations: Vec<u64>, resolved_model, unknown_observed}`. | reshaped | Becomes `LaneState`; `reservations` becomes `Vec<Reservation>` (id-carrying, §3/§4) instead of a bare `Vec<u64>`; `window`/`used` become `Option<Tokens>` instead of `Option<u64>`. |
| `meter.rs:41-99` (`Meter::load`, TSV decode) | Parses `fleet-meter-v1\ttokenizer=...` header + tab-separated lane rows, treating empty fields as `None` never `0`. | yes, logic verbatim | Move into `store.rs`'s TSV codec, called by `FileMeterStore` under its lock; field types widen to `Tokens`. |
| `meter.rs:101-136` (`Meter::save`) | `create_new` a `.tmp` file, `sync_all`, `fs::rename` over the real path. | **partially — this is the exact race, not yet a fix** | `rename` here overwrites unconditionally; it never checks whether the file changed since it was loaded. Two processes calling `load` → mutate → `save` concurrently can silently lose one's reservation (classic lost-update). This crate's `FileMeterStore` keeps the write-tmp-then-rename durability trick but wraps the *entire* load-mutate-save sequence in one held OS advisory file lock (`with_lane_locked`), which `save` alone never provided. |
| `meter.rs:138-160` (`reserve`) | In-memory `checked_sub`/`checked_add` + push to `reservations`, no locking, called once per CLI invocation with no concurrent caller in mind. | logic yes, concurrency no | Becomes `admit`'s body, executed inside `with_lane_locked`'s closure — the arithmetic is identical, the atomicity wrapper is new. |
| `meter.rs:176-191` (`measured_task_tokens`) | Ceiling-average of all lanes' held reservations, used by `plan` for capacity estimates. | not lifted | Superseded by real per-call `estimate()` via an injected `Tokenizer` (§3) — this crate never needs to *guess* a task's size from past reservations once a real tokenizer is available; `fleet-plan` (a different crate) may still want this kind of historical averaging and can read `MeterStore::snapshot_remaining` if so, but it is not this crate's job to keep computing it. |
| `meter.rs:194-201` (`state_path`, `FLEET_STATE` env var) | Ambient env-var read for where the meter file lives. | **no** | Ambient IO — excluded per §2. The caller (`src/`, at composition time) resolves the path and constructs `FileMeterStore` with it; this crate never calls `env::var`. |
| `meter.rs:204-218` (`tokenizer_generation`) | Shells `codex --version` / reads `CODEX_TOKENIZER_GENERATION` to *label* which tokenizer produced past counts. | **no** | Subprocess + env — excluded. If a caller wants to record this label alongside `LaneState.resolved_model`, it passes it in; this crate does not go looking for it. |
| `route.rs:280-304` (`adapter_contract`) / `route.rs:306-314` (`installed_non_interactive`) | Subprocess probes for adapter capability. | **no** | `fleet-worker`'s job (per `fleet-router`'s own non-goals, which name this exact split); arrives here as `FailoverInputs.capable`, already computed. |
| `route.rs:316-346` (`runtime`) | Assembles `RuntimeState` from the two probes above, `FLEET_ROUTE_COOLDOWNS`, a freelane-script existence check, and the meter snapshot. | reshaped | Becomes `next_provider`'s internal `RuntimeState` assembly: `capable` arrives via `FailoverInputs` (no subprocess here), `cooldown` comes from `CooldownStore::is_cooling_down` (no env var), `remaining` comes from `MeterStore::snapshot_remaining` (already `Tokens`-typed instead of raw `u64`), `preference` stays `ORDER.iter().map(|c| c.id).collect()` exactly as `fleet-router` already defines it — this crate does not redefine `ORDER`. |
| `route.rs:348-373` (`for_plan`, `for_plan_with_builder`) | Reads meter snapshot, builds `RuntimeState`, calls `decide`. | yes, re-homed | Becomes `next_provider` verbatim in shape, typed-error instead of raw `i32` (`route.rs:360`'s `?` on `planning_snapshot_at`'s `Result<_, i32>` becomes `Result<_, MeterIoError>`). |
| `budget.sh:27-38` + `receipt.sh:305-309` (`invariant_i2`) | `projected <= remaining` check, refuses otherwise (`ERR_BUDGET`, exit 5). | yes, logic verbatim | The exact comparison `admit`'s `InsufficientBudget` branch performs — `chain-rs/src/lib.rs:422-437`'s Rust port is the closer reference (typed `LedgerError::Invariant{code:5,..}`, same shape as this crate's `AdmitError::InsufficientBudget`). `budget.sh` itself is a stateless one-shot CLI gate with no reservation/persistence at all — it is evidence for the *invariant*, not for the admission *protocol*, which this crate must build new (see divergence note). |

## 6. Behavior spec

### `fn admit(store: &dyn MeterStore, lane: &LaneId, cost_est: Tokens) -> Result<Reservation, AdmitError>`

| Input dimension | Behavior |
|---|---|
| empty | `cost_est = Tokens::ZERO` on a configured, in-budget lane → `Ok(Reservation { estimated: Tokens::ZERO, .. })` — a zero-cost reservation is legal and immediately settleable; it still gets a real `ReservationId` so `settle` has something to close. |
| null / `None` | A lane whose `window`/`used` is `None` (unmeasured) → `Err(AdmitError::WindowUnknown)`/`Err(AdmitError::UsedUnknown)` respectively — never treated as "unlimited," matching `meter.rs:143-148`'s existing `ok_or_else` behavior exactly. |
| wrong-type | n/a — `lane: &LaneId` is already validated non-empty by `fleet-types`; `cost_est: Tokens` is already a `u64`-backed value, no stringly-typed input crosses this boundary. |
| huge | `cost_est = Tokens::new(u64::MAX)` against any finite window → `Err(InsufficientBudget { requested: u64::MAX, .. })`, computed via `checked_sub`/comparison, never overflowing or silently wrapping to "fits." |
| negative | n/a — `Tokens` cannot represent a negative count. |
| duplicate | Two `admit` calls for the same `lane` with identical `cost_est`, back to back, single-threaded → two distinct `Reservation`s with two distinct `ReservationId`s, both counted against `used` — "duplicate-looking" is not "duplicate identity," and both are legitimately held. |
| concurrent | Two threads/processes calling `admit` on the **same** `lane` at the same time → both calls serialize through `MeterStore::with_lane_locked`'s exclusive lock; the second sees the first's reservation already reflected in `used` before deciding — this is the fan-out race fix and the crate's central property, tested explicitly in §9's `concurrent_admits_never_oversubscribe_a_lane`. |
| unicode / non-ASCII | `lane`'s underlying `LaneId` already enforces non-empty via `fleet-types`; no ASCII-only constraint beyond that — a unicode lane name round-trips through the TSV codec (`store.rs`) the same as any other string field, since the codec only special-cases tab/newline as delimiters (matching `meter.rs:228`'s existing `contains(['\t','\n'])` guard, ported unchanged). |
| already-exists | Calling `admit` for a `lane` not yet present in the store → `Err(AdmitError::UnknownLane)` — this crate never silently creates a new lane with a fabricated window; lane configuration is the caller's job (mirrors `meter.rs:364-393`'s distinction between "known lane, update it" and "unknown lane" — but where `record_observation_at` *does* insert an unconfigured lane on first observation, `admit` deliberately does not, since inventing a window for admission would let an unconfigured lane be treated as unlimited). |
| partial-failure | The store's lock is acquired but the durable publish step (the tmp-file `rename`, `meter.rs:135`'s pattern) fails midway (e.g. disk full) → `Err(AdmitError::Store(MeterIoError(..)))`; the in-memory mutation inside the closure is discarded (the lock guards the *file*, not an already-committed value) — a caller seeing this error must treat the reservation as **not** granted, never assume partial success. |

### `fn settle(store: &dyn MeterStore, reservation: Reservation, actual: Tokens) -> Result<(), SettleError>`

| Input dimension | Behavior |
|---|---|
| empty | `actual = Tokens::ZERO` on a reservation estimated higher → `used` shrinks by the estimate-minus-zero difference via `checked_sub`, reservation removed — a task that used nothing still settles cleanly. |
| null / `None` | n/a — `actual: Tokens` is always present; there is no "unknown actual" case for `settle` (an unmeasurable outcome is the caller's problem before calling `settle`, same as `admit`'s `cost_est`). |
| wrong-type | n/a — same as `admit`. |
| huge | `actual = Tokens::new(u64::MAX)` on a small `estimated` → `used` is adjusted via `checked_add`, returning `Err(SettleError::Overflow)` rather than wrapping if the addition would exceed `u64::MAX`; the reservation is still removed from `reservations` (accounted-for, even on overflow error — never left dangling). |
| negative | n/a — `Tokens` is unsigned. |
| duplicate | Calling `settle` twice with the **same** `ReservationId` (already consumed) → `Err(SettleError::UnknownReservation)` on the second call — a reservation can be settled exactly once. |
| concurrent | Same lock-serialization as `admit` — two `settle` calls on the same lane never interleave their `used` adjustments. |
| unicode / non-ASCII | n/a beyond `admit`'s note — `settle` carries no new string input. |
| already-exists | n/a — the "already settled" case is the duplicate row above. |
| partial-failure | Same durable-publish-failure shape as `admit`: `Err(SettleError::Store(..))` means the settlement did **not** take effect; the reservation remains open for a caller to retry. |

### `fn next_provider(store: &dyn MeterStore, cooldowns: &dyn CooldownStore, inputs: &FailoverInputs<'_>) -> Result<Decision, MeterIoError>`

| Input dimension | Behavior |
|---|---|
| empty | `inputs.capable` empty → `RuntimeState.capable` empty → `fleet_router::decide` refuses at its stage 3 (per `fleet-router`'s own spec); this crate does not special-case it, it only assembles and forwards. |
| null / `None` | `inputs.builder_resolved_model: None` while `role == Some(Role::Verifier)` → forwarded as-is to `decide`, which refuses at stage 5 exactly per `fleet-router`'s §6. |
| wrong-type | n/a — `inputs` is already a typed struct; no stringly-typed input crosses this fn. |
| huge | Thousands of configured lanes in the store → `snapshot_remaining` is `O(lanes)`, a single `BTreeMap` build; no unbounded work beyond that, and `decide`'s own cost stays `O(|ORDER|)` regardless (per `fleet-router`'s §6). |
| negative | n/a — `required_tokens: Tokens` is unsigned. |
| duplicate | n/a — no identity-bearing collection at this boundary. |
| concurrent | `snapshot_remaining` and `is_cooling_down` are both read-only queries — safe to call from multiple threads concurrently; they may race with a concurrent `admit`/`start` (a snapshot might be one reservation stale), which is acceptable here because `next_provider` only informs a *choice*, never an admission decision (only `admit`'s `with_lane_locked` is the race-free boundary — see §4). |
| unicode / non-ASCII | Adapter names/lane names round-trip as plain strings through both stores; no normalization performed, matching `fleet-router`'s own documented non-normalizing string comparisons. |
| already-exists | n/a — `next_provider` is idempotent by construction (pure function of `store`'s/`cooldowns`' current readable state at call time), matching `fleet-router::decide`'s own determinism guarantee. |
| partial-failure | `MeterStore::snapshot_remaining` or `CooldownStore::is_cooling_down` failing (e.g. a corrupt meter file) → `Err(MeterIoError(..))` propagated whole; `next_provider` never calls `decide` on a partially-read `RuntimeState`. |

### `fn escalate(used: Tokens, policy: &EscalationPolicy, throttle_delay: Duration) -> Escalation`

| Input dimension | Behavior |
|---|---|
| empty | `used = Tokens::ZERO` against any policy with all thresholds `> 0` → `Escalation::Continue`. |
| null / `None` | n/a — all params are always-present values, no `Option` at this boundary. |
| wrong-type | n/a — no stringly-typed input. |
| huge | `used = Tokens::new(u64::MAX)` → matches `pause_at` (the highest threshold, assuming ascending order) → `Escalation::Pause`, regardless of how far past `pause_at` it is — this fn does not distinguish "just over" from "wildly over," which is correct: there is no rung more severe than `Pause`. |
| negative | n/a — `Tokens` is unsigned. |
| duplicate | n/a — not a collection operation. |
| concurrent | Pure value fn, no shared state — trivially safe. |
| unicode / non-ASCII | n/a — no string input. |
| already-exists | n/a — no persisted state; calling `escalate` repeatedly with the same inputs is required to be idempotent (pure fn). |
| partial-failure | n/a — no IO, cannot fail partially; always returns a value, never a `Result`. |
| ties (thresholds equal) | `used == throttle_at == downgrade_at` (a degenerate policy) → the **most severe** matching rung wins (`downgrade_at`'s `Downgrade`, not `throttle_at`'s `Throttle`) — `escalate` checks from `pause_at` down to `throttle_at`, returning on the first (highest) match; this ordering is asserted explicitly in §9, not left to match-arm accident. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace path (`{ path = "../fleet-types" }`) | `Tokens`, `TokensOverflow`, `Role`, `LaneId`. |
| `fleet-router` | workspace path (`{ path = "../fleet-router" }`) | `RuntimeState`, `TaskClass`, `Decision`, `decide` — `next_provider` calls straight through. |
| `thiserror` | `2.0.20` (matches `fleet/keel/Cargo.lock`, already used by `fleet-types`) | Every fallible fn returns a `thiserror`-derived enum (`AdmitError`, `SettleError`, `MeterIoError`) — no `String`/`anyhow`/bare `bool`. |
| `tiktoken-rs` | `0.6.0` | The real-tokenizer requirement — `cl100k_base`/`o200k_base` BPE encoding, the same family OpenAI/Anthropic-compatible tooling uses; actively maintained, pure-Rust, no network call at count time (vocab is compiled in). Wrapped by `tokenizer.rs`'s `TiktokenTokenizer`, never called directly outside that one file, so a future tokenizer swap touches one ~40-line file. |
| `fs4` | `0.9`, `features = ["sync"]` (built Cargo.toml enables the `sync` feature explicitly; not called out in this section originally) | Cross-platform OS advisory file locking (`lock_exclusive`/`unlock`) for `FileMeterStore::with_lane_locked`'s critical section — the successor to the now-unmaintained `fs2`, chosen for the same reason `fleet-types` prefers already-resolved crates: no existing fleet dependency provides file locking, and this is the one genuinely new capability this crate needs beyond what's already in the lockfile. The lock helper is shared (`file_lock.rs`) across `FileMeterStore`, `FileCooldownStore`, and `FileLoopStore` (§8). |

No other crate is needed — no async runtime (every port is a synchronous trait; `fleet-worker`
calls these fns from whatever thread it dispatches on), no logging framework, no JSON here (this
crate's TSV codec is hand-rolled, matching `meter.rs`'s own format — `fleet-router`'s `Decision` is
the only `serde`-shaped value this crate touches, and it derives `Serialize` in `fleet-router`
itself, not here).

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.**

```
crates/fleet-govern/
  Cargo.toml
  src/
    lib.rs               # ~47 — module decls + re-exports only
    types.rs              # ~70 — LaneState, Reservation, ReservationId, AdmitError, SettleError, MeterIoError
    store.rs               # ~39 — MeterStore trait, CooldownStore trait
    meter_codec.rs           # ~39 — hand-rolled TSV codec for the ledger file (top level)
    row_codec.rs               # ~47 — one lane's TSV row, split out of meter_codec.rs to hold the 80-line rule
    reservation_codec.rs         # ~16 — one `id:estimated` reservation field, split out of meter_codec.rs
    file_lock.rs                   # ~32 — shared fs4 exclusive-lock helper used by all three File*Store impls (not anticipated by this section)
    store_file.rs                    # ~77 — FileMeterStore: fs4 lock (via file_lock) + tmp-write-then-rename publish
    cooldown_file.rs                   # ~64 — FileCooldownStore: same lock+publish pattern, small table
    admit.rs                             # ~36 — admit()
    settle.rs                              # ~37 — settle()
    tokenizer.rs                             # ~46 — Tokenizer trait, TokenizerLoadError, TiktokenTokenizer (TokenizerLoadError not anticipated by this section)
    failover.rs                                # ~53 — FailoverInputs, next_provider() assembling RuntimeState
    escalate.rs                                  # ~44 — EscalationPolicy, Escalation, escalate()
    loop_types.rs                                  # ~50 — UnitId, LoopPlan, LoopProgress, LoopIoError (post-build addition, §3)
    loop_store.rs                                    # ~16 — LoopStore trait (post-build addition)
    loop_store_file.rs                                 # ~59 — FileLoopStore (post-build addition)
    loop_row_codec.rs                                    # ~24 — FileLoopStore's row codec, split out to hold the 80-line rule (post-build addition)
    loop_error.rs                                          # ~22 — LoopError (post-build addition)
    loop_outcome.rs                                          # ~31 — TickOutcome (post-build addition)
    loop_run.rs                                                # ~65 — AutonomousRun, tick() (post-build addition)
    loop_complete.rs                                             # ~36 — AutonomousRun::complete_unit()/lane_used(), split out of loop_run.rs to hold the 80-line rule (post-build addition)
  tests/
    admit_settle.rs      # ~63 — single-threaded admit/settle behavior-spec cases
    admit_settle_file.rs   # ~40 — admit/settle round-tripped through a real FileMeterStore (not anticipated by this section)
    settle_stale_id.rs       # ~28 — settle refuses an unknown/already-settled ReservationId (not anticipated by this section)
    cas_conflict.rs             # ~59 — concurrent admit/settle via real threads on FileMeterStore (tempdir)
    failover_pipeline.rs          # ~60 — next_provider end-to-end against an in-memory MeterStore/CooldownStore
    escalate_ladder.rs               # ~52 — ladder ordering + tie-breaking cases
    autonomous_run.rs                  # ~45 — AutonomousRun::tick/complete_unit happy path (post-build addition)
    autonomous_run_pause.rs              # ~36 — tick's Paused/Exhausted outcomes (post-build addition)
    autonomous_run_restart.rs              # ~75 — resuming a LoopPlan from a fresh FileLoopStore load after a simulated restart (post-build addition)
    support/
      mod.rs                                # ~42 — shared test-double wiring (post-build addition)
      meter_store.rs                          # ~69 — in-memory MeterStore test double (post-build addition)
      failover_store.rs                         # ~45 — in-memory CooldownStore/failover test doubles (post-build addition)
      loop_store.rs                               # ~28 — in-memory LoopStore test double (post-build addition)
```
`lib.rs`'s module list plus re-exports is the only file allowed to exceed a handful of `pub use`
lines; the originally-sketched `store.rs`/`store_file.rs` were split further once bodies landed
(`meter_codec.rs`/`row_codec.rs`/`reservation_codec.rs`/`file_lock.rs`), as this section
anticipated. The multi-day autonomous-loop files (`loop_*.rs` in `src/`, `autonomous_run*.rs` +
`support/` in `tests/`) are a whole post-build addition this section did not originally sketch —
see the divergence note at the end of this file.

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-govern"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-types = { path = "../fleet-types" }
fleet-router = { path = "../fleet-router" }
thiserror = "2.0.20"
tiktoken-rs = "0.6.0"
fs4 = { version = "0.9", features = ["sync"] }

[dev-dependencies]
tempfile = "3"
```

## 9. Test plan

**Unit tests:**
- `admit_refuses_unknown_lane` — asserts `Err(AdmitError::UnknownLane(_))` for a lane absent from
  the store.
- `admit_refuses_unmeasured_window_and_unmeasured_used` — two cases, `window: None` and (separately)
  `used: None` on an otherwise-configured lane, both `Err`, neither silently treated as available.
- `admit_boundary_exact_fit_succeeds_one_over_refuses` — `cost_est == remaining` → `Ok`;
  `cost_est == remaining + 1` → `Err(InsufficientBudget)` — the exact I2 boundary from
  `chain-rs/src/lib.rs:491-495`'s own `"500","500"` / `"501","500"` cases, ported.
- `settle_unknown_reservation_id_is_refused` — a fabricated `ReservationId` not currently open →
  `Err(SettleError::UnknownReservation)`.
- `settle_twice_on_same_reservation_is_refused` — first `settle` succeeds, second (same id) errors.
- `escalate_ladder_returns_most_severe_matching_rung` — degenerate policy with equal thresholds;
  asserts the *highest* rung wins, not match-arm order accident (§6's "ties" row).
- `escalate_below_every_threshold_is_continue` / `escalate_above_pause_is_pause` — the two ends of
  the ladder.

**Integration tests (`tests/`):**
- `concurrent_admits_never_oversubscribe_a_lane` (`cas_conflict.rs`) — spawn N real OS threads, each
  calling `admit` for `window / N` tokens against a `FileMeterStore` backed by one `tempdir()` file;
  assert every `admit` that returns `Ok` is accounted for in the final persisted `used`, and the
  total never exceeds `window` — the direct regression test for the fan-out race named in §1/§5.
- `admit_then_settle_round_trips_through_a_real_file` (`admit_settle.rs`) — `FileMeterStore` over a
  `tempdir()`; `admit` → `settle` with a different `actual` → reload a fresh `FileMeterStore` over
  the same path and assert `used` reflects `actual`, not `estimated`.
- `next_provider_forwards_to_fleet_router_and_respects_cooldown` (`failover_pipeline.rs`) — an
  in-memory `MeterStore`/`CooldownStore` with one lane cooling down; asserts the returned `Decision`
  never selects that lane's adapter (cross-checked against `fleet-router`'s own stage-4/cooldown
  semantics, without re-testing `decide`'s internals — this crate only tests that its own assembly
  is correct).
- `admit_publish_failure_leaves_no_partial_reservation` (`admit_settle.rs`) — inject a
  `MeterStore` whose publish step fails after the closure runs; assert a retried `admit` on a fresh,
  uncorrupted store sees no phantom reservation from the failed attempt.

**Mutation-testing targets (`cargo mutants -p fleet-govern`):**
- Flipping `admit`'s `requested <= remaining` to `requested < remaining` (the I2 boundary) must be
  killed by `admit_boundary_exact_fit_succeeds_one_over_refuses`.
- Removing the lock acquisition in `FileMeterStore::with_lane_locked` (or narrowing its scope to
  exclude the mutate step) must be killed by `concurrent_admits_never_oversubscribe_a_lane` — this
  is the single most important mutant this crate must never let survive.
- Flipping `escalate`'s rung-selection from highest-first to lowest-first must be killed by
  `escalate_ladder_returns_most_severe_matching_rung`.
- Swapping `settle`'s `checked_add`/`checked_sub` order (crediting before debiting, double-counting
  the estimate) must be killed by `admit_then_settle_round_trips_through_a_real_file`'s exact `used`
  assertion.

**Property tests:**
- *Admission never oversubscribes under any interleaving*: `proptest` over a random sequence of
  concurrent `admit`/`settle` calls (varied token amounts, varied thread interleavings via a small
  barrier) against one `FileMeterStore`, asserting the invariant `sum(open reservations) + used_base
  <= window` holds after every operation completes — the generalized form of
  `concurrent_admits_never_oversubscribe_a_lane`, at N = 200 cases minimum.

## 10. Verification recipe

```bash
cd crates/fleet-govern
cargo test -p fleet-govern --all-targets
cargo clippy -p fleet-govern --all-targets -- -D warnings
cargo mutants -p fleet-govern
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration + property tests pass, 0 skipped — publish as `<passed>/<total>`.
Clippy: 0 warnings. Mutants: publish `<caught>/<total>`; the lock-removal mutant on
`with_lane_locked` is non-negotiable — if it survives, this crate has not actually fixed the fan-out
race regardless of what the unit tests say (a proxy is not the property).

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`AdmitError`, `SettleError`, `MeterIoError`) —
      none swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code.
- [ ] Clock/RNG/IO are injected: `CooldownStore` takes `now`/`at` as parameters; `MeterStore` is the
      sole file-IO boundary; `Tokenizer` is the sole (pure) tokenizer boundary; no ambient
      `env::var`/`SystemTime::now()`/subprocess call anywhere in `crates/fleet-govern/src/` — verify
      with `grep -rn 'SystemTime::now\|env::var\|Command::new' crates/fleet-govern/src/` returning
      nothing outside `store_file.rs`'s lock-timeout use of `Instant` for the lock wait itself (if
      any; document there explicitly if used).
- [ ] Thread-safety documented: `MeterStore`/`CooldownStore` are `Send + Sync` trait objects;
      `FileMeterStore`'s atomicity comes from an OS advisory lock, not from Rust-level
      synchronization alone — say explicitly that this only protects against other *processes and
      threads going through this same trait impl*, not against another program editing
      `meter-v1.tsv` directly outside this crate.
- [ ] No float used for money, tokens, or any precision-sensitive count — `escalate`'s thresholds
      and comparisons are all `Tokens`, never a derived ratio.
- [ ] No self-grading — mutation testing run, not just unit tests; denominator published per §10.
- [ ] The verify command's pass/fail denominator is stated in this file (template placeholder here;
      restate the real numbers in the PR once built).
- [ ] Tests that touch the filesystem use `tempfile::tempdir()` exclusively (`cas_conflict.rs`,
      `admit_settle.rs`) — never the repo tree or `$HOME`.
- [ ] Every non-goal in §2 is actually absent from the code: no `Command::new` (capability probing
      stays in `fleet-worker`), no `fleet_store`/ledger-receipt call, no CLI arg parsing.
- [ ] No source file exceeds 80 lines — verified by the §10 `wc -l ... awk '$1>80'` gate.

## 12. Definition of Done

`fleet-govern` is DONE when: §10's four commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught including the non-negotiable lock-removal mutant) run from
`crates/fleet-govern/`; every unchecked box in §11 is checked with its real numbers;
`registry/services/REGISTRY.md` lists the crate (infrastructure, not a feature — C1/L2); and Opus has
re-derived the atomic-admission contract from this blueprint alone (without re-reading `meter.rs`),
reproduced the lock-removal mutation by hand against a real concurrent-thread test, and driven one
real `admit` → `settle` → `next_provider` sequence end-to-end confirming the final `used` matches
`actual`, not `estimated`.

---

## Divergence from MIGRATION-PLAN (for Opus)

MIGRATION-PLAN §3 row 11 describes `fleet-govern` as reuse from `budget.sh` (atomic admit) ·
`meter.rs` (token accounting) · `route.rs` failover — "add tiktoken-rs real counts." Having read all
three sources in full, two things need Opus's eyes:

1. **Neither `budget.sh` nor `meter.rs` is actually atomic today, despite the roster's "atomic
   admit" framing.** `budget.sh` is a stateless, single-shot CLI check (`invariant_i2` against two
   numbers the caller already computed) — it holds no reservation and touches no persisted state at
   all, so it cannot race, but it also cannot admit anything. `meter.rs::reserve` (`meter.rs:138-160`)
   *does* mutate persisted state, but `Meter::save`'s `rename` (`meter.rs:135`) is an unconditional
   overwrite with no lock and no compare-on-write — exactly the fan-out race the task brief names
   ("take the token before spawning children"). This blueprint builds the actual fix
   (`MeterStore::with_lane_locked`, an OS advisory-lock critical section) as new work, not an
   extraction. Row 11's evidence column should say "atomic admit: **not yet atomic in either
   source**, built new here" rather than implying the atomicity already exists somewhere to lift.

2. **The task brief's literal signature `admit(cost_est) -> Option<Reservation>` is implemented here
   as `Result<Reservation, AdmitError>` instead.** `Option` cannot distinguish "unmeasured window,"
   "unmeasured usage," and "measured but insufficient" — three meaningfully different refusals a
   caller's `escalate` ladder needs to tell apart (a `WindowUnknown` refusal should probably not
   trigger the same downgrade path as a measured `InsufficientBudget`). This follows this crate's own
   hard rule (§3's "never a bare `Option` standing in for an error") and `fleet-router`'s own
   convention of typed refusals-as-data; flagging it explicitly in case the brief's `Option` framing
   was meant as a firm interface constraint rather than shorthand for "may fail to admit."

3a. **(post-build) A whole multi-day autonomous-loop driver was added that this blueprint never
   sketched.** `AutonomousRun`/`tick`/`complete_unit` (`loop_run.rs`, `loop_complete.rs`) plus their
   supporting types (`LoopPlan`, `LoopProgress`, `UnitId`, `TickOutcome`, `LoopError`, `LoopStore`,
   `FileLoopStore`) compose this crate's own `next_provider`/`admit`/`escalate`/`settle` into a
   resumable state machine, persisted behind a fourth injected port (`LoopStore`, alongside
   `MeterStore`/`CooldownStore`/`Tokenizer`). Nothing in MIGRATION-PLAN row 11 or this blueprint's
   original brief describes a "multi-day loop" concept — the closest hint is this crate's header
   line ("...for multi-day fleet runs"), which reads as flavor text about *why* atomic admission
   matters, not as a scoped instruction to build a loop driver. This pass added §3's new
   "Multi-day autonomous loop" subsection and updated §1/§8 to describe what was actually built, but
   did not re-derive or second-guess the design (e.g. whether `tick` checking `escalate` only
   *after* `next_provider` has already picked a live, non-cooling-down adapter — rather than before,
   which could short-circuit a call to `next_provider` entirely when already known-exhausted — is
   the intended order). Flagged for Opus: this is a scope addition beyond the written contract, not
   a doc-staleness artifact, and it has no corresponding MIGRATION-PLAN row or reuse citation at all.
3. **`escalate`'s throttle→downgrade→cached→pause ladder has no fleet source at all** — it is
   genuinely `build-new`, not reflected anywhere in row 11's evidence column. This blueprint's design
   (integer `Tokens` thresholds, no float ratio) is this author's judgment call, not a lift; Opus
   should confirm the four-rung shape matches what the task brief's "escalate(spend) = throttle→
   downgrade→cached→pause ladder" actually intends operationally (in particular: does "cached" mean
   "serve from a response cache" — implying a cache port this blueprint does not yet define — or
   "prefer the cheapest/cached-capacity lane already warm"? This blueprint assumes the latter and
   leaves cache *selection* to `fleet-router`/`fleet-worker`, with `escalate` only signaling the
   caller should prefer it; if the former is intended, a `CacheStore` port belongs in §3 and this
   is a gap, not a design choice).
