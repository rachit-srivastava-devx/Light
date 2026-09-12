# BLUEPRINT — `broker`

## 1. Identity and LLD path
- Node id / label / tag: `broker / External Broker / deterministic`
- LLD authority: `docs/LLD/LLD.md §12, §13, §16, §21`, `docs/LLD/LLD-META-L8-ADDENDUM.md §B, §H`, and `docs/LLD/lld-full-detail.architecture.json:components[id=broker]`
- Why this node exists: provide the only parent-authorized path from an approved local proposal to GitHub/push or another external effect.
- Incoming edges: `approval -> broker` (`exact PR/publication proposal and consumed grant`).
- Outgoing edges: external provider acknowledgement/reconciliation; `broker -> notify` is a transport projection, not a workflow shortcut.
- Build status: `greenfield` — no complete external-effect broker exists; `crates/fleet-stream` and lifecycle receipt traits are precedents only.

## 2. Responsibility and non-goals
**Owns:** capability validation, PREPARED/DISPATCHING/ACK/UNKNOWN effect state, provider adapter invocation, idempotency, redaction, and reconciliation.

**Does not own:** deciding approval, changing repository content, inventing provider auth, retrying unknown effects blindly, or claiming a remote action from local intent.

## 3. Boundary and authority
The broker is parent-owned and the only external-effect boundary. It receives a consumed, exact approval grant and a proposal digest; workers cannot access credentials, broker sockets, remote refs, or provider APIs. The durable store commits `PREPARED` and idempotency key before the call, then `DISPATCHING` before network I/O. A crash after dispatch is `UNKNOWN`; only provider readback or a documented idempotency guarantee may resolve it.

## 4. Crate/package layout
```text
crates/broker/
  Cargo.toml
  src/lib.rs                 # exports, ≤40 lines
  src/effect.rs              # effect state/idempotency, ≤80 lines
  src/authorize.rs           # grant/resource checks, ≤80 lines
  src/provider.rs            # provider port and sanitized ack, ≤80 lines
  src/reconcile.rs           # readback state machine, ≤80 lines
  tests/broker.rs            # scripted provider faults, ≤80 lines
```

## 5. Public API contract
```rust
pub struct ExternalEffect { pub effect_id: String, pub action: String, pub resource: String, pub payload_hash: String, pub idempotency_key: String, pub grant: Grant }
pub struct PublicationGrant { pub approval_id: String, pub changeset_id: Option<String>, pub action: String, pub destinations: Vec<String>, pub content_digest: String, pub expires_at: String }
pub struct CapabilityBundle { pub lease_id: String, pub bundle_digest: String, pub manifest_digest: String, pub tool_list_digest: String, pub capabilities: Vec<ToolCapability>, pub credential_handles: Vec<CredentialHandle>, pub expires_at: String, pub revocation_epoch: u64 }
pub struct ToolCapability { pub server_id: String, pub tool_name: String, pub schema_digest: String, pub resource_scope: String, pub effect_class: String }
pub struct CredentialHandle { pub handle_id: String, pub audience: String, pub lease_id: String, pub expires_at: String, pub revocation_epoch: u64 }
pub struct ToolRequest { pub lease_id: String, pub bundle_digest: String, pub sequence: u64, pub server_id: String, pub tool_name: String, pub args_digest: String }
pub struct ToolResult { pub request_digest: String, pub sequence: u64, pub status: String, pub result_ref: Option<String>, pub provider_metadata_ref: Option<String> }
pub enum EffectState { Prepared, Dispatching, Acked, NotSeen, Failed, Unknown, Conflict, Expired }
pub trait Provider { fn dispatch(&self, e: &ExternalEffect) -> Result<ProviderAck, BrokerError>; fn readback(&self, key: &str) -> Result<Readback, BrokerError>; }
pub fn execute(p: &impl Provider, store: &mut impl EffectStore, e: ExternalEffect) -> Result<EffectState, BrokerError>;
```
Preconditions: exact grant/action/resource/payload, unique key, expiry valid, capability bundle,
and store available. Postcondition: every outcome has a receipt; `Acked` requires provider
acknowledgement; no panic. Tool requests are accepted only over the worker's inherited fd 3 and
never expose credential values.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| external effect | key unique from task/effect/scope; payload hash exact | duplicate publication | `Conflict/8` |
| capability grant | action/resource/digest/expiry match | confused deputy | `Grant/7` |
| effect state | durable transition before corresponding side effect | lost intent | `Store/3` |
| reconciliation verdict | `checked==total>0` and one of six outcomes | vacuous recovery | `Reconcile/6` |

