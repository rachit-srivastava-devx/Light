# BLUEPRINT — `fleet-events`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-events`
- **One-line purpose:** Own the INGRESS plane — pull the outside world (GitHub, Gmail, the
  filesystem, the CLI) into one typed `EventEnvelope`, keep the untrusted payload inert until a
  mandatory `guard` checkpoint runs, and hand the result to an injected durable-storage port —
  never deciding what the envelope means, never acting on it itself.
- **Build branch:** `build-new` — absent in fleet (MIGRATION-PLAN §3 row 3). There is no existing
  webhook/IMAP/fs-watch/CLI-ingress code in `fleet/keel/fleet/src` to extract; confirmed by search
  (`grep -rn -i "webhook|imap|gmail|notify::|inotify" fleet/keel/fleet/src` — zero hits, 2026-09-08).
  Fleet's own `scan.sh` is a security scanner (name collision only, unrelated purpose — see
  MIGRATION-PLAN §3 row 5's disambiguation of `fleet-scan`).
- **Imports:** `fleet-types` (`Blake3Hash` for `EventId`; no other type needed — see §5).
  `fleet-store` is **not** taken as a compile-time dependency by this blueprint — see the
  "Divergence from MIGRATION-PLAN" note at the end of this file for why, and what this crate
  defines instead (`EventSink`, an injected port fleet-store is expected to satisfy once its own
  blueprint exists).
- **Imported by:** `fleet-worker` (the kernel — reads `EventEnvelope.kind` to decide what lane runs,
  never reads `.payload` for control flow, and must call `guard` before turning a side-effecting
  `kind` into an actual action), `src/` (composition root — invokes each `Adapter` spawn-on-demand
  from a cron entry/systemd timer/manual CLI call, supplies the concrete `EventSink` that writes
  through to whatever `fleet-store` ends up exposing, and owns the real `SystemClock`).

## 2. Responsibility & non-goals

**Owns:** the one typed `EventEnvelope` shape every ingress source is normalized into; the
`Adapter` trait each source implements (open/closed — a new source is one new file, the trait and
`ingest_once` orchestration never change); the `EventId` derivation that makes at-least-once
adapter delivery idempotent; and the `guard` checkpoint that is the sole place a side-effecting
`EventKind` gets a verbatim, human-auditable quote of its untrusted source text before anything
downstream is allowed to act on it. This crate is the injection-surface boundary: everything past
it must be able to trust that `kind` alone was used for dispatch and that `payload` was never
interpreted as instructions by anything inside this crate.

**Non-goals (the seam):**
- Does **not** decide what a `kind` means or what lane/role should handle it — that's
  `fleet-worker`'s job (the kernel). This crate produces envelopes; it never dispatches them.
- Does **not** make a routing decision (`fleet-router`) or a scheduling/quota decision
  (`fleet-govern`) — an envelope carries no adapter/model selection, only `source`/`kind`/`payload`.
- Does **not** durably persist anything itself — `ingest_once` writes through the caller-supplied
  `EventSink` port; the concrete durable backend (redb/sqlite/whatever `fleet-store` settles on) is
  `fleet-store`'s job or the composition root's adaptation of it, never this crate's.
- Does **not** run a resident daemon of any kind — no long-lived HTTP listener, no IMAP `IDLE`
  connection held open, no infinite `notify` event loop. Every `Adapter::pull` call is bounded and
  returns; "continuous" ingestion is an external scheduler re-invoking a short-lived process, not a
  loop inside this crate (spawn-on-demand, not resident).
- Does **not** ever branch control flow on `payload` contents — not in this crate, and `guard`'s
  entire purpose is to make that discipline enforceable one layer further out too: `kind` decides
  `requires_confirmation` statically (`EventKind::side_effecting`), never something parsed out of
  the payload. If a future adapter needs payload-derived branching, that is itself a defect to push
  back on, not a feature to add here.
- Does **not** verify/authenticate a source's transport-level signature (GitHub's HMAC header,
  IMAP's TLS handshake) as a *shared* concern — each adapter owns its own transport authenticity
  check as part of what makes it a working adapter for that source (see §5); this crate does not
  provide a generic "verify signature" primitive because the mechanism is source-specific and
  there is exactly one adapter per source to own it.

## 3. Public API contract

