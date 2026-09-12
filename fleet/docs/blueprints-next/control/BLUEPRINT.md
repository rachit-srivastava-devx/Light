# BLUEPRINT — control

## 1. Identity and LLD path

- Node id / label / tag: `control / Controller / deterministic`
- LLD authority: `docs/LLD/LLD.md §5, §8, §16`, `docs/LLD/LLD-META-L8-ADDENDUM.md §A, §D, §E, §G, §H`, and `docs/LLD/lld-full-detail.architecture.json:components[id=control]`
- Why this node exists: Be the sole local authority for reducer transitions, leases, scheduling, cancellation, and post-commit dispatch.
- Incoming edges: `ingest -> control` normalized event; `intent -> control` admitted intent; `route -> control` route result; `store -> control` durable snapshot/read result.
- Outgoing edges: `control -> intent` request; `control -> store` state/reservation/outbox commit; later `control -> route` after admitted intent; durable effects through store/outbox.
- Build status: `partial`
- Exact workflow path: `user_cli/connectors -> ingest -> store/control -> intent -> route -> scan`.

## 2. Responsibility and non-goals

**Owns:** state machine admission, revisioned reducer, lease/resource conflict checks, pause/resume, bounded retry classes, and launch-after-commit.

**Does not own:** model-generated intent, provider transport, persistence implementation, policy predicates, or worker authority.

## 3. Boundary and authority

The controller is operator-approved trust zone and sole effect authority. It validates every event and IntentSpec, commits state/reservation/outbox before spawning, and supplies workers only fd 3 plus bounded context. Workers receive no ledger/state/socket path, actor, timestamp, model identity, approval, or settlement authority. Tokio is execution only; reducer decisions remain deterministic and store-backed.

## 4. Crate/package layout

```text
crates/control/
  Cargo.toml
  src/lib.rs                 # controller ports, 40 lines
  src/reducer.rs             # pure state transition, 80 lines
  src/scheduler.rs           # ready heap/conflict checks, 80 lines
  src/supervisor.rs          # Tokio child lifecycle, 80 lines
  tests/recovery.rs          # pause/crash/restart, 80 lines
```

## 5. Public API contract

```rust
pub trait AuthorityStore { fn transact(&mut self, command: Command) -> Result<Commit, ControlError>; fn snapshot(&self) -> Result<Snapshot, ControlError>; }
pub enum StoreCommand { AppendEvent { expected_revision: u64, event_ref: String }, Cas { key: String, expected_revision: u64, record_ref: String } }
pub struct DispatchRequest { pub run_id: String, pub intent_ref: String, pub snapshot_digest: String, pub requested_capabilities: Vec<String> }
pub struct ConfigSource { pub source_ref: String, pub content_digest: String, pub schema_version: u64, pub signature_ref: Option<String> }
pub struct ConfigTrustRoot { pub root_id: String, pub algorithm: String, pub public_key_ref: String, pub revision: u64, pub revoked: bool }
pub struct ConfigSnapshot { pub snapshot_digest: String, pub schema_version: u64, pub source_ref: String, pub content_digest: String, pub trust_root_id: String, pub signature_ref: Option<String>, pub policy_digest: String, pub loaded_at: String }
pub struct UsageObservation {
    pub id: String, pub attempt_id: String, pub source_namespace: String,
    pub provider_request_id: Option<String>, pub observation_key: String,
    pub input_tokens: Option<u64>, pub cached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>, pub tool_units: Option<u64>,
    pub price_microunits: Option<u64>, pub observed_at: String,
    pub confidence: String, pub reconciliation_status: String, pub digest: String,
}
pub struct CostSettlement {
    pub reservation_id: String, pub attempt_id: String, pub reserved_units: u64,
    pub settled_units: u64, pub released_units: u64, pub held_units: u64,
    pub usage_observation_id: Option<String>, pub adjustment_id: Option<String>,
}
pub struct BillingAdjustment {
    pub id: String, pub source_namespace: String, pub provider_record_id: String,
    pub delta_units: i64, pub reason: String, pub digest: String,
}
pub fn reduce(snapshot: &Snapshot, event: ControlEvent) -> Result<Transition, ControlError>;
pub async fn supervise<S: AuthorityStore>(store: S, input: impl Stream<Item=ControlEvent>) -> Result<(), ControlError>;
pub fn load_config(source: &ConfigSource, root: &ConfigTrustRoot) -> Result<ConfigSnapshot, ControlError>;
```

