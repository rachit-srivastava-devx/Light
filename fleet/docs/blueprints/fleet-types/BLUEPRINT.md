# BLUEPRINT — `fleet-types`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-types`
- **One-line purpose:** Own the plain data types, newtype identifiers, and typed errors that
  cross fleet crate boundaries — roles, task/module/lane identifiers, token counts, the lifecycle
  state vocabulary, the process exit-code taxonomy, and the ledger receipt / delivery-attestation
  wire shapes — with zero IO, zero clock, zero RNG, and zero decision logic of its own.
- **Build branch:** `refactor` (MIGRATION-PLAN §3 row 1) — the types exist today, but scattered,
  `pub(crate)`-private to the `fleet` monolith, and duplicated across files (five separate
  `EXIT_ENV`/`EXIT_ENVIRONMENT` redeclarations, a `Role` enum three crates need but only one can
  see). This crate collects them into one public, dependency-free base.
- **Imports:** `none` — this is the root of the DAG (`types → store → {siblings} → src/`).
- **Imported by:** every other crate in the roster (`fleet-store`, `fleet-events`, `fleet-router`,
  `fleet-scan`, `fleet-plan`, `fleet-context`, `fleet-verify`, `fleet-merge`, `fleet-memory`,
  `fleet-govern`, `fleet-stream`, `fleet-worker`, `src/`). `fleet-router` already depends on it
  today (its blueprint's §3 has `use fleet_types::Role;`) — this blueprint's `Role` API must be a
  superset of what that blueprint assumes, and is (see §5 note on `Role::parse`'s signature).

## 2. Responsibility & non-goals

**Owns:** the shared vocabulary — every plain data shape, newtype identifier, and typed error
that more than one fleet crate needs to name the same concept the same way. Concretely: `Role`
and its fixed scheduling/gate metadata; the `TaskId`/`NodeId`/`LaneId` identifier newtypes; the
`Tokens` integer-minor-unit count; the `LifecycleState` vocabulary (the 16 named stages a task
passes through, as data); the `ExitCode` taxonomy; and the `Receipt`/`Attestation` wire types that
must (de)serialize byte-for-byte against `fleet/contracts/receipt.v1.json` and
`fleet/contracts/attestation.v1.json`. If two crates would otherwise each define their own copy of
the same concept to talk to each other, that concept belongs here instead.

**Non-goals (the seam):**
- Does **not** decide anything — no routing decision (`fleet-router`), no scheduling/allocation
  decision (`fleet-govern`), no gate verdict (`fleet-verify`). This crate supplies the nouns;
  deciding what to do with them is every other crate's job.
- Does **not** perform the `Role`/`Check` safety-gate evaluation (`LEAD_WROTE_CODE`,
  `SELF_VERIFIED`) — that logic already lives in `fleet-router` as `RoleRefusal`/
  `evaluate_role_check` (its blueprint is `blueprint-done`, Opus-approved). This crate supplies
  `Role` itself; it does not re-derive or duplicate the check built on top of it. See the
  divergence note at the end of this file — MIGRATION-PLAN §3 row 1's evidence column reads as if
  the whole `Check`/`evaluate` primitive re-homes here, and it does not.
- Does **not** own the phantom-type/sealed-trait compile-time state-machine (`lifecycle.rs`'s
  `Task<S>`, its marker structs, `ReceiptLedger`, `TransitionReceipt`) — only the *data* half (the
  named states as an enum) lives here. See the divergence note: no crate in the current roster is
  named as the owner of the machine itself, and this blueprint does not invent one.
- Does **not** append to the ledger, read the meter state file, probe an adapter, or touch the
  filesystem/network/subprocess under any circumstance — every type here is constructed from a
  value already in memory (a parsed string, a deserialized JSON object) supplied by the caller.
- Does **not** know *how* a `Receipt`'s `body` is shaped for any particular `event` value — `body`
  is `serde_json::Value` precisely because that knowledge belongs to whichever crate emits that
  event, not to this one.

## 3. Public API contract

```rust
//! Pure shared vocabulary at the root of the fleet crate DAG.
//!
//! `fleet-types` imports nothing from any other `fleet-*` crate and is imported by nearly all of
//! them (`types -> store -> {siblings} -> src/`). It owns the plain data types, newtype
//! identifiers, and typed errors that cross crate boundaries. It performs no IO, spawns no
//! process, reads no clock, and makes no decision -- every fn here is a pure, total value
//! transformation (parse / format / validate / compare). If a future change needs any of those,
//! that change belongs in a sibling crate, not here.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// =====================================================================================
// A. Roles -- fleet/keel/fleet/src/roles.rs:1-65
// =====================================================================================

/// One of fleet's five swarm roles. Re-homed from `fleet/keel/fleet/src/roles.rs:1-8`, which
/// today declares this `pub(crate)` -- private to the monolith -- even though `route.rs`'s
/// `decide()` (destined for `fleet-router`), `console.rs`'s dashboard, and `main.rs`'s CLI
/// dispatch all depend on it. This is the base-crate seam that closes that gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Lead,
    Builder,
    Verifier,
    Designer,
    Meter,
}

/// `Role::parse` could not match `value` against any of the five lowercase wire names.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown role {0:?}: expected one of lead, builder, verifier, designer, meter")]
pub struct UnknownRole(pub String);

impl Role {
    /// All five roles, in the fixed dashboard/receipt order. Mirrors `roles.rs:67-73`'s `ALL`.
    pub const ALL: [Role; 5] = [Role::Lead, Role::Builder, Role::Verifier, Role::Designer, Role::Meter];

    /// Parse a role from its lowercase wire name (`"lead"`, `"builder"`, ...). Mirrors
    /// `roles.rs:11-20`'s `parse`, but returns a typed error instead of `Option` (this crate's
    /// every-fallible-op-is-typed rule) -- a caller that only wants presence/absence can `.ok()`
    /// the result. Never case-folds: `"Lead"`/`"LEAD"` are both rejected, matching today's
    /// behavior exactly (callers normalise before parsing, they do not rely on this fn to).
    pub fn parse(value: &str) -> Result<Self, UnknownRole> { unimplemented!() }

    /// The lowercase wire name, matching `fleet/contracts/lane-status.v1.json:20`'s `role` enum
    /// and `roles.rs:22-30`.
    pub fn name(self) -> &'static str { unimplemented!() }

    /// Fixed scheduling weight consumed by `fleet-govern`'s allocator. Verbatim policy numbers
    /// from `roles.rs:32-40` -- not derived; changing them is a scheduling policy change owned
    /// by `fleet-govern`, not this crate.
    pub fn bandwidth(self) -> u64 { unimplemented!() }

    /// Fixed scheduling fitness score consumed by `fleet-govern`'s allocator. Verbatim from
    /// `roles.rs:42-50`.
    pub fn allocation_fitness(self) -> u64 { unimplemented!() }

    /// The gate name this role has sole authority over (`"contract"`, `"implementation"`, ...).
    /// Mirrors `roles.rs:52-60`, made `pub` (was private `owned_gate`) -- `fleet-govern`'s gate
    /// dispatch and `fleet-store`'s receipt validation both need this fact and today cannot
    /// reach it (it is private to the monolith).
    pub fn owned_gate(self) -> &'static str { unimplemented!() }

    /// Whether this role may submit a diff that adds implementation code. Mirrors
    /// `roles.rs:62-64`, made `pub` (was private `may_write_code`). Only `Role::Builder` returns
    /// `true` -- this is the fact `fleet-router`'s `RoleRefusal::LeadWroteCode` is built on (see
    /// the divergence note).
    pub fn may_write_code(self) -> bool { unimplemented!() }
}

// =====================================================================================
// B. Identifiers -- newtypes for the bare `String` ids that cross a crate boundary today
// =====================================================================================

/// A field that was empty or all-whitespace where a non-empty identifier was required.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{field} must not be empty")]
pub struct EmptyIdentifier {
    pub field: &'static str,
}

/// A `node_id` that did not match `fleet/contracts/module-brief.v1.json:138`'s pattern
/// (`^[a-z0-9][a-z0-9-]{2,63}$`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("node_id must match ^[a-z0-9][a-z0-9-]{{2,63}}$")]
pub struct BadNodeId;

/// Stable identifier carried through every lifecycle transition of one task. Lifted from
/// `fleet/keel/fleet/src/lifecycle.rs:95-117`'s `TaskId`, which already enforces non-empty on
/// construction -- unchanged here, just made a shared type instead of one crate's private one.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyIdentifier> { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}

/// Stable identifier for a module/leaf brief. Matches `module-brief.v1.json:138`'s `node_id`:
/// lowercase alphanumeric + hyphen, 3-64 characters total, must not start with a hyphen. Today
/// nothing in fleet types this field at all (`main.rs` and friends pass it around as bare JSON
/// string values inside `serde_json::Value` bodies) -- this is new shared vocabulary the plan
/// asks for ("task/module identifiers"), not an extraction.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(String);

impl NodeId {
    pub fn parse(value: impl Into<String>) -> Result<Self, BadNodeId> { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}

/// Stable identifier for a lane within one dispatch (today the role name; see
/// `fleet/contracts/lane-status.v1.json:13-17` and `console.rs:170`'s bare `lane_id: String`
/// field). Newtyped here so `fleet-store`'s lane-status projection and `fleet-stream`'s
/// dashboard share one non-empty-enforced type instead of two crates each trusting a bare
/// `String` never to be empty.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LaneId(String);

impl LaneId {
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyIdentifier> { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}

/// Shared artifact-id shape check: exactly 64 lowercase hex characters. Lifted from
/// `fleet/keel/fleet/src/main.rs:2271-2277`'s inline check, made a free fn (not a newtype
/// constructor) because today's callers pass the id around as a bare `String` in several places
/// and only need a yes/no predicate, not an owned wrapper type.
pub fn valid_artifact_id(id: &str) -> bool { unimplemented!() }

// =====================================================================================
// C. Token accounting -- integer minor units, never float
// =====================================================================================

/// A count of language-model tokens. Wraps the bare `u64` that `meter.rs`'s `Lane.window`/
/// `.used`/`.reservations` (`meter.rs:19-21`) and `fleet-router`'s `RuntimeState.remaining`/
/// `.required_tokens` use today, so every crate moving a token count across a boundary shares one
/// overflow-checked type instead of re-deriving `checked_add`/`checked_sub` per call site (see
/// `meter.rs:150,157,187,190` for today's ad hoc, per-call-site `checked_sub`/`checked_add`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tokens(u64);

/// A `Tokens` arithmetic operation would have overflowed `u64`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("token arithmetic overflowed u64")]
pub struct TokensOverflow;

impl Tokens {
    pub const ZERO: Tokens = Tokens(0);
    pub fn new(value: u64) -> Self { Tokens(value) }
    pub fn get(self) -> u64 { self.0 }
    pub fn checked_add(self, rhs: Tokens) -> Result<Tokens, TokensOverflow> { unimplemented!() }
    pub fn checked_sub(self, rhs: Tokens) -> Result<Tokens, TokensOverflow> { unimplemented!() }
}

// =====================================================================================
// D. Lifecycle state vocabulary -- fleet/keel/fleet/src/lifecycle.rs:16-33
// =====================================================================================

/// The 16 lifecycle states a fleet task passes through, as plain serializable data -- the
/// vocabulary every crate that logs, dashboards, or receipts a task's current stage shares.
/// This is deliberately NOT the phantom-type marker mechanism (`lifecycle.rs`'s `Intake`/
/// `Specified`/... zero-sized structs + sealed `State` trait, `lifecycle.rs:35-93`) -- that
/// compile-time state machine is tied one-to-one to whichever crate owns `Task<S>`'s transition
/// methods, and no crate in the current roster is named as that owner (see the divergence note).
/// This enum is the data half every consumer needs regardless of who ends up owning the machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LifecycleState {
    Intake, Specified, Reviewed, Decomposed, Contracted, Briefed, Leased, Building, Built,
    Verifying, Verified, Attested, Accepted, Proposed, Observed, Refused,
}

impl LifecycleState {
    /// The wire name, matching `lifecycle.rs:16-33`'s state names and `TransitionReceipt.from`/
    /// `.to` (`lifecycle.rs:120-126`, today a bare `&'static str`).
    pub fn name(self) -> &'static str { unimplemented!() }

    /// The states legal to transition to from `self`. Mirrors the `STATES` table
    /// (`lifecycle.rs:16-33`) verbatim -- e.g. `Building.allowed_next() == &[Built, Refused]`.
    /// A pure data lookup; it enforces nothing by itself. Whichever crate ends up owning
    /// `Task<S>` must keep its own marker-type transitions in lockstep with this table (a unit
    /// test there should assert the two never drift — see §9's cross-crate note).
    pub fn allowed_next(self) -> &'static [LifecycleState] { unimplemented!() }
}

/// A failed gate check or a failed attempt to append a transition's evidence. Lifted from
/// `lifecycle.rs:134-163` (there named `Refusal`; disambiguated here as `GateRefusal` because
/// `fleet-router` already owns a distinct two-variant `RoleRefusal` for its own safety-gate
/// enum — this is the general `{code, message}` shape every other refusal site uses).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GateRefusal {
    code: &'static str,
    message: String,
}