SQLite is the durable authority; a bounded Tokio channel may wake an adapter only. Credentials are injected through a parent-owned provider port and never serialized in effects. Retry counts and deadlines are integers.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-lifecycle/src/receipt.rs:11-22` | exact local receipt port | durable receipt seam | one audit authority | effect-store integration |
| `crates/fleet-stream/src/sink.rs:6-27` and cursor modules | exact local delivery precedent | ack-after-delivery/restart cursor shape | avoid per-provider ledger | crash/replay test |
| SQLite WAL / `rusqlite` | research: local resolves `rusqlite 0.32.1`; newer `0.40.2` is a research candidate, not adopted | atomic domain+outbox transaction | single-host durable authority | file-backed WAL smoke |
| Tokio `1.53.1` | research/local lock evidence; verify lock at implementation | bounded wake-up channel | not a durable queue | backpressure test |
| GitHub/provider SDK | no provider implementation or capability proof in this checkout | future adapter behind port | prevents credentials/provider logic in kernel | approved live grant, endpoint metadata, readback |

The provider protocol/version, auth flow, scopes, and remote idempotency guarantee are unverified and are explicit implementation gates.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix
### `execute` / `reconcile`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refuse missing grant/effect/payload and write receipt |
| wrong type / Unicode | typed schema refusal; Unicode payload is redacted/hashed, not interpreted |
| huge / negative | bounded payload/output/attempts; negative expiry or budget refuses |
| duplicate / concurrent | unique key returns existing state; live lease cannot be stolen; expired lease is reclaimable with receipt |
| partial failure / timeout | after dispatch timeout state is `Unknown`; no blind retry; provider readback required |
| stale / unavailable | expired grant, changed payload, provider unavailable, or auth failure is typed refusal/unknown, never success |

## 9. Tiny implementation steps
1. In `src/lib.rs` and `src/effect.rs`: define `ExternalEffect`, `EffectState`, `ProviderAck`, and `BrokerError` types → `cargo check -p broker` exits 0.
2. In `src/authorize.rs`: implement deterministic idempotency key and grant/resource authorization predicate → add `broker::tests::approval_consumption_produces_effect` and run `cargo test -p broker approval_consumption_produces_effect` exits 0.
3. In `src/effect.rs`: add atomic `PREPARED`/`DISPATCHING` state persistence before dispatch — crash after dispatch yields `Unknown` → add `broker::tests::crash_after_dispatch_yields_unknown` and run `cargo test -p broker crash_after_dispatch_yields_unknown` exits 0.
4. In `src/provider.rs`: add bounded `Provider` port and sanitized acknowledgement with no credential in ack → add `broker::tests::provider_ack_advances_receipt` and run `cargo test -p broker provider_ack_advances_receipt` exits 0.
5. In `src/reconcile.rs` and `tests/broker.rs`: add readback reconciler and scripted-provider fixture → `target/debug/fleet broker --provider scripted --fixture tests/fixtures/blueprint-broker/scripted-success.json` output contains `checked=1,total=1`; live provider remains gated.

## 10. Test matrix
**Unit tests:** key determinism, grant mismatch, state transitions, payload redaction, lease expiry.

**Integration/contract tests:** approval consumption produces one broker effect; provider ack advances receipt exactly once.

**Hidden tests:** crash after dispatch, duplicate retry, changed payload with same key, credential in provider error, live lease theft, zero-row reconciliation.

**Property tests:** same logical effect always has same key and no invalid state transition; 1,000 fixed-seed effects.

**Differential tests:** stream sink ack/cursor semantics versus broker effect state; divergence limited to external effect phases.

**Real-binary/effect test:** `target/debug/fleet broker --provider scripted` with a local scripted provider; assert PREPARED→DISPATCHING→ACKED receipts. No GitHub/push success is claimed.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `broker::tests::approval_consumption_produces_effect` | `ExternalEffect` with valid consumed `Grant`, action, resource, payload hash, and unique idempotency key; scripted `Provider` returning `ProviderAck` | `EffectState::Acked` and receipt with `checked=1,total=1` | catches grant-bypass — any stub that dispatches without validating the consumed grant fails the effect-state assertion |
| `broker::tests::provider_ack_advances_receipt` | scripted `Provider` returning `ProviderAck` on first call; `ExternalEffect` transitioning from `Preparing` | receipt advances to `Acked` exactly once; second execute with same key returns `Acked` without calling provider again | catches duplicate-ack — a provider advancing receipt on every call produces double `Acked`, breaking idempotency |
| `broker::tests::missing_approval_blocks_dispatch` | `ExternalEffect` with a nil or unconsumed `Grant` | `Err(BrokerError::Grant)` before any `Provider::dispatch` call; `Provider` mock asserts it is never called | catches grant-check removal; a stub that skips authorization calls the provider and the provider-call assertion fails |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `dispatch` in `src/effect.rs` | Commit `DISPATCHING` state after the network call instead of before | `broker::tests::approval_consumption_produces_effect` | test injects a crash between PREPARED and DISPATCHING; moving the commit after the call leaves state as PREPARED on crash, breaking the receipt assertion |
| `record_receipt` in `src/effect.rs` | Skip marking the effect consumed in the receipt — allow a second dispatch on the same key | `broker::tests::provider_ack_advances_receipt` | test calls execute twice with the same key; skipping the consumed flag lets the second call dispatch again and the idempotency assertion fails |
| `dispatch` in `src/effect.rs` | Remove grant validation — call provider regardless of grant | `broker::tests::missing_approval_blocks_dispatch` | test supplies a nil grant; removing the check causes the provider mock to be invoked, which the test asserts must not happen |
| `dispatch` in `src/effect.rs` | Treat a provider timeout as `Acked` instead of `Unknown` | `broker::tests::provider_ack_advances_receipt` | test injects a timeout; marking it `Acked` instead of `Unknown` breaks the state-machine assertion |
| `record_receipt` in `src/effect.rs` | Pass `checked=0,total=0` in the reconciliation receipt | `broker::tests::approval_consumption_produces_effect` | test asserts `checked=1,total=1`; a zero-denominator receipt fails the denominator gate |

Safety mutation floor: `caught/total >= 75%`. Reviewer manually enables the grant-bypass mutation and confirms `missing_approval_blocks_dispatch` goes red.

## 12. Verification recipe and denominators
```bash
cargo test -p broker --no-fail-fast
cargo clippy -p broker --all-targets -- -D warnings
find crates/broker -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet broker --provider scripted --fixture tests/fixtures/blueprint-broker/scripted-success.json
# Mutation floor: caught/total >= 75%
```
Expected: transition cases `checked=12,total=12`; property effects `checked=1000,total=1000`; scripted effect `checked=1,total=1`; provider/live remote proof is blocked until approved credentials, scopes, endpoint metadata, and readback evidence exist.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p broker --no-fail-fast` exits 0 with `test result: ok` in output.
- `broker::tests::approval_consumption_produces_effect` appears in test output and passes.
- `broker::tests::provider_ack_advances_receipt` appears in test output and passes.
- `cargo clippy -p broker --all-targets -- -D warnings` exits 0 (zero warnings).
- `find crates/broker -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `target/debug/fleet broker --provider scripted --fixture tests/fixtures/blueprint-broker/scripted-success.json` output contains `checked=1,total=1`.
- Mutation floor: `caught/total >= 75%` for all 5 mutation targets listed in §11, including dispatch-before-PREPARED, timeout-as-acked, and key-reuse.
- `Cargo.toml` pins `serde`, `thiserror`, and `tokio` exactly as shown in §7.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a process died after remote publication and retried, creating a duplicate → dispatch outcome was treated as failed → persist UNKNOWN and resolve by provider readback/key before retry.

1. What is the only external-effect path? 2. What proves ack? 3. Can a worker ever receive the credential?