`reduce` is pure and implements every transition in Addendum §D. `load_config` fails closed and
returns provenance. `supervise` persists before launch and treats cancellation/unknown completion
as reconciliation, never as successful completion. `fleet-lifecycle` and `fleet-govern` are
extraction inputs, not alternate authorities.

`UsageObservation`, `CostSettlement`, and `BillingAdjustment` follow Addendum §E exactly at the
wire boundary. Missing provider usage is held, never converted to zero, and settlement preserves
`reserved = settled + released + held`.

`Command` is a local name only when it is an exact adapter for canonical `StoreCommand`; no second
store command vocabulary is permitted. `DispatchRequest` is the only control-to-intent request
shape and carries the snapshot digest and requested capability list.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| run snapshot | revision monotonic and state legal | split-brain reducer | 6/8 |
| lease | generation matches node revision and expiry | stale child effect | 6 |
| reservation | spent + outstanding + new ≤ budget | overspend | 6/7 |
| pause | admission barrier durable before checkpoint request | new work during pause | 6 |
| retry | max two same-signature repairs | infinite guessing | 7 |

Scheduler score uses integer scaled values and task-ID tie breaks. Concurrency is min of user lanes, adapter, RAM, CPU, independent nodes, and review capacity.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-lifecycle/src/task.rs:20-47` | local LV typed transition and receipt port | reducer boundary | preserves legal transition and evidence requirement | store-backed transaction |
| `crates/fleet-lifecycle/src/advance.rs:1-40` | local LV canonical advance | recovery fixture | no duplicate lifecycle graph | pause/restart proof |
| `src/pipeline/planahead/orchestrator.rs` | local seam; orchestration tests | scheduling cases | preserve existing crash cases | fd-3 real child |
| `tokio 1.53.1` | MIT; upstream docs/repo and checksum in research | async supervision/timers | maintained runtime, not authority | exact-pin cancellation probe |

No custom implementation is introduced where the listed crate already provides the facility.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix

### `reduce` / `supervise`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | no input means no run and a gate reports checked zero failure; missing revision refuses |
| wrong type / Unicode | malformed control event refuses with receipt; Unicode request remains opaque |
| huge / negative | bound queue 8 MiB per worker; reject negative budgets/revisions |
| duplicate / concurrent | event ID idempotent; conflicting revision returns CAS refusal; disjoint leases may run concurrently |
| partial failure / timeout | child is cancelled/reaped; lease becomes reconciliation; bounded repair only twice |
| stale / unavailable | stale policy/grant/model snapshot waits or refuses; no fallback to unverified authority |

## 9. Tiny implementation steps

1. In `src/lib.rs` and `src/reducer.rs`: define `Snapshot`, `ControlEvent`, `Transition`, `Commit`, and `ControlError` types → `cargo check -p control` exits 0.
2. In `src/reducer.rs`: port lifecycle legal-transition table and receipt requirement into the pure `reduce` function → add `control::tests::ingest_event_drives_state_then_intent` and run `cargo test -p control ingest_event_drives_state_then_intent` exits 0.
3. In `src/scheduler.rs`: add deterministic integer-scored ready heap and revision-CAS conflict predicate → add `control::tests::route_refusal_persists_receipt` and run `cargo test -p control route_refusal_persists_receipt` exits 0.
4. In `src/supervisor.rs`: implement `AuthorityStore::transact` before spawn and killed-controller recovery → add `control::tests::launch_before_commit_fails_recovery` and run `cargo test -p control launch_before_commit_fails_recovery` exits 0.
5. In `tests/recovery.rs`: wire Tokio cancellation and fd-3 child integration against real fleet binary → `target/debug/fleet __spawn_probe` exits 0 and output contains `checked=1,total=1`.

## 10. Test matrix

**Unit tests:** legal/illegal reducer transitions, revision CAS, resource conflict, retry cap, pause barrier.

**Integration/contract tests:** ingest event drives persisted state then intent dispatch; route refusal persists receipt and launches no child.

**Hidden tests:** controller killed after commit before spawn, child returns late generation, queue saturation, changed repo HEAD, and missing receipt capacity.

**Property tests:** 1,000 event permutations preserve illegal-transition refusal and monotonic revisions; fixed seed.

**Differential tests:** current lifecycle typed API versus new reducer for legal edge set; no silent state changes allowed.

**Real-binary/effect test:** `fleet __pipeline_probe` and `fleet __spawn_probe` with real executable and fd-3 observation; fake child cannot be sole proof.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `control::tests::ingest_event_drives_state_then_intent` | `ControlEvent` with valid task start, monotonic revision, and mock `AuthorityStore` | `Transition` with updated state committed to the store; exactly one `IntentSpec` emitted to the outbox | catches no-op reducer — any implementation that does not persist state or emit intent fails the store and outbox assertions |
| `control::tests::route_refusal_persists_receipt` | `ControlEvent` representing an illegal transition (re-starting a completed task) | `Err(ControlError)` returned; refusal receipt written to the store; no child spawned | catches missing receipt write — a stub that returns error without writing a receipt fails the store assertion |
| `control::tests::duplicate_event_idempotent` | same `ControlEvent` (same event ID) submitted twice | first call returns `Ok(Transition)`; second call returns the same `Ok` without a second state write | catches missing idempotency guard; a store applying the event twice creates a duplicate revision, breaking the monotonic-revision invariant |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `reduce` in `src/reducer.rs` | Skip the legal-transition check — allow any state-to-state transition | `control::tests::ingest_event_drives_state_then_intent` | test sends a valid event expecting a legal transition; skipping the check also accepts illegal transitions, breaking the receipt assertion on subsequent illegal events |
| `emit_intent` in `src/reducer.rs` | Suppress intent emission — return `Ok` without writing an `IntentSpec` to the outbox | `control::tests::ingest_event_drives_state_then_intent` | test asserts exactly one intent is emitted after a valid event; suppressing emission leaves the outbox empty and the assertion fails |
| `reduce` in `src/reducer.rs` | Remove event-ID idempotency key — apply the same event a second time | `control::tests::duplicate_event_idempotent` | test submits the same event twice; without the idempotency key the store receives two writes and the monotonic-revision invariant is violated |
| `reduce` in `src/reducer.rs` | Move spawn before store commit | kill-after-dispatch recovery test | effects require durable authority before launch; spawn-before-commit is caught by the recovery test |
| `reduce` in `src/reducer.rs` | Accept a stale child generation | late-child hidden test | old workers cannot mutate state; accepting a stale generation is caught by the hidden test |

Independent reviewer manually moves spawn before commit; recovery test must fail.

Safety mutation floor: `caught/total >= 80%`. Reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators

```bash
cargo test -p control --no-fail-fast
cargo clippy -p control --all-targets -- -D warnings
find crates/control -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-lifecycle --no-fail-fast
cargo test --test pipeline_resumes_after_crash --no-fail-fast
cargo run --bin fleet -- __spawn_probe --help
# Mutation floor: caught/total >= 80% (authority)
```

Expected evidence: transition cases `checked=16,total=16`; property sequences `checked=1000,total=1000`; real child frames `checked=1,total=1`; cancellation cases `checked=3,total=3`.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p control --no-fail-fast` exits 0 with `test result: ok` in output.
- `control::tests::ingest_event_drives_state_then_intent` appears in test output and passes.
- `control::tests::route_refusal_persists_receipt` appears in test output and passes.
- `cargo clippy -p control --all-targets -- -D warnings` exits 0 (zero warnings).
- `find crates/control -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `target/debug/fleet __spawn_probe` exits 0 and output contains `checked=1,total=1`.
- Mutation floor: `caught/total >= 80%` for all 5 mutation targets listed in §11, including launch-before-commit and stale-generation.
- `Cargo.toml` pins `serde`, `thiserror`, and `tokio` exactly as shown in §7.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** controller died after remote dispatch and retried blindly → completion was unknown → reconcile effect receipt before another attempt.

1. What survives process death? 2. What can a worker never receive? 3. Is scheduler choice reproducible from a snapshot?