```rust
//! The ingress plane: pull the outside world into one typed, guarded event envelope.
//!
//! This crate performs real IO (that is its entire purpose — it is fleet's injection-surface
//! boundary), but every fact this crate does NOT itself need to touch the outside world for
//! (the wall clock, the durable log) arrives through an injected port (`Clock`, `EventSink`), so
//! the orchestration logic (`ingest_once`, `guard`) is testable without a network, a mailbox, or a
//! filesystem. `kind` is the only field anything downstream may use for control flow; `payload` is
//! untrusted data, never instructions, and `guard` is the one mandatory checkpoint before a
//! side-effecting `kind` becomes an actual action.

use std::time::Duration;
use serde_json::Value;
use fleet_types::Blake3Hash;

// =====================================================================================
// A. Identity — deterministic, idempotent-by-construction
// =====================================================================================

/// Stable identity of one ingested event, derived (never random) from `(source, kind,
/// external_id)` so that two `pull()` calls observing the same underlying external event — GitHub
/// redelivering a webhook, an adapter re-polling and re-seeing an unread IMAP UID after a crash,
/// a filesystem watch re-scanning after a missed cycle — produce the identical `EventId`. This is
/// what lets `EventSink::append` be a no-op-on-repeat rather than a duplicate row.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct EventId(Blake3Hash);

impl EventId {
    /// `external_id` is whatever the adapter's own protocol already uses to name this event
    /// uniquely (GitHub's `X-GitHub-Delivery` header, an IMAP UID formatted as a string, an
    /// absolute file path + a change-generation counter, a CLI invocation nonce). Hashing
    /// `(source, kind, external_id)` together — not `external_id` alone — means the same
    /// `external_id` string reused by two different sources/kinds can never collide.
    pub fn derive(source: SourceKind, kind: EventKind, external_id: &str) -> Self { unimplemented!() }
    pub fn as_blake3(&self) -> &Blake3Hash { unimplemented!() }
}

// =====================================================================================
// B. The envelope — the ONE typed shape every source normalizes into
// =====================================================================================

/// Which ingress source produced this envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    GithubWebhook,
    Gmail,
    FsWatch,
    Cli,
}

/// The typed, closed vocabulary the kernel dispatches on. Chosen by trusted adapter code from
/// protocol-level metadata only (an HTTP header/route, an IMAP flag, a filesystem event type, an
/// argv shape) — never derived by parsing the untrusted payload text. This is what makes
/// `side_effecting` injection-proof: nothing in `payload` can change which variant an envelope
/// carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    GithubPullRequestOpened,
    GithubPullRequestUpdated,
    GithubIssueComment,
    GithubPush,
    GmailMessageReceived,
    FsFileCreated,
    FsFileModified,
    FsFileDeleted,
    CliInvoked,
}

impl EventKind {
    /// Fixed per variant at compile time — never a runtime/payload-derived value. `true` means:
    /// if the kernel acts on this envelope, that action is visible outside fleet's own event log
    /// (starts a lane, replies to an email, pushes a commit, writes a file) and MUST go through
    /// `guard` first. `GithubIssueComment`/`GmailMessageReceived`/`CliInvoked` are `true` (they can
    /// trigger a lane); `FsFileCreated`/`Modified`/`Deleted` are `true` only when they trigger a
    /// build (which they do, by fleet's design) — see the per-variant match in the reference impl.
    pub fn side_effecting(self) -> bool { unimplemented!() }
}

/// One normalized ingress event. `payload` is the untrusted body, exactly as observed, capped at
/// `MAX_PAYLOAD_BYTES` (§4) before construction — never parsed for control flow by this crate or
/// by any consumer; `kind` is the only dispatch key.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EventEnvelope {
    pub id: EventId,
    pub source: SourceKind,
    pub kind: EventKind,
    /// RFC3339, stamped by the adapter from its injected `Clock` — never
    /// `SystemTime::now()`/`Instant::now()` called directly inside adapter or orchestration logic.
    pub received_at: String,
    pub payload: Value,
}

/// `payload` is truncated to this many bytes before an `EventEnvelope` is constructed (applies to
/// the JSON-serialized form). Prevents an unbounded upstream body (a huge email attachment inlined
/// as base64, a pathological webhook payload) from becoming an unbounded envelope; the guard quote
/// (§C) is capped independently and tighter, for human readability.
pub const MAX_PAYLOAD_BYTES: usize = 262_144; // 256 KiB

// =====================================================================================
// C. Guard — the mandatory checkpoint before a side-effecting kind becomes an action
// =====================================================================================

/// What a caller must look at (and, if `requires_confirmation`, get explicit approval on) before
/// turning `kind` into an action. Always populated, even when confirmation isn't required, so the
/// audit trail never needs to re-fetch `payload` later to explain why an action proceeded.
#[derive(Clone, Debug, serde::Serialize)]
pub struct GuardedAction {
    pub envelope_id: EventId,
    pub kind: EventKind,
    /// Verbatim, byte-capped excerpt of `payload` (see `MAX_QUOTE_BYTES`) — never summarized,
    /// reinterpreted, or translated. A summary can smuggle an injected instruction past a human
    /// reviewer by silently dropping the parts that would have tipped them off; a verbatim quote
    /// cannot change meaning, it can only be too long (hence the cap, not a paraphrase).
    pub quote: String,
    pub requires_confirmation: bool,
}

/// Quote is capped to this many bytes (of the JSON-serialized `payload`) — small enough that a
/// human reviewing a confirmation prompt actually reads it, not so small that the quote could omit
/// the one line that mattered. Independent of, and smaller than, `MAX_PAYLOAD_BYTES`.
pub const MAX_QUOTE_BYTES: usize = 2_048;

/// The one mandatory checkpoint between an ingested envelope and any side-effecting action. Pure
/// and total — no IO, never panics. `envelope.kind.side_effecting()` alone decides
/// `requires_confirmation`; this fn never inspects `payload` to make that decision, only to build
/// the quote a human/kernel reads.
pub fn guard(envelope: &EventEnvelope) -> GuardedAction { unimplemented!() }

// =====================================================================================
// D. Ports — the seams this crate needs, supplied by the caller
// =====================================================================================