impl GateRefusal {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self { unimplemented!() }
    pub fn code(&self) -> &'static str { self.code }
    pub fn message(&self) -> &str { unimplemented!() }
}

// =====================================================================================
// E. Process exit-code taxonomy
// =====================================================================================

/// Fleet's process exit-code taxonomy, collapsing the `EXIT_*` constants redeclared today in at
/// least 14 files (`main.rs:32-37`, `agent.rs:24-25`, `console.rs:24-25`, `graph.rs:12-14`,
/// `lifecycle.rs:13-14`, `mcp.rs:15-16`, `meter.rs:13-14`, `ratchet.rs:8-11`, `repl.rs:27`,
/// `route.rs:12-13`, `skills.rs:9-10`, `sow.rs:13-16`, `status.rs:12`, `worktree.rs:22-23`) — two
/// of which even spell the same number differently (`EXIT_ENV` vs `EXIT_ENVIRONMENT`, both `= 3`,
/// e.g. `agent.rs:24` vs `lifecycle.rs:13`). This enum is the single source of truth those 30+
/// individual `const` declarations collapse into.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ExitCode {
    /// `main.rs:32`. Success.
    Ok = 0,
    /// `main.rs:33` / `agent.rs:24` / `graph.rs:12` / `worktree.rs:22` (`EXIT_ENV`), also spelled
    /// `EXIT_ENVIRONMENT` in `lifecycle.rs:13` / `meter.rs:13` / `sow.rs:14`. The environment
    /// (filesystem, env var, subprocess) did not provide what the caller needed — not a logic bug.
    Env = 3,
    /// `main.rs:34` / `agent.rs:25` / `route.rs:12` / `worktree.rs:23` (`EXIT_INVARIANT`). An
    /// internal invariant assumed to always hold did not hold (corrupt state, overflow, a
    /// should-never-happen branch).
    Invariant = 6,
    /// `main.rs:35` / `lifecycle.rs:14` / `route.rs:13` / `mcp.rs:16` (`EXIT_REFUSAL`). A gate
    /// deliberately refused — the expected, auditable "no" outcome, never an error.
    Refusal = 7,
    /// `main.rs:36` / `ratchet.rs:11` / `sow.rs:16` (`EXIT_MISMATCH`). A comparison against a
    /// committed baseline (mutation floor, hash chain) did not match.
    Mismatch = 8,
    /// `sow.rs:13`, `EXIT_READY_AWAITING_REVIEW`. Not a failure — the SOW is ready and is waiting
    /// on a human review gate before proceeding.
    ReadyAwaitingReview = 9,
}

