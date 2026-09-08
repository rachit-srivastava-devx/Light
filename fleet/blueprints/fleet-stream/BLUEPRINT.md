# BLUEPRINT — `fleet-stream`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-stream`
- **One-line purpose:** Tail the append-only receipt log durably, project each receipt into a
  `StreamEvent`, and fan it out to pluggable `Sink`s (dashboard, orb, meter, webhook, OTel, file)
  with per-sink at-least-once delivery, so nothing downstream of the ledger polls the ledger
  itself.
- **Build branch:** `partial+build` (MIGRATION-PLAN §3 row 12) — `telemetry_otel.py` (OTel export)
  and `console.rs` (the TUI dashboard) already exist and are read for their *projection* and
  *tailing* patterns; the unified `Sink` trait, the `pump` orchestration, and five of the six sink
  implementations (`OrbSink`, `DashboardSink`, `MeterSink`, `WebhookSink`, `FileSink`) are ABSENT
  and built new here. `OtelSink` wraps the existing script rather than re-implementing OTel.
- **Imports:** `fleet-types` (the `Receipt`/`ReceiptEvent`/`ExitCode` wire types every `StreamEvent`
  wraps); reads the ledger through a `LogSource` trait this crate defines and `fleet-store` is
  expected to implement (see the divergence note — `fleet-store` has no blueprint yet, so this is
  this crate's proposed shape of that seam, not a confirmed contract).
- **Imported by:** `src/` (composition root — wires a concrete `LogSource`/`CursorStore` from
  `fleet-store` and the configured sink list, then calls `pump`). No sibling crate depends on
  `fleet-stream` — it is a terminal leaf on the egress side of the DAG, the mirror of `fleet-events`
  on the ingress side (per the brief: "it owns the EGRESS plane — the mirror of fleet-events
  ingress").

## 2. Responsibility & non-goals

**Owns:** the egress pump loop — reading new receipts once (via the injected `LogSource`), turning
each into a `StreamEvent`, deciding per sink whether it wants it (`Sink::accepts`), delivering it
with retry (`Sink::deliver`), and persisting each sink's own durable cursor so a process restart
resumes exactly where it left off rather than replaying the whole ledger or silently skipping rows.
It also owns the six `Sink` implementations named in the brief — each is a thin adapter from
`StreamEvent` to one external protocol (SSE/WS, HTTP POST, NDJSON, a subprocess call), never a
second copy of ledger-reading logic.

**Non-goals (the seam):**
- Does **not** write to the ledger, compute `blake3` hashes, or own the receipt file itself —
  that's `fleet-store`'s job (C6, "own your data"). This crate only ever calls `LogSource::poll_since`
  and never opens `ledger/chain.jsonl` directly.
- Does **not** decide routing, scheduling, or gate verdicts — a `StreamEvent` is a fact already
  decided elsewhere (`fleet-router`, `fleet-govern`, `fleet-verify`); this crate only observes and
  forwards it.
- Does **not** run a resident daemon. `DashboardSink`'s axum server is started in-process, per run,
  bound to an ephemeral localhost port, and torn down when the pump stops — never a background
  service the operator must remember to kill (T4 doctrine: no ambient state beyond what one run
  creates and destroys).
- Does **not** re-implement the OTel wire protocol in Rust. `OtelSink` shells out to the existing,
  already-adversarially-scoped `telemetry_otel.py` (see §5) — this crate does not depend on the
  `opentelemetry` Rust crate (absent from `fleet/keel/Cargo.lock` and absent from the brief's
  dependency list) and must not add it.
- Does **not** fabricate a dollar figure. `MeterSink` reports only what it can measure (estimated
  tokens, window percentage) and a labelled `$` figure only when the receipt body itself carried
  one — it never derives a price from a token count and a guessed rate (see §6's honesty rule).
- Does **not** retry forever. A sink's `deliver` failure is either `Transient` (retried, bounded by
  `PumpConfig::max_retries`) or `Permanent` (recorded and the cursor still advances past it, because
  an unbounded retry on a permanently-rejecting sink would silently stall every OTHER sink sharing
  the same pump — except sinks are independent workers, so this is actually about not stalling
  *that one sink* forever on one bad event).

## 3. Public API contract

```rust
//! The egress plane: tail the append-only receipt log, project each row, fan out to pluggable
//! sinks. The mirror of `fleet-events` on the ingress side.
//!
//! This crate never opens the ledger file itself -- every fact about "what's new" arrives through
//! `LogSource`, injected by the caller (in production, `fleet-store`). Each `Sink` is delivered to
//! independently: one sink stalling (network down, disk full) never slows or drops events for any
//! other sink, because each sink owns its own durable cursor and its own bounded read-ahead queue.

use fleet_types::Receipt;
use std::time::Duration;

// =====================================================================================
// A. The event and the read boundary
// =====================================================================================

/// One ledger receipt as it flows through the egress pump. A thin wrapper (not a bare alias) so
/// this crate can add pump-internal bookkeeping later without changing `fleet_types::Receipt`,
/// which is the wire type every OTHER crate also shares.
#[derive(Clone, Debug)]
pub struct StreamEvent(pub Receipt);

impl StreamEvent {
    /// The ledger's own monotonic sequence number -- the sole dedupe/ordering key every sink cursor
    /// is keyed on (never a wall-clock timestamp, never an insertion-order guess).
    pub fn seq(&self) -> u64 { self.0.seq }
}

/// The injected read boundary. Implemented by `fleet-store` in production; a test fake backs this
/// crate's own suite. Deliberately `&self`, not `&mut self`: a real store answers "give me every
/// receipt after seq N" as a stateless query (like a SQL range scan), which is what lets every
/// sink's worker poll independently without sharing a mutable cursor -- the pattern `console.rs`'s
/// `refresh_ledger`/`LedgerTail` approximates today with a raw byte offset (see §5), reshaped here
/// into a seq-addressed query because `fleet-store` is expected to be a real store, not a raw file.
pub trait LogSource: Send + Sync {
    /// Every receipt with `seq > after` (or every receipt, if `after` is `None`), in ascending seq
    /// order. Must be pure w.r.t. its own state: two calls with the same `after` against an
    /// unchanged log return the same receipts -- the log is append-only, so this holds by
    /// construction as long as the implementer never mutates or reorders a row already returned.
    fn poll_since(&self, after: Option<u64>) -> Result<Vec<Receipt>, LogSourceError>;
}

/// Why a `LogSource::poll_since` call failed.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LogSourceError {
    /// The store is temporarily unreachable (connection refused, file locked) -- retry later.
    #[error("log source unavailable: {0}")]
    Unavailable(String),
    /// The source returned a receipt whose `seq` did not strictly increase, or was `<= after` --
    /// an invariant violation in the store, never expected in normal operation.
    #[error("log source returned seq {got} out of order (expected > {expected})")]
    OutOfOrder { expected: u64, got: u64 },
}

// =====================================================================================
// B. The durable per-sink cursor
// =====================================================================================

/// The durable resume position for one sink: the highest seq it has successfully delivered.
/// Persisted by whatever `CursorStore` the caller injects (in production, `fleet-store`) -- this
/// crate defines the shape, not the persistence mechanism.
pub trait CursorStore: Send + Sync {
    /// `Ok(None)` means this sink has never delivered anything -- a fresh sink starts from the
    /// beginning of the log, never from "now" (an omitted event is a correctness bug, not a
    /// performance shortcut).
    fn load(&self, sink_id: &'static str) -> Result<Option<u64>, CursorError>;
    /// Persist `seq` as the new resume point. Must be durable before returning `Ok` -- a crash
    /// immediately after this call and before the next `deliver` must still resume correctly
    /// (fsync or equivalent is the implementer's job, not this trait's, but the contract requires
    /// it: `load` after a crash must never return a seq that was not actually fully delivered).
    fn save(&self, sink_id: &'static str, seq: u64) -> Result<(), CursorError>;
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("cursor store error for sink {sink_id:?}: {reason}")]
pub struct CursorError { pub sink_id: &'static str, pub reason: String }

// =====================================================================================
// C. The sink trait
// =====================================================================================

/// One delivery target. Implementations must never block the pump indefinitely inside `accepts`
/// (a pure, fast predicate) and must treat `deliver` as the one place real IO happens.
pub trait Sink: Send {
    /// Stable identity: the `CursorStore` key and every log line/metric about this sink names it.
    /// Never renamed once shipped -- renaming silently resets that sink's resume position to "from
    /// the beginning," which is a correctness event, not a cosmetic one.
    fn id(&self) -> &'static str;
    /// Whether this sink wants to see `event` at all. Checked before `event` is ever queued, so
    /// e.g. `OrbSink` (accepts only `lane_status`) never even buffers a `run_start`.
    fn accepts(&self, event: &StreamEvent) -> bool;
    /// Attempt one delivery. `&mut self` because a sink typically owns a live connection/handle
    /// (an HTTP client, a broadcast sender, an open file) that delivery mutates or writes through.
    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError>;
}

/// Why a delivery attempt failed, and whether `pump` should retry it.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// Retry-worthy: the sink (or the network to it) is temporarily unavailable. `pump` retries up
    /// to `PumpConfig::max_retries` with `PumpConfig::retry_backoff` before giving up on this one
    /// event (see `Permanent` for what "giving up" means).
    #[error("sink {sink} temporarily unavailable: {reason}")]
    Transient { sink: &'static str, reason: String },
    /// Not retry-worthy: the sink has permanently rejected this specific event (e.g. a webhook
    /// endpoint returned 4xx). `pump` records it and advances the cursor past it rather than
    /// stalling this sink on one poison event forever.
    #[error("sink {sink} permanently rejected event seq {seq}: {reason}")]
    Permanent { sink: &'static str, seq: u64, reason: String },
}

// =====================================================================================
// D. The pump
// =====================================================================================

/// Tuning for one sink's worker loop. Every sink gets its own `PumpConfig` (they may differ --
/// e.g. `FileSink` can retry harder than `DashboardSink`, whose consumer disappearing is normal).
#[derive(Clone, Debug)]
pub struct PumpConfig {
    /// How many receipts `poll_since` is allowed to hand this worker in one cycle before the rest
    /// wait for the next cycle -- the "bounded per-sink queue." Never drops a receipt: an
    /// over-the-cap remainder simply is not fetched this cycle, because the next cycle's
    /// `poll_since(cursor)` re-derives it from the durable log, not from an in-memory buffer. This
    /// is what makes a slow sink's backlog bounded in memory without ever losing an event.
    pub queue_capacity: usize,
    /// How long to sleep between poll cycles when nothing new was found.
    pub poll_interval: Duration,
    /// How many times to retry a `Transient` failure on the same event before treating it as if it
    /// were `Permanent` (logged, cursor still advances) -- an unbounded retry would let one flaky
    /// event stall this sink's cursor forever.
    pub max_retries: u32,
    /// Backoff between retries of the same event: `base * 2^attempt`, capped at `max`.
    pub retry_backoff: RetryBackoff,
}

#[derive(Clone, Copy, Debug)]
pub struct RetryBackoff { pub base: Duration, pub max: Duration }

/// Per-sink outcome counters, returned when a sink's worker loop stops (on shutdown or a fatal
/// `LogSourceError`). Not a substitute for the sink's own tests -- see §10/§11's no-self-grading
/// rule -- but the minimum an operator needs to know whether a sink is keeping up.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SinkStats {
    pub delivered: u64,
    pub retried: u64,
    pub permanently_skipped: u64,
    pub last_delivered_seq: Option<u64>,
}

/// Run one sink's worker loop to completion (until `shutdown` resolves). Pure orchestration: reads
/// `cursors.load`, calls `source.poll_since`, filters by `sink.accepts`, delivers each survivor in
/// ascending seq order with retry, and calls `cursors.save` only after a successful (or
/// exhausted-`Permanent`) outcome for that seq -- so `cursors.save` is always called with a
/// strictly increasing `seq` and is the single place "this event is done" is decided.
///
/// # Panics
/// Never. Every fallible path returns inside `SinkStats`'s bookkeeping or ends the loop on a fatal
/// `LogSourceError`; a single event's delivery failure never panics or aborts the loop.
pub async fn run_sink(
    source: &(dyn LogSource + 'static),
    cursors: &(dyn CursorStore + 'static),
    sink: &mut (dyn Sink + 'static),
    config: &PumpConfig,
    shutdown: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<SinkStats, LogSourceError> {
    unimplemented!("see §6 for the exact per-cycle behavior; §9 lists the tests this body must pass")
}

/// Spawn one `run_sink` task per sink and run them concurrently until `shutdown` fires. This is the
/// crate's one public "just run it" entry point; `src/` calls this once at process start with the
/// concrete `LogSource`/`CursorStore` it built from `fleet-store` and the sink list assembled from
/// config.
pub async fn pump(
    source: std::sync::Arc<dyn LogSource + 'static>,
    cursors: std::sync::Arc<dyn CursorStore + 'static>,
    sinks: Vec<(Box<dyn Sink + 'static>, PumpConfig)>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Vec<Result<SinkStats, LogSourceError>> {
    unimplemented!("join_all over one run_sink task per (sink, config) pair, each with its own \
                    cloned shutdown receiver")
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `StreamEvent` | Wraps a `fleet_types::Receipt` verbatim; `seq()` is always `self.0.seq` — never recomputed or re-derived. | A `StreamEvent` whose ordering key disagrees with the ledger's own `seq`, which would break every sink's cursor math silently. |
| `LogSource::poll_since` | Callers pass `after` as an *exclusive* lower bound; every returned `Receipt.seq` is strictly greater than `after` and strictly increasing across the returned `Vec`. Violated → `LogSourceError::OutOfOrder`, never silently re-sorted. | A sink processing the same receipt twice within one `poll_since` call, or processing them out of order, either of which would corrupt a cursor that assumes strict monotonic progress. |
| `Cursor` (the `(sink_id, last_delivered_seq)` pair `CursorStore` persists) | `last_delivered_seq` only ever increases for a given `sink_id`, and only after `deliver` returned `Ok` (or was downgraded from `Transient` to abandoned-as-`Permanent` after `max_retries`). | A cursor silently rewinding (which would redeliver a large historical backlog to a sink that already saw it) or silently skipping ahead (which would drop events a sink never actually saw). |
| `SinkError` | Exactly two variants, `Transient`/`Permanent`; a sink author cannot represent "retry a few times then give up silently" — `pump` alone owns the retry-count/backoff policy in `PumpConfig`, a sink cannot smuggle its own retry loop into `deliver` and hide failures from the pump's stats. | A sink whose `deliver` swallows its own errors and returns `Ok` after a failed send, which would advance the cursor past an event that was never actually delivered. |
| `PumpConfig.queue_capacity` | A `usize` bound on receipts processed per poll cycle; never interpreted as "drop the excess" — the excess is simply left for the next `poll_since(cursor)` call, which re-derives it from the durable log. | A "bounded queue" implementation that silently drops receipts on overflow instead of merely deferring them — the property the brief requires ("a slow sink never backpressures the pipeline") without becoming data loss. |
| `SinkStats` | `delivered + permanently_skipped` only ever grows; `last_delivered_seq` is `None` until at least one event has been fully resolved (delivered or permanently skipped) for that sink. | A stats snapshot that looks like progress happened when in fact every event since start is still `Transient`-retrying. |

**Money/precision:** `MeterSink`'s labelled dollar figure (§9's `sinks/meter.rs`) is carried as
integer cents (`u64`), never `f32`/`f64`, matching `fleet-types`' own `Tokens`/money convention —
this crate never invents a new money representation. Estimated token counts are `u64`; a Wilson-
score-style rate, if ever surfaced by a future sink, is a measurement statistic, not money, and
stays a float only in that narrow role (none exists in this crate today).

**Clock/RNG/IO injection points:** `LogSource`/`CursorStore` are the sole read/persist boundaries
(injected, per §3). `PumpConfig.poll_interval`/`retry_backoff` are durations, not read from a clock
inside the pump — the actual sleep/backoff wait uses `tokio::time::sleep`, which is the one place
this crate touches wall-clock time, and it is isolated to `pump`'s scheduling loop, never to any
`Sink`'s decision logic. Each `Sink` implementation names its own IO boundary in §9's file layout
(reqwest for `OrbSink`/`WebhookSink`, axum/broadcast for `DashboardSink`, `std::process::Command`
for `OtelSink`, `std::fs::File` for `FileSink`) — every one is the sink's own `deliver`, never
ambient, and every sink is constructed with its IO handle already open/configured, not conjured
from an environment variable read deep inside `deliver`.

## 5. Reuse map

Source read in full: `fleet/registry-reference/registry/features/telemetry/telemetry_otel.py` (171
lines) and `fleet/keel/fleet/src/console.rs` (1499 lines, focusing on `LedgerTail`/`refresh_ledger`/
`load_ledger`/`load_lane_status`, lines 716–980); `fleet/contracts/receipt.v1.json` and
`fleet/contracts/lane-status.v1.json` (both cited in `fleet-types`' own blueprint, reused here by
name, not by re-deriving the schema).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `console.rs:716-724` (`LedgerTail`) | A byte-offset cursor over `ledger/chain.jsonl`, trimmed to the last newline so a half-written row is never parsed. | no — reshaped | This crate's `CursorStore`/`LogSource` split replaces byte offsets with `seq`-addressed queries, because `fleet-store` (the intended `LogSource` implementer) is expected to be a real store capable of "give me everything after seq N," not a raw file this crate re-parses. The *idea* (never re-process bytes/rows already consumed; recover cleanly if the underlying log was truncated/rewritten) is what's reused — see `refresh_ledger`'s `size < tail.offset` truncation-recovery check, mirrored in this crate's requirement that `LogSourceError::OutOfOrder` is a named, handled case, not a panic. |
| `console.rs:756-800` (`refresh_ledger`) | Re-reads only appended bytes, trims to the last full line, parses each new line as JSON, and calls `load_ledger` on the batch. | logic yes, mechanism no | The "only process what's new since last time, in order, and never re-emit an already-seen row" behavior is exactly `run_sink`'s per-cycle contract (§3, §6) — reused as *behavior*, not as file-reading code, since the byte-offset mechanism itself is `fleet-store`'s concern, not this crate's. |
| `console.rs:802-925` (`load_ledger`) | Dispatches on a receipt's `event` field (`run_start`/`run_end`/`refusal`/`gate_verdict`/`lane_status`/`artifact_frozen`/`attested`) to update one shared `ConsoleState`. | no — different consumer shape | This crate does not build one shared mutable projection; each `Sink` does its own narrow, stateless-per-event projection (`OrbSink` only cares about `lane_status`; `MeterSink` only cares about cost/token fields). The *pattern* — "one receipt stream, many different narrow projections of it, dispatched on `event`/body fields" — is exactly what `Sink::accepts` + each sink's own `deliver` body reuses; the console's single combined `ConsoleState` is TUI-specific and does not port. |
| `console.rs:930-980` (`load_lane_status`) | Projects a `lane_status` receipt's `body` + the receipt's own `seq`/`hash` envelope into a `LaneRecord`, matching `lane-status.v1.json`'s `ledger_ref`-stamped-after-append shape. | yes, as `OrbSink`'s projection | `OrbSink`'s `deliver` (§9) builds the *exact* `lane-status.v1.json` shape this function builds — `lane_id`/`role`/`state`/`agent`/`resolved_model` from `body`, `ledger_ref: {seq, hash}` from the receipt envelope — and POSTs it, per `lane-status.v1.json`'s own comment naming this as "fleet-rs's half of the design-graph / lane-status object ... the orb repo['s] ... voice/graph UI ... consumes this." |
| `console.rs:1203-1208` (`parse_cost`) | Reads `body.cost_cents` (falling back to `body.cost`) as the only money-shaped field in the whole file. | yes, as `MeterSink`'s labelled-`$` field | Confirms no other money field exists anywhere else in fleet's receipt bodies — `MeterSink`'s "optional labelled $" is this exact field, read the same way, never fabricated from a token count. |
| `telemetry_otel.py:14-62` (`cmd_span`) | Builds one Phoenix/OTel span per event, tagging `fleet.component`/`fleet.task_id`/`fleet.attempt`/`fleet.exit_code`/`fleet.token_source`, and emits `gen_ai.client.token.usage` events split by `gen_ai.token.type=input|output` — "no hand-rolled OTel plumbing" (the file's own header comment). | yes, called as a subprocess | `OtelSink::deliver` (§9) shells out to `python3 telemetry_otel.py span` with the same JSON-on-stdin shape this function expects (`event`, `component`, `task_id`, `attempt`, `exit_code`, `token_source`, `model`, `provider`, `tokens_in`, `tokens_out`, `duration_ms`), reading back `{trace_id, span_id}` from stdout. This crate does **not** reimplement OTel wiring in Rust — the file's own comment ("the only code in fleet that imports phoenix/openinference/tiktoken") is the reason to keep it that way. |
| `telemetry_otel.py:160-171` (the `CMDS` dispatch + exit-code contract) | `unknown command` → exit 2; any exception → `{"error": ...}` on stderr, exit 4; success → one JSON object on stdout. | yes, as `OtelSink`'s subprocess contract | `OtelSink` treats exit 0 + parseable stdout as `Ok`, any other exit code or unparseable stdout as `SinkError::Transient` (the script's own failures are typically environment: Phoenix not running, tiktoken model unknown) — never `Permanent`, since a transient OTel collector outage is exactly the retry-worthy case. |
| `fleet/contracts/lane-status.v1.json` (whole file) | JSON-Schema wire contract for the lane-status projection, `additionalProperties: false`. | greenfield Rust mirror inside `OrbSink` | `OrbSink`'s outbound JSON body matches this schema field-for-field; this crate does not import `fleet-types` types for it beyond `Receipt` itself, because `fleet-types`' blueprint does not define a `LaneStatus` wire type (only `Receipt`/`Attestation`) — see the divergence note. |

## 6. Behavior spec

### `async fn run_sink(source, cursors, sink, config, shutdown) -> Result<SinkStats, LogSourceError>`

| Input dimension | Behavior |
|---|---|
| empty | `source.poll_since(cursor)` returns `Ok(vec![])` → no events accepted, no `deliver` call, loop sleeps `poll_interval` and retries; `SinkStats` unchanged (all fields stay at their prior values, starting at `Default` — all zero, `last_delivered_seq: None`). |
| null / `None` | `cursors.load(sink.id())` returns `Ok(None)` (a fresh sink) → treated as `after: None`, i.e. `poll_since(None)` — the sink starts from the very first ledger row, never from "now." |
| wrong-type | n/a at this fn's boundary — `Receipt` is already a typed, deserialized value by the time it reaches `StreamEvent`; a malformed JSON row is `fleet-store`'s/`LogSource`'s problem to reject before it ever reaches `poll_since`'s `Ok` return (this crate's contract assumes `LogSource` only ever returns well-formed `Receipt`s, same assumption `fleet-types`' own blueprint makes about its callers). |
| huge | `poll_since` returns 100,000 receipts in one call (e.g. after a long outage) → only the first `config.queue_capacity` (post-`accepts`-filter) are processed this cycle; the remainder is not fetched again from this same `Vec` — the next cycle's `poll_since(new_cursor)` re-derives them from the durable log, so nothing is lost, but latency to catch up is `ceil(backlog / queue_capacity)` cycles, disclosed here rather than left as a surprise. |
| negative | n/a — `seq: u64`, no negative values representable; a cursor can never be negative and `poll_since`'s `after: Option<u64>` cannot underflow. |
| duplicate | Two receipts sharing the same `seq` in one `poll_since` result → `LogSourceError::OutOfOrder` (the second one is not `> ` the first, so it fails the strict-increase check) — this crate never silently dedupes by re-checking `seq` equality; a source that can produce duplicate seqs is itself broken and must be fixed at the source, not papered over here. |
| concurrent | Each sink's `run_sink` task is independent — no shared mutable state between two sinks' loops (each has its own `cursor`, its own `Sink` instance, its own retry counters). Two sinks reading the same `LogSource` concurrently is safe because `poll_since` takes `&self` (§3) and is documented as side-effect-free from the caller's perspective. |
| unicode / non-ASCII | A `Receipt.body` containing non-ASCII text (e.g. a unicode brief) flows through unchanged — `StreamEvent` does no string processing of its own; only individual `Sink`s that serialize `body` (all six do, ultimately) inherit `serde_json`'s UTF-8-native behavior, same as `fleet-types`' own note on `Receipt`'s round-trip. |
| already-exists | Calling `run_sink` twice concurrently for the *same* `sink.id()` against the *same* `CursorStore` is a caller error this crate does not defend against structurally (no distributed lock) — `pump`'s contract (one task per `(sink, config)` pair, spawned once) is the documented way this is avoided; a future requirement for multi-process fan-out of the same sink id would need `CursorStore` to add compare-and-swap semantics, which is out of scope today. |
| partial-failure | `deliver` returns `Err(Transient)` → retried up to `config.max_retries` times with `retry_backoff`, without advancing past that `seq` — every OTHER accepted event in the same batch still waits behind it (order is preserved: `pump` never delivers `seq` N+1 before `seq` N has resolved for a given sink, so a stuck event blocks only that one sink, never a burst of unrelated reordering). After `max_retries` is exhausted, the event is treated as `Permanent` (logged with its original `Transient` reason), `SinkStats.permanently_skipped` increments, and the cursor still advances past it — a permanently-stuck-forever sink is a worse failure mode than "this one event was dropped after genuinely trying." |

### `Sink` implementations — one behavior-spec row per sink's `accepts`/`deliver` (full tables in §9's per-file tests; summarized here)

| Sink | `accepts` | `deliver` boundary | Failure classification |
|---|---|---|---|
| `OrbSink` | `event.0.event == ReceiptEvent::LaneStatus` only | `reqwest::blocking::Client::post` to a configured local URL, body = the `lane-status.v1.json` projection (§5) | HTTP 5xx/connect-refused → `Transient`; HTTP 4xx (schema rejected by the orb side) → `Permanent`. |
| `DashboardSink` | every event (the dashboard shows everything; a client-side filter is a UI concern, not this sink's) | `tokio::sync::broadcast::Sender<StreamEvent>::send` — the axum SSE/WS handlers are just subscribers of this same channel; the HTTP server itself is started once, lazily, on first `deliver`, bound to `127.0.0.1:0` (ephemeral port) | A `send` with zero subscribers is not an error (`broadcast::Sender::send` returning `Err(SendError)` for "no receivers" is treated as a no-op success — a dashboard nobody is watching is not a delivery failure); a bind failure at startup is `Permanent` (surfaced once, not retried per-event). |
| `MeterSink` | receipts whose `body` carries any of `cost_cents`/`cost`/`tokens_in`/`tokens_out`, or `event == GateVerdict` (fleet-govern-authored verdicts often co-occur with cost) | Pure in-process aggregation into a `MeterSample` (§9) exposed via a `std::sync::mpsc`/shared `Mutex<MeterSnapshot>` the composition root reads — no network/file IO of its own; the "delivery" is making the sample observable, not shipping it anywhere | Never `Transient`/`Permanent` in practice (no IO to fail) — the one failure mode is a poisoned internal mutex, surfaced as `Permanent` (this sink cannot recover mid-process from a poisoned lock). |
| `WebhookSink` | Configurable allow-list of `ReceiptEvent` variants (from `WebhookConfig`); default = all | `reqwest::blocking::Client::post` to the configured URL with the raw `Receipt` as JSON body, an optional bearer token header | Same HTTP classification as `OrbSink`. |
| `OtelSink` | every event (span-worthy) | `std::process::Command::new("python3").arg(telemetry_otel_path).arg("span")`, JSON on stdin, per §5's field mapping | Non-zero exit or unparseable stdout → `Transient` (matches telemetry_otel.py's own exit-4-on-exception convention — an environment issue, not a permanent rejection); exit 2 (unknown command — a programmer error in this crate, not the environment) → `Permanent`, since retrying will never fix it. |
| `FileSink` | configurable (default: all) | `std::fs::OpenOptions::append(true)` + one `serde_json::to_writer` + `\n` per event, to a configured path — zero server, zero network | Disk-full / permission-denied → `Transient` (matches the retry-then-give-up policy; a full disk is often transient in practice); this sink never returns `Permanent` on its own (a well-formed JSON write to an open file handle does not "permanently reject" a specific event the way an HTTP 4xx does). |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace path (`{ path = "../fleet-types" }`) | `Receipt`/`ReceiptEvent`/`ExitCode` — the wire shape every `StreamEvent` wraps. |
| `tokio` | `1.53.1` (matches `fleet/keel/Cargo.lock`'s already-resolved version) | `run_sink`/`pump` are async (concurrent per-sink workers); `tokio::sync::{watch,broadcast}`, `tokio::time::sleep`. |
| `serde` | `1.0.229` (matches lock) | Every sink's outbound body derives/uses `Serialize` on `Receipt`/the lane-status projection. |
| `serde_json` | `1.0.151` (matches lock) | (De)serializing sink payloads; `FileSink`'s NDJSON writer. |
| `thiserror` | `2.0.20` (matches lock) | Every fallible op returns a typed error enum (`LogSourceError`, `CursorError`, `SinkError`) — this crate's hard rule against `String`/`anyhow`. |
| `axum` | `0.8.1` (not yet in `fleet/keel/Cargo.lock` — a genuinely new dependency per the brief; verify the exact latest 0.8.x patch via `cargo add axum` at build time rather than trusting this pin blindly) | `DashboardSink`'s per-run HTTP server: SSE via `axum::response::sse` and WS via `axum::extract::ws`, both built on the `ws`/default features. |
| `tokio-tungstenite` | `0.24.0` (new; verify latest patch at build time) | Transitively required by axum's `ws` feature for the WebSocket upgrade handshake; pinned directly here (per the brief's dependency list) so its version is under this crate's explicit control rather than left to Cargo's default resolution. |
| `reqwest` | `0.12.9` (new; verify latest patch at build time), `features = ["json", "blocking"]` | `OrbSink`/`WebhookSink`'s outbound HTTP POST. The `blocking` feature is used deliberately: each sink's `deliver` runs inside its own dedicated `run_sink` tokio task, so a short blocking HTTP call inside one sink's turn does not stall any other sink's task (each has its own task, not a shared executor thread pool contended for CPU-bound work) — documented here rather than silently reaching for the async client and adding avoidable complexity to a fire-and-forget POST. |

No other crate is needed: `OtelSink` uses `std::process::Command` (already in `std`, no new dep);
`FileSink` uses `std::fs`. This keeps the new-dependency surface exactly the three crates the brief
names (`axum`, `tokio-tungstenite`, `reqwest`) plus their one transitive addition already listed.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** `lib.rs` is a thin hub; six sinks each get their own
> file (per the brief: "Six sink impls, each its own ≤80-line file"), splitting further where an
> implementation's HTTP/process/file-IO plumbing pushes it over.

```
crates/fleet-stream/
  Cargo.toml
  src/
    lib.rs              # ~20 — module decls + re-exports only
    event.rs             # ~15 — StreamEvent
    log_source.rs         # ~30 — LogSource, LogSourceError
    cursor.rs              # ~35 — CursorStore, CursorError
    sink.rs                 # ~40 — Sink, SinkError
    pump/
      mod.rs                # ~10 — re-exports
      config.rs             # ~30 — PumpConfig, RetryBackoff
      stats.rs               # ~25 — SinkStats
      worker.rs               # ~75 — run_sink: poll/filter/deliver/retry/persist per cycle
      orchestrate.rs           # ~40 — pump: spawns one worker task per (sink, config)
    sinks/
      mod.rs                 # ~15 — re-exports
      orb.rs                  # ~70 — OrbSink: lane_status filter + lane-status.v1 projection + POST
      dashboard/
        mod.rs                 # ~15 — DashboardSink struct + Sink impl (delegates to server/broadcast)
        server.rs               # ~70 — lazy axum app bring-up on 127.0.0.1:0, SSE + WS routes
        broadcast.rs             # ~35 — the broadcast::Sender<StreamEvent> + subscribe helper
      meter.rs                  # ~75 — MeterSink: est-tokens/window%/labelled-$ MeterSample
      webhook.rs                 # ~70 — WebhookSink: config-only allow-list + reqwest POST
      otel.rs                     # ~70 — OtelSink: subprocess to telemetry_otel.py span
      file.rs                     # ~55 — FileSink: append-only NDJSON writer
  tests/
    log_source_fake.rs            # ~40 — shared in-memory LogSource/CursorStore test fakes
    run_sink_ordering.rs            # ~75 — ordering/backlog/cursor-advance behavior (§9)
    run_sink_retry.rs                 # ~70 — Transient retry-then-Permanent-downgrade behavior
    sink_accepts_filtering.rs           # ~55 — accepts() per sink type, table-driven
    file_sink_ndjson.rs                  # ~45 — FileSink writes valid NDJSON, one line per event
    orb_sink_projection.rs                 # ~60 — OrbSink's body matches lane-status.v1.json shape
```
> If `worker.rs` or `sinks/meter.rs`/`sinks/dashboard/server.rs` still project over 80 lines once
> bodies land, split further (e.g. `worker.rs` → `worker_cycle.rs` + `worker_retry.rs`). Verify with
> `find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80'` before Opus review.

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-stream"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-types = { path = "../fleet-types" }
tokio = { version = "1.53.1", features = ["rt-multi-thread", "macros", "time", "sync", "process"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.20"
axum = { version = "0.8.1", features = ["ws"] }
tokio-tungstenite = "0.24.0"
reqwest = { version = "0.12.9", features = ["json", "blocking"] }

[dev-dependencies]
tokio = { version = "1.53.1", features = ["test-util"] }
tempfile = "3"
```

## 9. Test plan

**Unit tests:**
- `stream_event_seq_matches_receipt_seq` — `StreamEvent(receipt).seq() == receipt.seq` for a table
  of representative receipts (varying `event` kinds).
- `pump_config_queue_capacity_bounds_but_never_drops` — a fake `LogSource` returning 500 receipts in
  one `poll_since` call, `queue_capacity: 50` → the first cycle processes exactly 50; the next cycle
  (after cursor advances) fetches and processes the remaining 450 — total processed across both
  cycles equals 500, none skipped.
- `sink_error_transient_vs_permanent_are_the_only_variants` — exhaustively match `SinkError`
  (compiles without a wildcard arm), documenting the two-variant closed set from §4.
- `orb_sink_accepts_only_lane_status` — table over all 7 `ReceiptEvent` variants: `OrbSink::accepts`
  returns `true` only for `LaneStatus`.
- `meter_sink_never_fabricates_a_dollar_figure` — a receipt body with `tokens_in`/`tokens_out` but no
  `cost_cents` → `MeterSample.labelled_cost_cents == None` (never computed from tokens * a guessed
  rate) while `estimated_tokens` is populated — the honesty rule from §2/§6, pinned as a test so a
  future "helpful" change can't silently start guessing a price.

**Integration tests** (calling only the public API, against the in-memory `LogSource`/`CursorStore`
fakes in `tests/log_source_fake.rs`):
- `run_sink_resumes_from_persisted_cursor` — deliver events 1..=5 to a fake sink, crash (drop the
  worker without processing 6..=10), restart `run_sink` with the same `CursorStore` → only 6..=10
  are delivered, 1..=5 are never re-delivered (at-least-once, not re-processed-from-scratch).
- `run_sink_transient_failure_retries_then_advances` — a sink whose `deliver` fails `Transient` for
  the first `max_retries` attempts on one event, then succeeds → `SinkStats.retried >= max_retries`,
  `delivered` includes that event, cursor advances past it.
- `run_sink_permanent_failure_does_not_stall_later_events` — event 5 always fails `Permanent`; events
  1-4 and 6-10 must still all be delivered and the cursor must reach 10, with
  `SinkStats.permanently_skipped == 1`.
- `two_sinks_progress_independently` — one sink deliberately blocked (`Transient` forever, capped at
  `max_retries`), a second sink delivering normally against the same `LogSource` → the second sink's
  cursor reaches the log's tip while the first is still retrying — proves "a slow sink never
  backpressures the pipeline" is real, not just documented.
- `orb_sink_projection_matches_lane_status_schema` — construct a `lane_status` receipt with a known
  body, call `OrbSink::deliver` against a local test HTTP server (e.g. a `tiny_http`/`axum` test
  listener), assert the received JSON body validates against `fleet/contracts/lane-status.v1.json`'s
  required fields (`schema_version`, `lane_id`, `role`, `state`, `ledger_ref.seq`, `ledger_ref.hash`,
  `actor`, `ts_wall`) — no schema library needed, a field-presence + type assertion is sufficient
  since this is a closed, small object.
- `file_sink_writes_one_ndjson_line_per_event_in_a_tempdir` — deliver 20 events, read the file back
  line-by-line, assert 20 lines, each `serde_json::from_str::<Value>` succeeds, and the file lives
  under `tempfile::tempdir()` — never the repo tree.

**Mutation-testing targets** (`cargo mutants -p fleet-stream`):
- Flipping the strict `>` in `poll_since`'s "returned seq must be `> after`" check to `>=` must be
  killed by a dedicated `log_source_rejects_seq_equal_to_after` test (a fake source that returns the
  boundary seq itself must trigger `OutOfOrder`).
- Deleting the `queue_capacity` truncation in the poll cycle (processing everything in one shot
  instead of bounding it) must be killed by `pump_config_queue_capacity_bounds_but_never_drops`
  asserting the *per-cycle* count, not just the eventual total.
- Swapping `Transient`→`Permanent` handling (so a `Transient` failure never gets retried) must be
  killed by `run_sink_transient_failure_retries_then_advances`'s explicit `retried >= max_retries`
  assertion.
- Removing the "advance cursor after `Permanent`" step (so a poison event stalls the sink forever)
  must be killed by `run_sink_permanent_failure_does_not_stall_later_events`.
- Widening `OrbSink::accepts` to `true` unconditionally must be killed by
  `orb_sink_accepts_only_lane_status`'s full 7-variant table.

**Property tests** (`proptest`, recommended given the ordering/at-least-once guarantees):
- *Every receipt is eventually delivered exactly once per sink, regardless of `Transient` failure
  injection*: generate a random sequence of receipts and a random schedule of `Transient` failures
  (bounded, so eventual success is guaranteed) for a fake sink; assert the final delivered sequence
  (by seq) is exactly the accepted subsequence, in order, with no gaps and no duplicates — the
  general form of the four ordering/retry integration tests above, at N ≥ 100 cases.

## 10. Verification recipe

```bash
cd crates/fleet-stream
cargo test -p fleet-stream --all-targets
cargo clippy -p fleet-stream --all-targets -- -D warnings
cargo mutants -p fleet-stream
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration + property tests pass, 0 skipped — publish as `<passed>/<total>`
(e.g. `24/24`, never just "tests pass"). Clippy: 0 warnings. Mutants: every target named in §9
caught; publish `<caught>/<total mutants>` — any survivor gets a new test, not a lowered floor.
`DashboardSink`'s axum server binds `127.0.0.1:0` in tests (never a fixed port) so the test suite
never collides with a real run or another test process.

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`LogSourceError`, `CursorError`, `SinkError`)
      — none swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code.
- [ ] Clock/RNG/IO are injected: `LogSource`/`CursorStore` are traits the caller supplies; the one
      ambient-looking call (`tokio::time::sleep` in the poll-cycle backoff) is isolated to `pump`'s
      scheduling loop, never inside a `Sink`'s `accepts`/business logic.
- [ ] Thread-safety documented: `LogSource`/`CursorStore` are `Send + Sync` (safe to call from
      multiple concurrent sink-worker tasks via `&self`); `Sink` is `Send` but not required `Sync`
      (each sink is owned by exactly one `run_sink` task, never shared across tasks) — stated here so
      a future change doesn't accidentally assume a `Sink` can be called from two tasks at once.
- [ ] No float used for money or token counts — `MeterSample`'s `labelled_cost_cents`/
      `estimated_tokens` are `u64`; only a measurement rate (none exists yet) would ever be float.
- [ ] No self-grading: verification runs `cargo mutants`, not just this crate's own unit tests;
      denominator published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10, template) — restate the
      real numbers in the PR once the crate is built (mark done then).
- [ ] Tests that touch the filesystem (`FileSink`'s tests) write only under `tempfile::tempdir()`,
      never the repo tree or `$HOME`.
- [ ] Every non-goal in §2 is actually absent from the code: no `Command::new` anywhere except
      `sinks/otel.rs`; no direct `ledger/chain.jsonl` path string anywhere in `src/` (grep-checkable:
      `grep -rn 'ledger/chain' crates/fleet-stream/src/` must return nothing — this crate only knows
      `LogSource`, never the ledger's file path).
- [ ] No source file exceeds 80 lines — §8 splits every sink and the pump internals into small files;
      verified by the §10 `wc -l ... awk '$1>80'` gate before Opus review.

## 12. Definition of Done

`fleet-stream` is DONE when: §10's four commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught, file-size gate silent) run from `crates/fleet-stream/`;
every unchecked box in §11 is checked with its real numbers; `registry/services/REGISTRY.md` lists
the crate (infrastructure/egress plane, per C1/L2 — `services/`, matching `fleet-router`'s
placement); and Opus has re-derived the at-least-once/bounded-queue/independent-sink-progress
argument from this blueprint alone, reproduced the "queue truncation doesn't lose data because the
next cycle re-derives from the durable log" mutation by hand, and driven one real `OrbSink` delivery
end-to-end against a test HTTP listener confirming the body matches `lane-status.v1.json`.

---

## Divergence from MIGRATION-PLAN (for Opus)

1. **`fleet-store` has no blueprint yet, so `LogSource`/`CursorStore` are this crate's proposal for
   that seam, not a confirmed contract.** MIGRATION-PLAN's row 12 says fleet-stream "reads log via
   fleet-store" but row 2 (`fleet-store`) is still `todo` in §5's status table — there is no
   `blueprints/fleet-store/BLUEPRINT.md` to check this crate's `LogSource` trait against. This
   blueprint chose a `&self`-taking, seq-addressed `poll_since(after) -> Vec<Receipt>` query shape
   (rather than mirroring `console.rs`'s mutable byte-offset `LedgerTail` verbatim) because it
   assumes `fleet-store` will be a real store (redb/rusqlite per MIGRATION-PLAN row 2's "add redb;
   unify"), not a raw file this crate re-tails. **Whoever writes `fleet-store`'s blueprint should
   either adopt this exact trait or, if it diverges, this blueprint's `log_source.rs` needs a
   matching edit** — flagged here rather than silently assumed compatible.

2. **`fleet-types` does not define a `LaneStatus` wire type**, only `Receipt`/`Attestation` (per its
   own blueprint's §3). `OrbSink`'s outbound projection (§5's `lane-status.v1.json` mirror) is
   therefore built ad hoc inside `sinks/orb.rs` from `Receipt.body`'s fields, the same way
   `console.rs`'s `load_lane_status` does today, rather than being a shared typed struct another
   crate could reuse. If a second consumer of the lane-status shape appears (e.g. a future
   `fleet-govern` dashboard), that struct should move to `fleet-types` — noted here so it isn't
   silently duplicated a third time.

3. **`OtelSink` depends on a Python subprocess (`telemetry_otel.py`) rather than a pure-Rust OTel
   client.** The brief's dependency list (axum, tokio-tungstenite, reqwest) does not include an OTel
   crate, and `fleet/keel/Cargo.lock` has none resolved either — confirming this is deliberate, not
   an oversight, but worth stating plainly: this sink's reliability is bounded by Python/venv
   availability in whatever environment runs `fleet-stream`, a different failure surface than the
   crate's other five (pure-Rust) sinks. If that becomes a real operational problem, the fix is a
   `fleet-otel` crate wrapping `opentelemetry-rust` directly — out of scope for this blueprint.