/// Injected wall-clock source. `SystemClock` is the only concrete impl this crate ships, and it is
/// used exclusively by adapters at the composition-root call site — no logic in this crate reads
/// the clock ambiently.
pub trait Clock {
    fn now_rfc3339(&self) -> String;
}

/// The real clock. Constructing and using this is the composition root's job (`src/`); nothing in
/// `ingest_once`, `guard`, or any adapter's decision logic calls `SystemClock` itself — they all
/// take `&dyn Clock`.
pub struct SystemClock;
impl Clock for SystemClock {
    fn now_rfc3339(&self) -> String { unimplemented!() }
}

/// A failure appending to the durable event log.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SinkError {
    #[error("event log unavailable: {0}")]
    Unavailable(String),
}

/// The durable-write port this crate needs and does not implement. `fleet-store` (once its own
/// blueprint exists) is expected to provide a type implementing this trait, or the composition
/// root adapts fleet-store's real API into one — see the divergence note at the end of this file.
pub trait EventSink {
    /// MUST be idempotent on `envelope.id`: calling twice with an envelope carrying the same `id`
    /// is a successful no-op, never a duplicate row and never an error. This is required because
    /// adapters are at-least-once by design (§6) — exactly-once effect is achieved here, not there.
    fn append(&self, envelope: &EventEnvelope) -> Result<(), SinkError>;
}

// =====================================================================================
// E. Adapter — one pull-based source, open/closed: a new source is one new file
// =====================================================================================

/// One ingress source. Every concrete adapter (`adapters/github.rs`, `adapters/gmail.rs`,
/// `adapters/fs_watch.rs`, `adapters/cli.rs`) implements this and nothing else touches the trait —
/// adding a fifth source is a fifth file behind this trait, never a change to `ingest_once` or to
/// any existing adapter.
pub trait Adapter {
    type Error: std::error::Error + Send + Sync + 'static;

    fn source(&self) -> SourceKind;

    /// Pull whatever is newly available since this adapter's own last call, using `clock` for any
    /// timestamp it stamps and its own persisted cursor (last GitHub delivery id, last IMAP UID,
    /// last fs snapshot — adapter-specific, never shared) for what "new" means. Never blocks
    /// indefinitely: every concrete adapter bounds its own network/IO wait internally (see each
    /// adapter file's fixed timeout). Returns an empty `Vec` — not an error — when nothing new is
    /// available; only a genuine IO/protocol failure is `Err`.
    fn pull(&mut self, clock: &dyn Clock) -> Result<Vec<EventEnvelope>, Self::Error>;
}

// =====================================================================================
// F. Orchestration — the one spawn-on-demand ingest cycle
// =====================================================================================