/// An `i32` did not match any known `ExitCode` variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0} is not a recognised fleet exit code")]
pub struct UnknownExitCode(pub i32);

impl ExitCode {
    pub fn as_i32(self) -> i32 { self as i32 }
}

impl TryFrom<i32> for ExitCode {
    type Error = UnknownExitCode;
    fn try_from(value: i32) -> Result<Self, Self::Error> { unimplemented!() }
}

// =====================================================================================
// F. Ledger receipt wire type -- fleet/contracts/receipt.v1.json
// =====================================================================================

/// A blake3 digest in the ledger's wire format: exactly `"blake3:"` followed by 64 lowercase hex
/// characters. Matches `receipt.v1.json:12`'s `hash` pattern.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Blake3Hash(String);

/// A string did not match `"blake3:" + 64 lowercase hex chars`.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0:?} is not a well-formed blake3 hash")]
pub struct BadBlake3Hash(pub String);

impl Blake3Hash {
    pub fn parse(value: impl Into<String>) -> Result<Self, BadBlake3Hash> { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}
impl TryFrom<String> for Blake3Hash {
    type Error = BadBlake3Hash;
    fn try_from(value: String) -> Result<Self, Self::Error> { unimplemented!() }
}
impl From<Blake3Hash> for String {
    fn from(hash: Blake3Hash) -> String { unimplemented!() }
}

/// `receipt.v1.json`'s `prev_hash`: either the ledger genesis marker or a prior row's hash.
/// Matches `"^(GENESIS|blake3:[0-9a-f]{64})$"` by construction — no third value is
/// representable. `Serialize`/`Deserialize` are hand-written (not derived): the wire form is a
/// plain string (`"GENESIS"` or `"blake3:..."`), and a derived externally-tagged enum would
/// instead nest under a variant-name key, which the schema forbids (`additionalProperties:
/// false`, no such nesting in `receipt.v1.json`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrevHash {
    Genesis,
    Hash(Blake3Hash),
}

impl PrevHash {
    pub fn parse(value: impl Into<String>) -> Result<Self, BadBlake3Hash> { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}
impl Serialize for PrevHash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> { unimplemented!() }
}
impl<'de> Deserialize<'de> for PrevHash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> { unimplemented!() }
}

/// `receipt.v1.json`'s `event` enum (`receipt.v1.json:15`). Includes `Rollback`, an 8th variant
/// surfaced by MIGRATION-PLAN §7 (fleet's whitelist `main.rs:4373-4386` stores `"rollback"`) —
/// not present in the schema's original 7-name enum as first drafted; added during the build so
/// the type stays a total mirror of what the ledger actually accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptEvent {
    RunStart,
    ArtifactFrozen,
    Attested,
    Refusal,
    GateVerdict,
    RunEnd,
    LaneStatus,
    Rollback,
}

/// The fixed `"1.0"` schema-version marker for `Receipt`. A zero-sized type instead of `String`
/// so a `Receipt` cannot be constructed carrying any other schema version at all — the illegal
/// state is unrepresentable, not merely rejected at runtime. `Serialize`/`Deserialize` are
/// hand-written to produce/require exactly the JSON string `"1.0"`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct SchemaV1;
impl Serialize for SchemaV1 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> { unimplemented!() }
}
impl<'de> Deserialize<'de> for SchemaV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> { unimplemented!() }
}

/// One append-only ledger row, matching `fleet/contracts/receipt.v1.json` field-for-field
/// (`additionalProperties: false` in the schema — this struct's field set IS the schema's field
/// set; adding a field here without updating the `.json` contract in the same commit is a
/// divergence, not a private extension, per PLAYBOOK.md rule 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub schema_version: SchemaV1,
    pub seq: u64,
    pub prev_hash: PrevHash,
    pub hash: Blake3Hash,
    /// STAMPED BY THE LEDGER, per the schema's own description (`receipt.v1.json:14`) — never
    /// constructed by a worker. Kept as an RFC3339 `String`, not parsed into a calendar type,
    /// because this crate does no time arithmetic and injects no clock (§4); a sibling that
    /// needs arithmetic parses it with its own chosen date/time library.
    pub ts_wall: String,
    pub event: ReceiptEvent,
    /// STAMPED BY THE LAUNCHER (schema's own description, `receipt.v1.json:17`) — never declared
    /// by the launched process.
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub resolved_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub exit_code: Option<ExitCode>,
    /// "the only part a worker authors" (schema's own description, `receipt.v1.json:20`) —
    /// deliberately untyped: every `event` kind puts different fields here, and this crate does
    /// not know every event's body shape; that knowledge belongs to whichever crate emits it.
    pub body: Value,
}

// =====================================================================================
// G. Delivery attestation wire type -- fleet/contracts/attestation.v1.json
// =====================================================================================

/// `attestation.v1.json`'s `predicate.tier` enum (`attestation.v1.json:28`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DeliveryTier {
    #[serde(rename = "T-min")]
    TMin,
    #[serde(rename = "T-std")]
    TStd,
    #[serde(rename = "T-max")]
    TMax,
}

/// A 64-lowercase-hex blake3 digest with NO `"blake3:"` prefix — `attestation.v1.json:18`'s
/// `digest.blake3` pattern, distinct from `Blake3Hash` (which requires the prefix) because the
/// two contracts genuinely disagree on wire format for the same hash function.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BareBlake3Digest(String);
impl BareBlake3Digest {
    pub fn parse(value: impl Into<String>) -> Result<Self, BadBlake3Hash> { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}
impl TryFrom<String> for BareBlake3Digest {
    type Error = BadBlake3Hash;
    fn try_from(value: String) -> Result<Self, Self::Error> { unimplemented!() }
}
impl From<BareBlake3Digest> for String {
    fn from(digest: BareBlake3Digest) -> String { unimplemented!() }
}

/// `attestation.v1.json`'s `subject[]` entries (`attestation.v1.json:10-20`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttestationSubject {
    pub name: String,
    pub digest: AttestationDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttestationDigest {
    pub blake3: BareBlake3Digest,
}

/// `attestation.v1.json`'s `predicate.builder` (`attestation.v1.json:29`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttestationBuilder {
    pub id: String,
}

/// `attestation.v1.json`'s `predicate.elements` (`attestation.v1.json:30-43`). Every field is
/// `Option<Value>` because the schema declares no required keys under `elements` at all (only
/// `oracle_independence` carries a schema *comment* marking it non-droppable — a fact recorded
/// here as a doc comment, not as a type-level requirement, since the schema itself does not
/// enforce it either — see §4).
#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct AttestationElements {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sow: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blind_suite: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub independent_verification: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub adequacy: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blast_radius: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rollback: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cost: Option<Value>,
    /// "element 8 — NON-DROPPABLE AT EVERY TIER" (`attestation.v1.json:42`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub oracle_independence: Option<Value>,
}