/// Why one `ingest_once` call failed. Carries the adapter's own error type so a caller retains the
/// concrete diagnostic (an HTTP status, an IMAP error string, an `io::Error`) rather than a
/// stringly-typed wrapper.
#[derive(Debug, thiserror::Error)]
pub enum IngestError<E: std::error::Error + 'static> {
    #[error("adapter pull failed: {0}")]
    Pull(#[source] E),
    #[error("event sink rejected an envelope: {0}")]
    Sink(#[source] SinkError),
}

/// What one `ingest_once` call did, for the caller to log/print. `guarded` always has one entry
/// per pulled envelope (§C — guard runs unconditionally, whether or not confirmation is required).
#[derive(Debug, Default)]
pub struct IngestReport {
    pub pulled: usize,
    pub written: usize,
    pub guarded: Vec<GuardedAction>,
}

/// Run exactly one spawn-on-demand ingest cycle: call `adapter.pull` once, run `guard` over every
/// envelope it returned (so the report has a full audit trail even for envelopes that don't need
/// confirmation), write each envelope through `sink` (idempotent per `EventSink`'s own contract),
/// and return. Never loops, never re-polls "until empty," never blocks waiting for more to arrive
/// — one `pull()` call, one pass, done. "Continuous" ingestion is the caller re-invoking this from
/// a cron entry/systemd timer, not a loop living inside this crate.
///
/// On a `Sink` failure partway through the batch: envelopes already written stay written (the sink
/// is idempotent, so a retried `ingest_once` re-writing them is safe); the fn returns
/// `Err(IngestError::Sink(..))` immediately rather than continuing past a failing store, and
/// `IngestReport` is not returned on error (the caller retries the whole cycle — safe, because
/// `EventId` derivation and `EventSink::append` are both idempotent).
pub fn ingest_once<A: Adapter, S: EventSink>(
    adapter: &mut A,
    sink: &S,
    clock: &dyn Clock,
) -> Result<IngestReport, IngestError<A::Error>> {
    unimplemented!()
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `EventId` | Deterministic function of `(source, kind, external_id)`, never random, never time-based. | Two observations of the same underlying external event silently becoming two different ledger rows. |
| `EventKind::side_effecting` | A `const fn`-shaped, per-variant match with no wildcard arm and no read of `payload`. | A future adapter change quietly making "requires confirmation" depend on parsed payload content — the injection hole this whole crate exists to close. |
| `EventEnvelope.payload` | Capped at `MAX_PAYLOAD_BYTES` before construction; always the untrusted body, never a parsed/interpreted structure this crate branches on. | An unbounded upstream body turning into an unbounded envelope; `payload` being silently trusted as instructions anywhere in this crate. |
| `GuardedAction.quote` | Always populated (even when `requires_confirmation` is `false`), capped at `MAX_QUOTE_BYTES`, always a verbatim substring/excerpt — never a summary or translation. | A side-effecting action being approved (or auto-passed) with no verbatim record of what was actually being approved. |
| `EventSink::append` | Documented idempotent-on-`id` contract (this crate cannot enforce it across a trait boundary — it is a documented precondition every implementor must satisfy, verified by this crate's own fakes in tests). | An at-least-once adapter redelivering the same external event and the durable log recording it twice. |
| `IngestReport` | `guarded.len() == pulled` always (guard runs over every pulled envelope, unconditionally). | A silently un-guarded envelope reaching the caller with no audit trail entry. |

**Money/precision:** no money or token type in this crate.

**Clock/RNG injection points:** `Clock` (trait, §3D) — every RFC3339 timestamp any adapter stamps
goes through it; `SystemClock` is the only concrete impl and is used only by the composition root.
No RNG anywhere: `EventId` is a deterministic hash, not a random identifier, by design (§4 above).

**IO/filesystem/network/subprocess boundaries** (this crate's entire purpose is to be one, so it
is exhaustively named rather than avoided):
- `adapters/github.rs` — network (HTTPS via `reqwest`, blocking). Direct, not further injected
  behind a second trait — the `Adapter` trait itself is the injection point (a test supplies a fake
  `Adapter`, not a fake HTTP client), matching the router blueprint's precedent of injecting at the
  outermost seam a caller actually needs.
- `adapters/gmail.rs` — network (IMAPS via `imap` + TLS). Same: direct inside the adapter, injected
  at the `Adapter` trait boundary.
- `adapters/fs_watch.rs` — filesystem (via `notify`, bounded-timeout poll mode, never `IDLE`-style
  indefinite watch). Direct inside the adapter.
- `adapters/cli.rs` — reads argv/stdin handed to it at construction (no IO call of its own — the
  composition root already did the OS-level read); the closest thing to "pure" of the four.
- `EventSink` implementors — filesystem/network/subprocess, entirely outside this crate (§2 non-goal).

## 5. Reuse map

**Greenfield** — build-new, per MIGRATION-PLAN §3 row 3. No fleet source to lift; confirmed absent
by search (§1). Chosen libraries (checked against `fleet/keel/Cargo.lock` first — none of the four
below are already resolved there, so all four are genuinely new dependencies; verified each exists
and is actively maintained via crates.io, 2026-09-08):

| Library | Version | Why this one |
|---|---|---|
| `reqwest` (blocking feature) | `0.13` | De facto standard Rust HTTP client; `blocking` avoids pulling an async runtime into a crate whose adapters run as short, spawn-on-demand processes rather than inside a long-lived executor. Powers `adapters/github.rs`'s poller (GitHub REST API `since`/ETag polling, not an inbound webhook listener — see the divergence note on why "webhook" here means "poller," not "receiver"). |
| `notify` | `8.2.0` | Cross-platform filesystem-event backend (inotify/FSEvents/ReadDirectoryChangesW) with 60M+ downloads, actively maintained. Used in `adapters/fs_watch.rs` in bounded-timeout poll mode (drain events for a fixed `Duration`, then return) — never as a resident indefinite watcher, matching the spawn-on-demand non-goal. |
| `imap` | `2.4.1` | Maintained, widely-used Rust IMAP client (RFC 3501 + extensions). Used in `adapters/gmail.rs` for `UID SEARCH ... SINCE <last_uid>`-style polling against a persisted cursor — never `IDLE` (which requires holding a connection open, i.e. a resident daemon). |
| `mailparse` | `0.16.1` | Simple, maintained MIME email parser. Used in `adapters/gmail.rs` to pull the subject/from/body text out of a raw IMAP-fetched message into `EventEnvelope.payload` — parsing structure, not interpreting the content as instructions (the parsed fields still count as untrusted `payload`, never as `kind`). |

Each adapter also owns its own transport-authenticity check as part of "being a working adapter,"
per §2's non-goal on why this isn't a shared primitive: GitHub's `X-Hub-Signature-256` HMAC
verification lives in `adapters/github.rs`; IMAP's TLS-verified connection is `imap`'s own
responsibility, configured in `adapters/gmail.rs`; the fs adapter has no remote party to
authenticate; the CLI adapter trusts its caller (the composition root already ran as the invoking
user).

## 6. Behavior spec

### `fn guard(envelope: &EventEnvelope) -> GuardedAction`

| Input dimension | Behavior |
|---|---|
| empty | `payload: Value::Null` or `{}` → `quote` is the serialized empty form (`"null"`/`"{}"`), never a panic; `requires_confirmation` still comes from `kind.side_effecting()` alone, independent of how little payload there is. |
| null / `None` | Same as empty — there is no `Option` in this fn's signature; `Value::Null` is the payload-level equivalent and is handled identically to any other JSON value: quoted, capped, never specially rejected. |
| wrong-type | n/a — `payload: Value` already accepts any JSON shape by construction; there is no narrower type for a caller to misuse here. |
| huge | `payload` serialized form over `MAX_QUOTE_BYTES`: `quote` is truncated to exactly that many bytes at a UTF-8 char boundary (never mid-codepoint) with a trailing marker (e.g. `"…[truncated]"`) appended after the cap, itself counted within the byte budget — never allocates more than `MAX_QUOTE_BYTES` plus the marker's fixed small overhead. |
| negative | n/a — no numeric input. |
| duplicate | Calling `guard` twice on the same `EventEnvelope` value (same `id`) is not an error and not tracked as "already guarded" by this fn — idempotent by virtue of being pure (same input, same output); any "have we already asked for confirmation on this one" bookkeeping is the caller's job, not this fn's. |
| concurrent | Pure value fn, no shared state — trivially safe to call from any number of threads. |
| unicode / non-ASCII | `quote`'s truncation is UTF-8-char-boundary-safe (see "huge" above); a payload containing right-to-left override characters, zero-width joiners, or other text-direction/rendering tricks is quoted byte-for-byte, unmodified — this fn does not sanitize or normalize for display, it only caps length; a caller rendering the quote to a terminal/UI is responsible for safe display (documented here as a known boundary, not silently handled). |
| already-exists | n/a — no persisted state in this fn. |
| partial-failure | n/a — no IO, cannot fail partially; always returns a fully-populated `GuardedAction` synchronously. |

### `fn ingest_once<A: Adapter, S: EventSink>(adapter: &mut A, sink: &S, clock: &dyn Clock) -> Result<IngestReport, IngestError<A::Error>>`

| Input dimension | Behavior |
|---|---|
| empty | `adapter.pull` returns `Ok(vec![])` → `IngestReport { pulled: 0, written: 0, guarded: vec![] }`, `Ok(_)` overall — an empty pull is success, never an error. |
| null / `None` | n/a — no `Option` parameters; `adapter`/`sink`/`clock` are non-nullable references by construction. |
| wrong-type | n/a — generic bounds (`Adapter`, `EventSink`) are enforced at compile time; there is no runtime type to get wrong. |
| huge | `adapter.pull` returns thousands of envelopes in one call: `ingest_once` processes them in the order returned, one `guard` + one `sink.append` per envelope, `O(n)` in the batch size, no unbounded buffering beyond the `Vec` the adapter itself already allocated — no additional quadratic behavior introduced by this fn. |
| negative | n/a — no numeric input. |
| duplicate | Two envelopes in the same batch sharing an `id` (an adapter bug, since `EventId::derive` should prevent this from distinct external events): both are written; `sink.append`'s idempotence contract means the second write is a documented no-op, not a crash — this fn does not itself deduplicate within a batch, it relies on the sink's contract, which is the one place idempotence is actually enforced. |
| concurrent | `ingest_once` takes `&mut A` (adapter) and `&S`/`&dyn Clock` (sink/clock, both shared-safe by their trait's own contract) — a single `ingest_once` call is not designed to run concurrently with itself over the *same* `adapter` (the `&mut` prevents that at compile time); concurrent calls over *distinct* adapters/sinks are safe if `S`/`Clock` impls are themselves `Sync` (documented per-impl, not assumed here). |
| unicode / non-ASCII | Passed through unchanged — `ingest_once` never inspects payload text itself, only counts and forwards envelopes. |
| already-exists | An envelope whose `id` the sink has already stored: `sink.append` is documented idempotent, so `written` still increments (this fn counts "envelopes successfully written-or-already-present," not "new rows created" — the caller cannot distinguish the two from `written` alone, which is intentional: idempotent-success and first-write both mean "the log has it now"). |
| partial-failure | If `sink.append` fails on envelope *k* of *n*: envelopes `1..k-1` are already durably written (and safe to re-see on retry, being idempotent); `ingest_once` returns `Err(IngestError::Sink(..))` immediately without attempting `k+1..n` or returning a partial `IngestReport` — the caller's correct response is "retry the whole `ingest_once` call," which is safe by the same idempotence argument, not "resume from k." |

### `fn EventId::derive(source: SourceKind, kind: EventKind, external_id: &str) -> EventId`

| Input dimension | Behavior |
|---|---|
| empty | `external_id: ""` → still hashes deterministically (an empty string is a valid hash input); not a special case, not an error — if an adapter ever has no natural external id, that adapter itself must supply *some* stable substitute (e.g. the CLI adapter uses its own invocation nonce), never call this with a value that varies run-to-run for what should be the same logical event. |
| null / `None` | n/a — `external_id: &str` is non-nullable. |
| wrong-type | n/a — no other type accepted. |
| huge | `external_id` megabytes long (pathological adapter bug): blake3 hashing is linear in input length and has no practical size limit; no truncation needed before hashing (unlike `payload`, which is capped for storage/readability, not for hashing). |
| negative | n/a. |
| duplicate | The entire purpose of this fn: same `(source, kind, external_id)` in → byte-identical `EventId` out, always — this is not a bug case, it is the specified idempotence property (§9's `same_external_event_yields_same_id` test asserts exactly this). |
| concurrent | Pure value fn — trivially safe. |
| unicode / non-ASCII | Hashed as raw bytes (UTF-8) — a unicode `external_id` (an internationalized email `Message-ID`, say) hashes correctly and deterministically like any other string; no normalization performed (documented limit, not a bug: two byte-different-but-visually-identical unicode strings naming the "same" external id would hash differently — acceptable, since `external_id` is protocol-supplied, not user-typed prose). |
| already-exists | n/a — no persisted state in this fn; "already exists" is `EventSink`'s concern, downstream of this fn's output. |
| partial-failure | n/a — no IO, cannot fail partially. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace-path (`{ path = "../fleet-types" }`) | Supplies `Blake3Hash`, reused as `EventId`'s inner representation instead of redefining a hash-string newtype. |
| `serde` | `1.0.229` (match `fleet/keel/Cargo.lock`) | `EventEnvelope`/`EventId`/`GuardedAction` (de)serialize for storage and for the guard-report/CLI output. |
| `serde_json` | `1.0.151` (match lock) | `payload: Value` and its byte-length capping/truncation. |
| `thiserror` | `2.0.20` (match lock) | Typed error enums (`SinkError`, `IngestError`, each adapter's own error type) — no `String`/`anyhow` error anywhere in non-test code. |
| `blake3` | `1.8` | `EventId::derive`'s hash — not listed in the pre-build proposal, but load-bearing: `fleet-types::Blake3Hash` is a thin newtype and the actual `(source, kind, external_id)` hashing happens in this crate's own `ids.rs`, which needs the `blake3` crate directly. |
| `reqwest` (features = `["blocking", "json"]`) | `0.13` | GitHub REST API poller in `adapters/github.rs` (§5). |
| `notify` | `8.2.0` | Bounded-timeout filesystem poll in `adapters/fs_watch.rs` (§5). |
| `imap` | `2.4.1` | IMAP UID-cursor poller in `adapters/gmail.rs` (§5). |
| `mailparse` | `0.16.1` | MIME parsing of fetched messages in `adapters/gmail.rs` (§5). |
| `native-tls` | `0.2` | TLS transport for the `imap` connection in `adapters/gmail.rs` — not named in the pre-build proposal, added alongside `imap` since the crate does not provide its own TLS implementation. |

No async runtime (`tokio`) dependency — every adapter runs synchronously/blocking, consistent with
the spawn-on-demand, no-resident-daemon design; a future adapter that genuinely needs async is a
renegotiation of this choice, not a silent addition.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.**
>
> **As built** (`find crates/fleet-events -name '*.rs' \| sort`, line counts via `wc -l`), the split
> landed close to the pre-build sketch below with two differences: (1) `envelope.rs` was split
> further into `envelope.rs` (just `EventEnvelope`/`MAX_PAYLOAD_BYTES`) plus separate
> `source_kind.rs` and `event_kind.rs` files, to stay under the 80-line gate; (2) `adapters/github.rs`
> grew a sibling `github_map.rs` (the REST `type`-field → `EventKind` mapping) for the same reason —
> both are exactly the kind of further split §8's own note anticipated.

```
crates/fleet-events/
  Cargo.toml
  src/
    lib.rs             # 34 — module decls + re-exports only
    ids.rs             # 46 — EventId + derive()/as_blake3()
    source_kind.rs      # 23 — SourceKind
    event_kind.rs        # 50 — EventKind (+ side_effecting)
    envelope.rs           # 49 — EventEnvelope, MAX_PAYLOAD_BYTES
    guard.rs               # 46 — GuardedAction, MAX_QUOTE_BYTES, guard()
    clock.rs                # 38 — Clock trait + SystemClock
    sink.rs                  # 19 — SinkError, EventSink trait
    adapter.rs                # 18 — Adapter trait
    ingest.rs                  # 45 — IngestError, IngestReport, ingest_once()
    adapters/
      mod.rs                   # 12 — re-exports
      github.rs                 # 78 — GithubAdapter: reqwest poller, cursor = last delivery id/ETag, HMAC check
      github_map.rs               # 14 — maps the REST events-API `type` field to EventKind
      gmail.rs                     # 80 — GmailAdapter: imap poller, cursor = last UID, mailparse body extraction
      fs_watch.rs                   # 76 — FsAdapter: notify bounded-timeout poll, cursor = last snapshot
      cli.rs                         # 54 — CliAdapter: wraps a single pre-read argv/stdin invocation
  tests/
    id_derivation.rs        # 31 — EventId determinism/collision-avoidance
    guard_behavior.rs         # 72 — guard()'s full §6 table
    ingest_orchestration.rs    # 48 — ingest_once() with fake Adapter + fake EventSink
    ingest_orchestration_retry.rs # 36 — the "retry the whole batch after a sink failure" contract
    adapters_cli.rs             # 38 — CliAdapter end-to-end (no network/fs needed)
    support/mod.rs                # 53 — shared fakes (Adapter/EventSink/Clock) used across the above
```
> If any file above still projects over 80 lines once bodies land, split again (e.g.
> `adapters/github.rs` → `github_poll.rs` + `github_auth.rs`). The file-size gate (§10) runs before
> Opus review, same as every other crate.

`Cargo.toml` (as built):
```toml
[package]
name = "fleet-events"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-types = { path = "../fleet-types" }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.20"
blake3 = "1.8"
reqwest = { version = "0.13", features = ["blocking", "json"] }
notify = "8.2.0"
imap = "2.4.1"
mailparse = "0.16.1"
native-tls = "0.2"

[dev-dependencies]
tempfile = "3"
```

## 9. Test plan

**Unit tests:**
- `same_external_event_yields_same_id` — `EventId::derive` called twice with identical
  `(source, kind, external_id)` produces byte-identical `EventId`s.
- `distinct_sources_never_collide_on_shared_external_id` — the same `external_id` string under two
  different `(source, kind)` pairs produces two distinct `EventId`s.
- `side_effecting_is_exhaustive_and_payload_independent` — call `EventKind::side_effecting` for
  every variant with two different `payload`s constructed around it, assert the boolean result is
  identical regardless of payload content (guards the injection-proofing claim in §4 directly).
- `payload_over_cap_is_truncated_not_rejected` — an `EventEnvelope` built from a payload over
  `MAX_PAYLOAD_BYTES` is capped, never an error, never panics.

**Integration tests** (calling only the public API):
- `guard_quote_is_verbatim_and_capped` — for a payload under `MAX_QUOTE_BYTES`, `quote` is an exact
  match of the serialized payload; for one over, `quote`'s byte length is exactly
  `MAX_QUOTE_BYTES` plus the fixed truncation-marker overhead, and it is valid UTF-8 (no split
  codepoint).
- `guard_requires_confirmation_matches_side_effecting_exactly` — for every `EventKind` variant,
  `guard(...).requires_confirmation == kind.side_effecting()`, tying the two together explicitly
  rather than trusting them to agree by convention.
- `ingest_once_empty_pull_is_a_clean_success` — a fake `Adapter` returning `Ok(vec![])` yields
  `IngestReport { pulled: 0, written: 0, guarded: vec![] }`.
- `ingest_once_guards_every_pulled_envelope` — a fake `Adapter` returning N envelopes yields
  `report.guarded.len() == N` always, mixed side-effecting/non-side-effecting.
- `ingest_once_stops_at_first_sink_failure_and_is_safely_retryable` — a fake `EventSink` that fails
  on the k-th `append`: `ingest_once` returns `Err` without calling `append` for `k+1..n`; a second
  `ingest_once` call against a fake sink that now succeeds re-processes the whole batch without
  error (proving the "retry the whole cycle" contract in §6 is actually safe against the fake's own
  idempotence).
- `duplicate_id_within_a_batch_is_a_sink_no_op_not_a_crash` — two envelopes sharing an `id` in one
  adapter batch both get written without `ingest_once` panicking or erroring, relying on the fake
  sink's documented idempotence.
- `cli_adapter_produces_one_envelope_then_empties` — `CliAdapter` constructed with fixed argv/stdin
  yields exactly one envelope on the first `pull`, then `Ok(vec![])` on every subsequent `pull`.

**Mutation-testing targets** (`cargo mutants -p fleet-events`):
- Flipping `side_effecting`'s `true`/`false` for any one variant must be killed by
  `guard_requires_confirmation_matches_side_effecting_exactly` (it checks every variant, not a
  sample).
- Deleting the byte-cap check in `guard`'s quote construction (so an oversized payload is quoted in
  full) must be killed by `guard_quote_is_verbatim_and_capped`'s "over" case.
- Changing `ingest_once`'s early-return-on-sink-error to "continue past the failure" must be killed
  by `ingest_once_stops_at_first_sink_failure_and_is_safely_retryable`'s assertion that `append` is
  never called for `k+1..n`.
- Changing `EventId::derive` to hash `external_id` alone (dropping `source`/`kind` from the input)
  must be killed by `distinct_sources_never_collide_on_shared_external_id`.

**Property tests** (`proptest`, recommended given the injection-proofing claim this crate exists
to make):
- *Guard's confirmation requirement is a pure function of kind, for any payload*: for every
  `EventKind` variant, generate arbitrary JSON `payload` values (including deeply nested,
  non-ASCII, and empty) and assert `guard(...).requires_confirmation` never varies for a fixed
  `kind` — the direct, generalized regression test for the entire injection-surface design, at
  N = 200 cases minimum per variant.

## 10. Verification recipe

```bash
cd crates/fleet-events
cargo test -p fleet-events --all-targets
cargo clippy -p fleet-events --all-targets -- -D warnings
cargo mutants -p fleet-events
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration tests pass, 0 skipped — publish as `<passed>/<total>` (e.g.
`15/15`). Clippy: 0 warnings. Mutants: every target named in §9 caught — publish
`<caught>/<total mutants>`; given this crate is fleet's injection-surface boundary, the floor is
**100% of viable mutants caught** on `guard`/`EventKind::side_effecting`/`EventId::derive`
specifically (the security-load-bearing functions), not a blended percentage across the whole
crate — adapter-file mutants (network/IO call sites) may be harder to reach without live
network/mailbox fixtures and can be scoped out of the mutants run with `--exclude adapters/*` if
so, but that exclusion must be stated explicitly in the PR, never silently assumed.

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`SinkError`, `IngestError<E>`, each adapter's
      own `*AdapterError`) — none swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test
      code. (Mark done once the real adapter error enums are written and reviewed.)
- [ ] Clock/RNG/IO are injected: `Clock` is a trait, no RNG anywhere (`EventId` is a deterministic
      hash — §4), and every IO boundary is named explicitly in §4 rather than hidden.
- [ ] Thread-safety documented: `guard`/`EventId::derive` are pure value fns (`Send + Sync` for
      free); `ingest_once` takes `&mut A` (single-writer per adapter by the type system) and
      `&S`/`&dyn Clock` (safe to share only if the concrete `S`/`Clock` impl is `Sync` — documented
      per-impl, not crate-wide, since `EventSink`/`Clock` implementors live outside this crate).
- [ ] No float used anywhere in this crate (no money, no tokens — payload/quote byte caps are
      `usize`).
- [ ] No self-grading — verification runs `cargo mutants`, not just unit tests; denominator
      published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10, template) — restate
      the real numbers in the PR once the crate is built.
- [ ] Tests that touch the filesystem (the `fs_watch` adapter's tests, if any beyond the fake-based
      orchestration tests in §9) use `tempfile::TempDir` exclusively — never the repo tree or
      `$HOME`; the network/IMAP adapters are not live-tested in this crate's own suite (see §10's
      mutants-scoping note) — their fakes exercise `ingest_once`/`guard` instead.
- [ ] Every non-goal in §2 is absent from the code: no dispatch/routing decision, no direct
      durable-storage write outside the `EventSink` trait, no resident loop (`grep -rn
      "loop\s*{" crates/fleet-events/src/adapters/` should show only bounded/timeout-guarded loops,
      never an unconditional one).
- [ ] No source file exceeds 80 lines — §8's layout, verified by the §10 `wc -l ... awk '$1>80'`
      gate before Opus review.

## 12. Definition of Done

`fleet-events` is DONE when: §10's four commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught on the security-load-bearing fns per §10's floor) run
from `crates/fleet-events/`; every unchecked box in §11 is checked with its real numbers;
`registry/services/REGISTRY.md` lists the crate (infrastructure/ingress, not a product feature —
`services/`, per C1/L2); and Opus has independently re-derived why `guard`'s confirmation
requirement can never be influenced by payload content (re-deriving §4's injection-proofing
argument from this file alone, without re-reading this blueprint's prose a second time), reproduced
the "drop the byte-cap check" mutation by hand, and driven one real `ingest_once` call end-to-end
(a fake `Adapter` returning a side-effecting envelope, a fake `EventSink`) confirming
`guarded.len() == pulled` and `requires_confirmation` matches `side_effecting()` as claimed.

---

## Divergence from MIGRATION-PLAN (for Opus)

MIGRATION-PLAN's crate roster (§3 row 3) and this crate's own task brief both describe
`fleet-events` as depending on `fleet-types` "+ writes via fleet-store." Two things about that
framing needed a call this blueprint had to make explicitly rather than leave implicit:

1. **`fleet-store` has no blueprint yet (state `todo`, §5 of MIGRATION-PLAN, confirmed empty
   `../fleet-store/` on disk 2026-09-08) — so this blueprint cannot cite a real
   `fleet-store` signature the way it cites real `fleet-types` signatures.** Taking a hard
   compile-time dependency on an unspecified API right now would be premature coupling: whatever
   `fleet-store` eventually exposes (the plan says "add redb; unify" alongside the existing
   blake3-chained receipt ledger, `graph.rs`'s rusqlite, and `memory_store.py`'s sqlite-vec — four
   different storage shapes to reconcile) might not even be a single trait `fleet-events` could
   call directly. Instead, following the same "define the port the consumer needs, let the
   caller/producer satisfy it" pattern `fleet-router`'s `RuntimeState` already established for an
   equally not-yet-built neighbor (`fleet-govern`), this blueprint defines `EventSink` (§3D) as the
   seam: this crate depends on `fleet-store` **conceptually** (the port `EventSink` exists because a
   durable log exists) but not as a **compile edge** until `fleet-store`'s own blueprint fixes a
   real shape. **Recommendation for whoever writes `fleet-store`'s blueprint next:** either export a
   type implementing `fleet_events::EventSink` directly, or make clear in that blueprint's own §2
   that the composition root (`src/`) is expected to adapt between the two — either is fine, but it
   should be a stated decision in that blueprint, not an implicit assumption carried only here.
2. **"GitHub webhook" in the task brief and MIGRATION-PLAN's row 3 reads naturally as an inbound
   HTTP listener, but an inbound listener is itself a resident daemon** — it must stay up to receive
   pushed deliveries, which directly contradicts the brief's own "NO resident daemon
   (spawn-on-demand)" requirement one sentence later. This blueprint resolves the apparent tension
   by implementing GitHub ingestion as a `reqwest`-based **poller** against GitHub's REST API
   (`adapters/github.rs`, §5) rather than a webhook receiver — matching the brief's explicit library
   preference ("a thin reqwest-based poller") over its own more colloquial "webhook" framing. If
   genuine push-based webhook delivery is later required (lower latency than polling), that is a
   distinct component — an actual always-on HTTP service — outside this crate's non-goals (§2), not
   a duty this blueprint takes on under the `fleet-events` name.

Neither of these is a DAG violation (the DAG in MIGRATION-PLAN §3 explicitly permits
`fleet-events` to depend on `fleet-store`; this blueprint simply chooses not to exercise that edge
yet, for the reason above) — both are implementation-detail calls this blueprint is entitled to
make per the template's own precedence rule, flagged here per the brief's instruction to surface
gaps rather than silently pick.