/// `attestation.v1.json`'s `predicate` object (`attestation.v1.json:23-47`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeliveryPredicate {
    pub tier: DeliveryTier,
    pub builder: AttestationBuilder,
    #[serde(default)]
    pub elements: AttestationElements,
    pub receipts: Vec<String>,
}

/// The fixed in-toto statement/predicate type markers (`attestation.v1.json:9,22`). Same
/// unrepresentable-by-construction treatment as `Receipt`'s `SchemaV1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct InTotoStatementV1;
impl Serialize for InTotoStatementV1 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> { unimplemented!() }
}
impl<'de> Deserialize<'de> for InTotoStatementV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> { unimplemented!() }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct DeliveryAttestationV1;
impl Serialize for DeliveryAttestationV1 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> { unimplemented!() }
}
impl<'de> Deserialize<'de> for DeliveryAttestationV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> { unimplemented!() }
}

/// The full in-toto Statement carrying fleet's Delivery predicate
/// (`fleet/contracts/attestation.v1.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attestation {
    #[serde(rename = "_type")]
    pub statement_type: InTotoStatementV1,
    #[serde(rename = "predicateType")]
    pub predicate_type: DeliveryAttestationV1,
    pub subject: Vec<AttestationSubject>,
    pub predicate: DeliveryPredicate,
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `Role` | Exactly 5 variants, exhaustively matched everywhere a role maps to a fixed fact (`bandwidth`, `allocation_fitness`, `owned_gate`, `may_write_code`). | A role silently having no scheduling weight or no owned gate — the compiler forces every match to cover all 5. |
| `TaskId` / `NodeId` / `LaneId` | Non-empty after trimming (`TaskId`/`LaneId`); `NodeId` additionally matches `^[a-z0-9][a-z0-9-]{2,63}$`. | An identifier that is silently the empty string flowing into a receipt, a ledger key, or a lane dashboard row and colliding with every other empty identifier. |
| `Tokens` | Always constructed from a `u64`; every combining operation is `checked_*`, returning a typed `TokensOverflow` instead of wrapping or panicking. | A token count silently wrapping past `u64::MAX` and appearing to *shrink*, which would let a caller believe a quota-exhausted lane still had budget. |
| `LifecycleState` | Exactly the 16 named states from `lifecycle.rs`'s `STATES` table; `allowed_next` is a total function over all 16 (no state is missing an entry, including `Refused`, whose only legal next state is "none"). | A state name that doesn't correspond to any of the 16 real stages appearing in a receipt or dashboard — the enum's variants ARE the closed set. |
| `GateRefusal` | `code` is always a `&'static str` (compiled-in, not user-supplied) even though `message` is a caller-supplied `String`. | A refusal code that varies at runtime per call site, which would make refusal codes ungrep-able across the ledger. |
| `ExitCode` | Exactly the 6 values fleet's `EXIT_*` constants use (`0,3,6,7,8,9`); `TryFrom<i32>` is the only path from a raw process exit code into this type and it is fallible. | A 7th, invented exit code silently being treated as one of the 6 known ones (e.g. code `4` — never defined anywhere in fleet today — being coerced into `Env` by an off-by-one bug). |
| `Blake3Hash` / `BareBlake3Digest` | Constructed only via `TryFrom<String>`/`parse`, which checks the exact character class and length; `PrevHash`'s two variants are the only two representable prev-hash values. | A malformed or truncated hash (63 hex chars, uppercase hex, missing `blake3:` prefix) silently passing through as a valid ledger reference. |
| `Receipt` | Field set matches `receipt.v1.json`'s `required` + optional keys exactly; `schema_version: SchemaV1` cannot hold any string but `"1.0"`; `resolved_model`/`exit_code` round-trip as absent (not `null`) when `None`, matching the schema's `additionalProperties: false` + optional-key semantics. | A `Receipt` claiming a schema version fleet hasn't shipped, or a serialized receipt carrying an extra key the ledger's validator would reject. |
| `Attestation` / `DeliveryPredicate` / `AttestationElements` | `_type`/`predicateType` cannot hold any value but the two fixed in-toto/fleet constants; `tier` is exactly `T-min`/`T-std`/`T-max`; `elements`' 8 named slots are the only slots (`additionalProperties: false`) — no 9th element can be silently added by a typo. | An attestation claiming a `predicateType` other than fleet's Delivery predicate, or an `elements` object with a misspelled key that a hand-rolled `serde_json::Value` blob would have silently accepted and then silently dropped downstream. |

**Money/precision:** no money type in this crate (grep of `fleet/keel/fleet/src/meter.rs` and
`main.rs` found no dollar/cent accounting — only Wilson-score `f64` *rates*, which are a
measurement statistic, not money, and stay in whichever crate computes them, likely `fleet-verify`
or `fleet-govern`). Token counts (`Tokens`) are `u64` throughout, never float.

**Clock/RNG/IO injection points:** none — this crate is pure. Every constructor takes an
already-in-memory value (a `&str`, a `String`, a deserialized `Value`) and every accessor reads
only `self`. `Receipt.ts_wall` is a `String` precisely so this crate never has to decide *how* to
read or parse wall-clock time — that decision belongs to whichever crate stamps or consumes it.

## 5. Reuse map

Source read in full: `fleet/keel/fleet/src/roles.rs` (168 lines, 2026-09-08), `fleet/keel/fleet/
src/agent.rs` (476 lines), `fleet/keel/fleet/src/lifecycle.rs` (first 200 of 1478 lines — the
`STATES` table, marker structs, `TaskId`, `Refusal`; the remaining ~1280 lines are `Task<S>`'s
transition methods, out of scope per §2), `fleet/contracts/receipt.v1.json`, `fleet/contracts/
attestation.v1.json`, `fleet/contracts/module-brief.v1.json`, `fleet/contracts/lane-status.v1.json`,
and a repo-wide `grep -rn "^const EXIT_"` across `fleet/keel/fleet/src/*.rs`.

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `roles.rs:1-8` (`Role`) | 5-variant role enum. | yes | `pub` instead of `pub(crate)`; add `Serialize`/`Deserialize`/`Ord`/`Hash` (needed for `BTreeMap`/`BTreeSet` keys and receipt round-tripping — none of fleet's own uses needed these yet because nothing outside the monolith could reach `Role` to serialize it). |
| `roles.rs:11-20` (`parse`) | `&str -> Option<Role>`. | logic yes, signature no | Return `Result<Role, UnknownRole>` instead of `Option` — this crate's hard rule against bare `Option` standing in for an error. Behavior (which strings match, no case-folding) is unchanged. |
| `roles.rs:22-30` (`name`) | `Role -> &'static str`. | yes | `pub` instead of `pub(crate)`. |
| `roles.rs:32-40` (`bandwidth`) | Fixed per-role `u64` weight. | yes, verbatim | `pub`. |
| `roles.rs:42-50` (`allocation_fitness`) | Fixed per-role `u64` score. | yes, verbatim | `pub`. |
| `roles.rs:52-60` (`owned_gate`) | Fixed per-role gate name, private `fn`. | yes | `pub` (was unreachable outside `roles.rs`). |
| `roles.rs:62-64` (`may_write_code`) | Fixed per-role bool, private `fn`. | yes | `pub`. |
| `roles.rs:67-73` (`ALL`) | `[Role; 5]` constant. | yes, verbatim | `pub const ALL`. |
| `roles.rs:75-110` (`Check`, `Refusal`, `evaluate`) | The two safety-gate rules. | **no — already lives in `fleet-router`** | `fleet-router`'s Opus-approved blueprint re-homes these as `RoleRefusal`/`evaluate_role_check` (its own §3, §5 row "roles.rs:83-110"). Duplicating them here would give two crates two different `Role`-adjacent refusal types for the same two rules — a defect, not redundancy. See divergence note. |
| `lifecycle.rs:13-14` (`EXIT_ENVIRONMENT`, `EXIT_REFUSAL`) | Two of the six exit codes, redeclared. | folded into `ExitCode` | See the `ExitCode` E section — collapses 30+ `const` redeclarations, cited per-file in §3's doc comments. |
| `lifecycle.rs:16-33` (`STATES`) | `&[(&str, &[&str])]` legal-transition table. | yes, reshaped | Becomes `LifecycleState::allowed_next`, a match returning `&'static [LifecycleState]` instead of a `(name, names)` tuple table — same data, typed instead of stringly. |
| `lifecycle.rs:35-93` (`sealed::Sealed`, `State`, 16 marker structs) | Phantom-type compile-time state machine. | **no** | This is the type-state *mechanism*, not the state *vocabulary* — out of scope per §2 (see divergence note; no crate in the roster currently owns `Task<S>`). |
| `lifecycle.rs:95-117` (`TaskId`) | Non-empty-validated `String` newtype. | yes, verbatim behavior | Signature-compatible: `new` renamed `parse` for naming consistency with this crate's other identifier types; still returns a typed error (was already `Result<Self, Refusal>`, now `Result<Self, EmptyIdentifier>` since `lifecycle::Refusal`'s `{code, message}` shape is overkill for one fixed failure mode). |
| `lifecycle.rs:120-126` (`TransitionReceipt`) | `{task_id, from: &'static str, to: &'static str, evidence}`. | **no** | Stays with `Task<S>`'s owner — it directly references the (out-of-scope) marker-type transition, not just the state name. |
| `lifecycle.rs:129-131` (`ReceiptLedger` trait) | The injected IO boundary for appending a transition receipt. | **no** | An injection-point trait belongs beside the code that calls it (`Task<S>`'s owner), not in the pure-types crate that never calls it. |
| `lifecycle.rs:134-163` (`Refusal`) | `{code: &'static str, message: String}` + accessors. | yes, renamed | `GateRefusal` here (see §3 for why — name collision avoidance with `fleet-router`'s `RoleRefusal`). |
| `lifecycle.rs:165-186` (`HumanApproval`) | Capability token only mintable inside `lifecycle.rs` (`pub(crate)` constructor). | **no** | Deliberately not reachable outside its owning crate today (`pub(crate) fn recorded`) — re-homing it here would make it constructible from every crate that depends on `fleet-types`, which is the opposite of its whole purpose. Stays wherever `Task<S>` ends up. |
| `agent.rs:127-175` (`Scorecard`) | `{agent_id, charter, credited, faulted, unknown, checked, total}` + arithmetic. | **no, considered and rejected** | This looks like shared vocabulary at a glance, but every field's arithmetic (`record_credited`/`refresh`) is entangled with `agent.rs`'s file-locking read-modify-write (`record_scorecard_outcome`, `agent.rs:228-289`) — it is IO-adjacent state, not a pure value type, and belongs with whichever crate re-homes `agent.rs` (likely `fleet-worker`, per MIGRATION-PLAN row 13's `agent.rs` citation). |
| `agent.rs:24-25` (`EXIT_ENV`, `EXIT_INVARIANT`) | Two more exit-code redeclarations. | folded into `ExitCode` | Same as `lifecycle.rs`'s. |
| repo-wide `grep -rn "^const EXIT_"` (14 files) | 6 distinct numeric values, ≥30 redeclarations, 2 spellings of the `=3` constant. | folded into `ExitCode` | Every citation is in §3's `ExitCode` doc comments; this table does not re-list all 30 — see there. |
| `fleet/contracts/receipt.v1.json` (whole file) | JSON-Schema wire contract for one ledger row. | greenfield Rust mirror | `Receipt`/`ReceiptEvent`/`PrevHash`/`Blake3Hash`/`SchemaV1` in §3 F — field-for-field, cited by line in each doc comment. |
| `fleet/contracts/attestation.v1.json` (whole file) | JSON-Schema wire contract for the Delivery attestation. | greenfield Rust mirror | `Attestation`/`DeliveryPredicate`/`AttestationElements`/`DeliveryTier`/`AttestationSubject` in §3 G. |
| `fleet/contracts/module-brief.v1.json:138` (`node_id` pattern) | Regex constraint on module identifiers. | greenfield Rust mirror | `NodeId::parse`. |
| `fleet/contracts/lane-status.v1.json:13-17` (`lane_id`) | Bare non-empty-string field, no dedicated type anywhere. | greenfield Rust mirror | `LaneId::parse`. |
| `meter.rs:19-21` (`Lane.window`/`.used`/`.reservations: Option<u64>`/`Vec<u64>`) | Bare `u64` token counts, ad hoc `checked_add`/`checked_sub` at each of 6+ call sites (`meter.rs:150,157,187,190,287,316,368,439`). | reshaped | `Tokens` newtype centralises the overflow-checked arithmetic; `meter.rs`'s own re-homing (`fleet-govern`, MIGRATION-PLAN row 11) switches its fields to `Tokens`/`Option<Tokens>` when it lands, not part of this crate's work. |

## 6. Behavior spec

### `fn Role::parse(value: &str) -> Result<Role, UnknownRole>`

| Input dimension | Behavior |
|---|---|
| empty | `""` matches no variant → `Err(UnknownRole(String::new()))`. |
| null / `None` | n/a — takes `&str`, not `Option<&str>`; an absent role is the caller's problem before calling this fn. |
| wrong-type | n/a — no type erasure at this boundary; the only "wrong type" is a non-matching string, covered by "empty"/duplicate rows. |
| huge | A 10,000-character string that isn't one of the 5 names → `Err(UnknownRole(<the full string>))`, no truncation, no panic — `UnknownRole` owns the `String` so the caller can inspect exactly what was rejected. |
| negative | n/a — not numeric. |
| duplicate | n/a — no collection of roles is being parsed, one string in, one `Role` or one error out. |
| concurrent | Pure value fn, no shared state — trivially safe from any number of threads. |
| unicode / non-ASCII | `"léad"`/`"LEAD"`/`"lead "` (trailing space) all fail to byte-match `"lead"` exactly → `Err(UnknownRole(...))`, never normalized, never trimmed — matches `roles.rs:11-20`'s existing exact-match behavior. |
| already-exists | n/a — no persisted state. |
| partial-failure | n/a — no IO, cannot fail partially. |

### `fn Tokens::checked_add(self, rhs: Tokens) -> Result<Tokens, TokensOverflow>`

| Input dimension | Behavior |
|---|---|
| empty | `Tokens::ZERO.checked_add(Tokens::ZERO)` → `Ok(Tokens::ZERO)`. |
| null / `None` | n/a — both operands are always present `Tokens` values, never `Option`. |
| wrong-type | n/a — `u64`-backed, no type erasure. |
| huge | `Tokens::new(u64::MAX).checked_add(Tokens::new(1))` → `Err(TokensOverflow)`, never wraps to `0`. |
| negative | n/a — `u64` cannot represent a negative count; `Tokens::new` takes `u64`, so a negative literal is a compile error at the call site, not a runtime case. |
| duplicate | n/a — not identity-bearing. |
| concurrent | `Copy` value type, no shared state — trivially safe. |
| unicode / non-ASCII | n/a — no string involved. |
| already-exists | n/a — no persisted state. |
| partial-failure | n/a — no IO. |

### `fn Blake3Hash::parse(value: impl Into<String>) -> Result<Blake3Hash, BadBlake3Hash>` (and `BareBlake3Digest::parse`, `PrevHash::parse` — same shape, prefix differs)

| Input dimension | Behavior |
|---|---|
| empty | `""` → `Err(BadBlake3Hash(String::new()))` (fails the length/prefix check). |
| null / `None` | n/a — takes an owned `String`-convertible value, not `Option`. |
| wrong-type | n/a at the Rust boundary; at the *serde* boundary (`#[serde(try_from = "String")]`), a JSON number or object where a string was expected is a `serde_json` deserialization error before this fn is ever called — covered by the crate's serde round-trip tests (§9), not by this fn's own signature. |
| huge | A 10,000-character string → `Err`, `BadBlake3Hash` carries the full (unbounded) rejected string; no truncation, no panic, no unbounded allocation beyond the input's own length. |
| negative | n/a — not numeric. |
| duplicate | n/a — not a collection operation. |
| concurrent | Pure value fn — trivially safe. |
| unicode / non-ASCII | `"blake3:" + 64 unicode characters (e.g. full-width hex-look-alikes)` → `Err`, because the digest-character check is ASCII-hex-only (`[0-9a-f]`); a byte-length check alone would wrongly accept multi-byte UTF-8 sequences that only *look* like the right length — the parse must count **characters**, not bytes, and must fail unicode digests explicitly (a concrete case in §9's test plan, not left implicit). |
| already-exists | n/a — no persisted state. |
| partial-failure | n/a — no IO. |

### `Receipt` / `Attestation` (de)serialization round-trip (`serde_json::to_string`/`from_str`)

| Input dimension | Behavior |
|---|---|
| empty | `Receipt.body = json!({})` → round-trips as `"body":{}`, not omitted (the schema requires `body` present, just allows it to be an empty object). `Attestation.predicate.elements` with every field `None` → serializes as `"elements":{}` (all 8 keys omitted via `skip_serializing_if`), which the schema's `additionalProperties: false` on `elements` still accepts (zero of the optional keys present is legal). |
| null / `None` | `Receipt.resolved_model: None` / `.exit_code: None` → both keys **absent** from the serialized JSON (not present as `null`) — matches `receipt.v1.json`'s `["string","null"]`/`["integer","null"]` typing, which permits `null` OR absence, and this crate chooses absence to keep wire output minimal; deserializing a payload that instead sent literal `null` for either key must also succeed (both are `Option<T>`, and `null` deserializes to `None` the same as an absent key) — covered explicitly in §9, not left to accident. |
| wrong-type (deserialize) | A payload with `"seq": "3"` (string instead of integer) → `serde_json::Error`, propagated by `from_str::<Receipt>`, never silently coerced. A payload with `"event": "unknown_event_kind"` → `serde_json::Error` (no `ReceiptEvent` variant matches; no catch-all `_` variant exists to silently absorb it). |
| huge | `seq: u64::MAX`, a `body` containing a 1MB nested `Value` → both round-trip exactly (u64 has no smaller ceiling than the schema's `minimum: 0`, unbounded above; `Value` is recursively arbitrary already). |
| negative | `"seq": -1` → deserialization error (`u64` cannot represent it) before this crate's own logic runs — the schema's `minimum: 0` is enforced for free by the type, not by a runtime check. |
| duplicate keys | A hand-crafted payload with `"seq"` appearing twice in the source JSON text → `serde_json`'s documented behavior applies (last-value-wins for `serde_json::from_str`, since it does not use a duplicate-key-preserving map by default) — this crate does not special-case it, and §9 records one test asserting last-value-wins explicitly so a future serde upgrade changing that default is caught, not silently absorbed. |
| concurrent | Deserializing/serializing two `Receipt`/`Attestation` values on different threads shares no state — trivially safe (no interior mutability anywhere in either type). |
| unicode / non-ASCII | `Receipt.actor = "アクター"` / `body` containing emoji → round-trips byte-for-byte (`serde_json` is UTF-8 native; no field here does ASCII-only validation except the hash/id newtypes, which are separately covered above). |
| already-exists | n/a — these types have no identity/uniqueness concept of their own; that is the ledger's (a sibling crate's) concern. |
| partial-failure | n/a — (de)serialization is a single in-memory operation; it either fully succeeds or returns one `serde_json::Error`, never a partially-populated `Receipt`. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `serde` | `1.0.229` (matches `fleet/keel/Cargo.lock`'s already-resolved version) | Every public type derives or hand-implements `Serialize`/`Deserialize` — this crate's entire reason to exist is types that cross a (de)serialization boundary. |
| `serde_json` | `1.0.151` (matches `fleet/keel/Cargo.lock`) | `Receipt.body`/`AttestationElements`'s fields are `serde_json::Value` — the one deliberately-untyped escape hatch this crate needs (see §2's non-goals). |
| `thiserror` | `2.0.20` (matches `fleet/keel/Cargo.lock`) | Every fallible fn returns a `thiserror`-derived typed error (`UnknownRole`, `EmptyIdentifier`, `BadNodeId`, `TokensOverflow`, `UnknownExitCode`, `BadBlake3Hash`) instead of `String`/`anyhow`/bare `bool` — this crate's hard requirement (§3 header, template §3). |

No other crate is needed — no async runtime, no logging framework, no filesystem/network crate:
this is deliberately the leanest, most dependency-free crate in the roster, matching its position
at the root of the DAG.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** The type-heavy `receipt`/`attestation` modules are
> split accordingly below; `lib.rs` is a thin declaration/re-export hub. Verify before review:
> `find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80'` must print nothing.

```
crates/fleet-types/
  Cargo.toml
  src/
    lib.rs             # ~34 — re-exports; module wiring only
    role.rs            # ~71 — Role, UnknownRole
    ident.rs           # ~55 — TaskId, LaneId, EmptyIdentifier, valid_artifact_id
    node_id.rs         # ~38 — NodeId, BadNodeId (split out of ident.rs to hold the 80-line cap)
    tokens.rs          # ~33 — Tokens, TokensOverflow
    lifecycle.rs        # ~62 — LifecycleState
    gate_refusal.rs      # ~26 — GateRefusal (split out of lifecycle.rs to hold the 80-line cap)
    exit_code.rs        # ~48 — ExitCode, UnknownExitCode
    receipt.rs          # ~44 — Blake3Hash, BadBlake3Hash (the hash newtype)
    prev_hash.rs         # ~43 — PrevHash (split out of receipt.rs to hold the 80-line cap)
    receipt_event.rs     # ~66 — ReceiptEvent, SchemaV1, Receipt (the wire record)
    attest_subject.rs    # ~65 — DeliveryTier, BareBlake3Digest, AttestationSubject, AttestationDigest
    attest_predicate.rs  # ~47 — AttestationBuilder, AttestationElements, DeliveryPredicate
    attest_stmt.rs       # ~62 — InTotoStatementV1, DeliveryAttestationV1, Attestation
  tests/
    attestation_roundtrip.rs   # serde round-trip against attestation.v1.json
    attestation_extras.rs      # additional attestation edge cases
    receipt_roundtrip.rs       # serde round-trip against receipt.v1.json
    receipt_edge_cases.rs      # additional receipt edge cases (null-vs-absent, duplicate keys, ...)
    hash_types.rs              # Blake3Hash/BareBlake3Digest/PrevHash parse edge cases
    identifiers.rs             # TaskId/NodeId/LaneId/valid_artifact_id parse edge cases
    lifecycle_and_exit_code.rs # LifecycleState::allowed_next totality, ExitCode round-trip
    role_and_tokens.rs         # Role::parse/facts, Tokens::checked_add/checked_sub
    contract_value_parity.rs   # asserts literal values against the .v1.json contracts
    schema_parity.rs           # schema-parity harness (uses tests/schema_parity/support.rs)
    schema_parity/
      support.rs                # shared helpers for schema_parity.rs
```

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-types"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.20"

[dev-dependencies]
serde_json = "1.0.151"
```

## 9. Test plan

**Unit tests** (in each module's `#[cfg(test)]`):
- `role_parse_matches_all_five_and_only_five` — every `Role::ALL` member round-trips through
  `name()` → `parse()`; every string not in `{lead, builder, verifier, designer, meter}` (including
  mixed-case and trailing-whitespace variants) is `Err(UnknownRole(_))`.
- `role_bandwidth_and_fitness_are_fixed_per_variant` — asserts the exact verbatim numbers from
  `roles.rs:32-50` for all 5 roles (a policy-drift regression test — if a future edit changes one
  of these five numbers, this test names exactly which role's which field moved).
- `task_id_node_id_lane_id_reject_empty_and_whitespace_only` — `""`, `"   "`, `"\t\n"` all fail
  for `TaskId`/`LaneId`; `NodeId` additionally rejects `"Ab-c"` (uppercase), `"-abc"` (leading
  hyphen), `"ab"` (too short, 2 chars < the 3-char floor from `{2,63}` after the first char).
- `tokens_checked_add_and_sub_never_wrap` — `u64::MAX + 1` and `0 - 1` (via `Tokens`) both return
  `Err(TokensOverflow)`, never a wrapped value.
- `lifecycle_state_allowed_next_is_total_and_matches_the_source_table` — for each of the 16
  `LifecycleState` variants, asserts `allowed_next()` returns exactly the successor set from
  `lifecycle.rs:16-33`'s `STATES` table (e.g. `Building -> [Built, Refused]`, `Refused -> []`).
- `exit_code_round_trips_through_i32_for_all_six_values` — `[0,3,6,7,8,9]` each `TryFrom<i32>` to
  the matching variant and `.as_i32()` back to the same number; every other `i32` in
  `{-1, 1, 2, 4, 5, 10, i32::MAX, i32::MIN}` is `Err(UnknownExitCode(_))`.
- `blake3_hash_rejects_wrong_length_wrong_case_missing_prefix_and_unicode_lookalikes` — the
  concrete cases named in §6's behavior-spec table.

**Integration tests** (calling only the public API, one file per wire contract):
- `receipt_roundtrip.rs::receipt_matches_the_json_schema_shape` — construct a `Receipt` for each
  of the 8 `ReceiptEvent` variants, serialize, and assert against a literal expected-JSON string
  (not just "deserializes back equal" — a literal string catches a field silently renamed or
  reordered in a way `assert_eq!` on the round-tripped struct would miss).
- `receipt_roundtrip.rs::optional_fields_absent_vs_explicit_null_both_deserialize_to_none` — feeds
  two payloads (one omitting `resolved_model`/`exit_code`, one setting them to JSON `null`) and
  asserts both produce `None`.
- `receipt_roundtrip.rs::duplicate_seq_key_is_last_value_wins` — a hand-written JSON string with
  `"seq"` appearing twice at different values; asserts the parsed `Receipt.seq` equals the second
  occurrence (documents `serde_json`'s actual behavior as a pinned regression, per §6).
- `receipt_roundtrip.rs::malformed_event_and_negative_seq_are_rejected` — `"event":"bogus"` and
  `"seq":-1` both fail to deserialize with a `serde_json::Error`, never panicking.
- `attestation_roundtrip.rs::attestation_matches_the_json_schema_shape` — one `Attestation` per
  `DeliveryTier` variant, serialized and compared against a literal expected-JSON string including
  the fixed `_type`/`predicateType` constants.
- `attestation_roundtrip.rs::elements_with_every_slot_absent_still_round_trips` — `elements: {}`
  (all 8 `None`) serializes to `"elements":{}` and deserializes back to all-`None`.
- `identifiers.rs::node_id_accepts_the_schema_boundary_lengths` — 3-char and 64-char valid
  `node_id`s both accepted; 2-char and 65-char both rejected (off-by-one boundary coverage for
  `{2,63}` after the mandatory first character).

**Mutation-testing targets** (`cargo mutants -p fleet-types`):
- Flipping `Blake3Hash::parse`'s digest-length check from `== 64` to `>= 64` or `<= 64` must be
  killed by a dedicated `blake3_hash_rejects_63_and_65_char_digests` test (exact-length boundary).
- Deleting the `#[serde(skip_serializing_if = "Option::is_none")]` attribute on `Receipt.exit_code`
  (so `None` serializes as explicit `"exit_code":null` instead of an absent key) must be killed by
  `receipt_roundtrip.rs::receipt_matches_the_json_schema_shape`'s literal-string comparison — a
  round-trip-equality-only test would NOT catch this (both forms deserialize back to the same
  struct), which is exactly why that test asserts against a literal JSON string, not just
  round-trip equality.
- Swapping `LifecycleState::allowed_next`'s `Building` and `Built` match arms (returning the wrong
  successor set for one state) must be killed by
  `lifecycle_state_allowed_next_is_total_and_matches_the_source_table`'s per-variant assertions
  (one assertion per state, not one aggregate assertion — a single combined check could pass by
  coincidence if two states' successor sets happen to overlap).
- Changing `ExitCode::Refusal`'s discriminant from `7` to any other unused number must be killed by
  `exit_code_round_trips_through_i32_for_all_six_values`'s exact-value assertions per variant.

**Property tests** (`proptest`, recommended given the D50 "silently unreachable" history other
blueprints in this roster cite):
- *Every valid `Tokens` pair's `checked_add` is commutative and associative up to overflow*: for
  `a, b, c: u64` sampled below `u64::MAX / 4` (guaranteed non-overflowing), `Tokens(a).checked_add
  (Tokens(b))` and `Tokens(b).checked_add(Tokens(a))` are both `Ok` and equal; chaining `a+b+c` in
  either associative grouping yields the same `Ok` value. 256 cases minimum.
- *Every `Blake3Hash`/`BareBlake3Digest`/`NodeId` string accepted by `parse` re-serializes to the
  exact same string* (no normalization silently changes case or trims characters on a value that
  already passed validation). 256 cases minimum, generated from the exact accepted character
  classes (lowercase hex; lowercase-alnum-hyphen respectively).

## 10. Verification recipe

```bash
cd crates/fleet-types
cargo test -p fleet-types --all-targets
cargo clippy -p fleet-types --all-targets -- -D warnings
cargo mutants -p fleet-types
```
Expected: all unit + integration + property tests pass, 0 skipped — publish as `<passed>/<total>`
(e.g. `31/31`, never just "tests pass"). Clippy: 0 warnings. Mutants: every target named in §9
caught — publish `<caught>/<total mutants>`; as fleet's most-imported, purest crate, the floor is
**100% of viable mutants caught**, not a partial-credit percentage — any survivor gets a new test
before this crate is marked done, never a lowered floor (matching `fleet-router`'s precedent, §10).

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`UnknownRole`, `EmptyIdentifier`,
      `BadNodeId`, `TokensOverflow`, `UnknownExitCode`, `BadBlake3Hash`) — none swallowed into
      `bool`/`Option`/`String`/`.unwrap()` in non-test code. (Mark done once implemented and
      `grep -rn '\.unwrap()\|panic!' crates/fleet-types/src/` outside `#[cfg(test)]` returns
      nothing.)
- [ ] Clock/RNG/IO are injected — trivially, by having none: every public fn takes only
      already-in-memory values and returns owned values; `Receipt.ts_wall` is a passthrough
      `String`, never parsed or read from `SystemTime::now()`.
- [x] Thread-safety documented: every public type is a plain value (`Copy` where small enough —
      `Role`, `Tokens`, `LifecycleState`, `ExitCode`, `DeliveryTier`; owned-`String`-backed
      otherwise) with no interior mutability anywhere — `Send + Sync` for free, safe to construct,
      clone, and serialize concurrently from any number of threads.
- [x] No float used for money, tokens, or any precision-sensitive count — `Tokens` wraps `u64`;
      no `f32`/`f64` appears anywhere in §3's public API.
- [ ] No self-grading — verification runs `cargo mutants`, not just the crate's own unit tests;
      denominator published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10, template) — restate
      the real numbers in the PR once the crate is built (mark done then).
- [x] Tests that touch the filesystem: none needed — this crate has zero filesystem access to
      test against; if a future test ever needs a temp file it must use `tempdir()`, never the
      repo tree.
- [ ] Every non-goal in §2 is actually absent from the code — no `Check`/`evaluate`/
      `RoleRefusal`-shaped safety-gate logic, no `Task<S>`/marker-struct state machine, no
      `ReceiptLedger`/IO trait, no `agent.rs` `Scorecard` arithmetic anywhere in
      `crates/fleet-types/src/`. Enforce with
      `grep -rn 'fn evaluate\|struct Task<\|trait ReceiptLedger\|struct Scorecard' crates/fleet-types/src/`
      returning nothing.

## 12. Definition of Done

`fleet-types` is DONE when: §10's three commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught, floor 100%) run from `crates/fleet-types/`; every
unchecked box in §11 is checked with its real numbers; `registry/services/REGISTRY.md` (base
infrastructure, per C1/L2) lists the crate; `fleet-router`'s blueprint (which already assumes
`use fleet_types::Role;`) is re-checked against the actual shipped `Role` API and any drift is
reconciled in the same commit (PLAYBOOK.md rule 1 — spec and code never drift silently); and Opus
has independently re-derived every type in §3 from `fleet/keel/fleet/src/roles.rs`,
`lifecycle.rs`, and `fleet/contracts/{receipt,attestation}.v1.json` alone (without trusting this
blueprint's citations), reproduced the `SchemaV1`/`exit_code`-omission mutation by hand, and driven
one real `Receipt` and one real `Attestation` through a full serialize → deserialize → re-serialize
cycle confirming byte-stable output against the literal JSON string, not merely struct equality.

---

## Divergence / open questions (for Opus)

1. **MIGRATION-PLAN §3 row 1's evidence column reads as if `Check`/`evaluate` (the safety-gate
   primitive) re-homes into `fleet-types`.** Having read `fleet-router`'s own (Opus-approved,
   `blueprint-done`) blueprint, that decision is already made the other way: `fleet-router`
   explicitly claims these two rules as `RoleRefusal`/`evaluate_role_check`, reasoning "nothing
   outside routing consumes it." This blueprint follows `fleet-router`'s precedent rather than row
   1's literal text, to avoid two crates each defining a competing version of the same two rules.
   **If a second consumer of the safety-gate check appears later** (e.g. `fleet-worker` wanting to
   refuse a dispatch before even calling `fleet-router`), that is the trigger to promote
   `RoleRefusal`/`evaluate_role_check` up into `fleet-types` — not before. Opus should confirm this
   reading of row 1 is intentional (i.e., row 1's evidence column should be corrected to say
   "`Role` only; the gate check itself lives in `fleet-router`") rather than a plan gap.

2. **No crate in the 14-crate roster is named as the owner of `lifecycle.rs`'s `Task<S>`
   phantom-type state machine.** `fleet-plan` (row 6) cites `intake.sh`/`lld.rs`/`lld_ready.rs`/
   `review.sh` — the *Intake→Contracted* half of the 16-state chain by name, but never
   `lifecycle.rs` itself. This blueprint resolves the ambiguity by giving `fleet-types` only the
   **data half** (`LifecycleState`, a plain 16-variant enum + `allowed_next`) and explicitly
   leaving the **compile-time mechanism** (`sealed::Sealed`, the 16 zero-sized marker structs,
   `State`, `Task<S>`, `TransitionReceipt`, `ReceiptLedger`, `HumanApproval`) unassigned pending
   Opus's call on which crate (`fleet-plan`? a new `fleet-lifecycle`? `src/` itself, since the
   chain spans intake through PR-proposal end to end?) owns it. Two considered alternatives Opus
   should weigh: (a) put the phantom marker structs themselves in `fleet-types` too (matching the
   task prompt's literal suggestion) and let each stage-owning crate write inherent impls on a
   `Task<S>` *also* defined in `fleet-types` — this does not work as stated, because Rust's orphan
   rule requires inherent impls on a generic struct to live in the crate that defines the struct,
   so splitting `Task<S>`'s transition methods across `fleet-plan`/`fleet-worker`/`fleet-verify`/
   `src/` is impossible if `Task<S>` itself lives in `fleet-types`; or (b) this blueprint's choice
   — one crate owns the whole `Task<S>` chain end to end (simplest, matches how `lifecycle.rs`
   already reads as one coherent unit today), with `fleet-types`'s `LifecycleState` enum used only
   for cross-crate *reporting* of "which state is this task in," never for enforcement. **Row 1
   and the roster should get a line naming which crate owns `lifecycle.rs`'s `Task<S>`** — today
   it is a genuine gap, not a documented decision, the same class of gap `fleet-router`'s
   blueprint flagged for `roles.rs`.

3. **`agent.rs`'s `Scorecard`** was read in full and considered for this crate (it looks like
   shared vocabulary: `{agent_id, charter, credited, faulted, unknown, checked, total}`) but
   rejected — its arithmetic is entangled with `agent.rs`'s per-agent file-locked
   read-modify-write (`record_scorecard_outcome`, `agent.rs:228-289`), making it IO-adjacent state
   rather than a pure value type. It is not cited in MIGRATION-PLAN's roster at all (only
   `agent.rs`'s worktree/skills/mcp/fd-3 pieces are, under row 13 `fleet-worker`). Opus should
   confirm `Scorecard` belongs with `fleet-worker` (row 13, since that is where the rest of
   `agent.rs` is heading) rather than a crate this blueprint should have claimed.

4. **`Tokens` is new — no such newtype exists in fleet today** (`meter.rs` uses bare
   `Option<u64>`/`Vec<u64>` throughout). This blueprint introduces it because the task prompt
   explicitly asks for an integer-minor-units token type and `fleet-router`'s `RuntimeState`
   already has `remaining: BTreeMap<String, Option<u64>>` / `required_tokens: u64` that a future
   revision could switch to `Tokens`/`Option<Tokens>`. **This is a suggested future migration, not
   a requirement of this blueprint** — `fleet-router`'s already-`blueprint-done` API is not being
   changed retroactively here; Opus should decide whether to fold `Tokens` into `fleet-router`'s
   and `fleet-govern`'s (not-yet-written) blueprints going forward.
