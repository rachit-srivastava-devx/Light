# BLUEPRINT — `notify`

## 1. Identity and LLD path
- Node id / label / tag: `notify / Notification / deterministic`
- LLD authority: `docs/LLD/LLD.md §16, §20, §21` and `docs/LLD/lld-full-detail.architecture.json:components[id=notify]`
- Why this node exists: project meaningful redacted state changes through authorized notification transports.
- Incoming edges: `approval -> notify` (`state change`); control/rollback state changes.
- Outgoing edges: terminal/outbox transport acknowledgement and receipt; no workflow authority.
- Build status: `partial` — `crates/fleet-stream/src/sink.rs:6-27`, cursor/pump modules are delivery precedents; no complete durable notification outbox exists.

## 2. Responsibility and non-goals
**Owns:** redaction, notification outbox projection, transport capability result, lease/retry scheduling, and delivery receipt.

**Does not own:** deciding workflow state, granting effects, provider credentials, or treating popup delivery as integration proof.

## 3. Boundary and authority
Notification is an effect. A grant must cover the transport, recipient/resource, and redacted payload hash. Durable outbox insertion is atomic with the state transition; actual transport is parent/broker-mediated. `terminal-jsonl` is the deterministic baseline. Desktop/provider denial is recorded as unsupported/failed, not silently successful.

## 4. Crate/package layout
```text
crates/notify/
  Cargo.toml
  src/lib.rs                 # exports, ≤40 lines
  src/outbox.rs              # effect record/idempotency, ≤80 lines
  src/redact.rs              # payload policy, ≤80 lines
  src/transport.rs           # transport port, ≤70 lines
  src/reconcile.rs           # ack/unknown classification, ≤80 lines
  tests/notify.rs            # sink/retry/redaction cases, ≤80 lines
```

## 5. Public API contract
```rust
pub struct Notification { pub task_id: String, pub transport: String, pub recipient: String, pub payload: String, pub idempotency_key: String, pub grant: Grant }
pub struct DeliveryReceipt { pub effect_id: String, pub status: DeliveryStatus, pub attempts: u64, pub checked: u64, pub total: u64 }
pub trait Transport { fn send(&self, n: &Notification) -> Result<TransportAck, NotifyError>; }
pub fn enqueue(store: &mut impl OutboxStore, n: Notification) -> Result<String, NotifyError>;
```
Payload is redacted before hashing/storage. Ack is not inferred from local send return unless the transport contract says so; timeout is `Unknown`.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| outbox effect | unique deterministic idempotency key and payload hash | duplicate notification | `Conflict/8` |
| grant | transport/recipient/scope/expiry match | unauthorized egress | `Grant/7` |
| delivery receipt | ack only after transport acknowledgement; counts nonzero for reconciliation | false delivery | `Unknown/8` |
| redacted payload | no secret/credential/source body outside scope | leakage | `Redaction/6` |

Outbox is single-writer durable state; retry attempts are integers; no provider version is assumed.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-stream/src/sink.rs:6-27` | exact local source | sink trait/error shape | preserve cursor/ack boundary | outbox integration |
| `crates/fleet-stream/src/cursor_file.rs` | local source | durable cursor precedent | avoid per-transport cursor invention | restart/resume test |
| `notify-rust 4.18.0` | research primary source; local dependency/adoption unverified | optional Linux adapter | maintained desktop protocol wrapper | capability/ack smoke |
| Apple `UNUserNotificationCenter` | Apple primary docs; no CLI proof | future app-bundle adapter | official permission surface | signed app authorization test |
| Tokio bounded channels | research records local `1.53.1`; exact lock verify at implementation | wake-up only | no durable queue duplication | backpressure smoke |

**Note:** Notification providers (Slack, email, desktop) are NOT direct crate dependencies. All providers implement the generic `Transport` trait defined in `src/transport.rs`. Provider crates are optional, feature-gated, and never required for the core crate to compile or test.

**Exact `Cargo.toml` `[dependencies]` block for `crates/notify/Cargo.toml`:**
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Current macOS run has no real desktop acknowledgement; this is a blocker to claiming desktop delivery.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
# notify-rust = "4.18.0"  — optional, feature-gated; see §7 table
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix
### `enqueue` / `deliver`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refuse missing grant/recipient/payload; receipt records refusal |
| wrong type / Unicode | typed payload error; Unicode allowed after redaction |
| huge / negative | payload/attempt bounds; negative delay refuses |
| duplicate / concurrent | same key returns one effect; lease prevents concurrent delivery |
| partial failure / timeout | `Unknown` plus receipt; retry only after readback/idempotency rule |
| stale / unavailable | expired grant or missing session bus is unsupported/refusal, not success |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `Notification`, `DeliveryReceipt`, `DeliveryStatus`, `Grant`, `Transport`, `NotifyError`, and `OutboxStore`; re-export all public items; `cargo check -p notify` exits 0.
2. In `src/redact.rs`: Implement `redact_pii` and deterministic idempotency-key derivation; `cargo test -p notify state_change_emits_redacted_notification` exits 0.
3. In `src/outbox.rs`: Add atomic outbox-store insertion (durable intent before transport send); `cargo test -p notify sensitive_fields_not_in_notification` exits 0.
4. In `src/reconcile.rs`: Add bounded lease/backoff/reconciliation with `Unknown` on transport error; `cargo test -p notify failed_delivery_does_not_panic` exits 0.
5. In `tests/notify.rs`: Wire all three named integration tests end-to-end; `cargo test -p notify --no-fail-fast` exits 0 with `checked=1,total=1` in terminal-jsonl fixture output.

## 10. Test matrix
**Unit tests:** redaction, key determinism, grant scope, duplicate, unknown timeout, backoff bounds.

**Integration/contract tests:**

### Named integration tests (these 4 must compile and pass)

| Test name | Inputs | Expected output | Mutation caught |
|---|---|---|---|
| `notify::tests::state_change_emits_redacted_notification` | `StateChangeEvent` with a task state transition and a non-empty payload | A `Notification` is emitted to the outbox; payload field contains no raw secret/credential text; `DeliveryReceipt.checked > 0`; `DeliveryReceipt.total > 0` | Any stub that passes the event through without redaction fails; this is the LLD edge contract test (`approval → notify`) |
| `notify::tests::sensitive_fields_not_in_notification` | `StateChangeEvent` with `user_email = "alice@example.com"` in its data | The emitted `Notification.payload` does NOT contain the string `"alice@example.com"` (assert with `!payload.contains("alice@example.com")`) | Any stub that copies the event payload verbatim fails — a pass-through implementation cannot satisfy this assertion |
| `notify::tests::duplicate_delivery_refused` | Same `Notification` with identical idempotency key sent twice to `enqueue` | First call succeeds; second call returns `Conflict/8` error (duplicate idempotency key rejected); outbox record count is 1, not 2 | Any impl that omits the idempotency key check passes both calls silently, creating duplicate records |
| `notify::tests::zero_denominator_vacuous_proof_refused` | `NotifyInput` where `delivery_attempts = 0` and `total_recipients = 0`; invoke `build_delivery_receipt` | `Err(NotifyError::ZeroDenominator)` returned; no receipt written | Mutating `build_delivery_receipt` to return `checked=0, total=0` vacuously yields a pass, failing `assert!(result.is_err())` |

**Hidden tests:** secret in payload, expired grant, provider timeout, lease theft, duplicate after restart, zero-row reconciliation.

**Property tests:** same logical notification always yields same key; 1,000 generated notifications.

**Differential tests:** stream sink cursor behavior versus notify outbox cursor; only durable effect fields are added.

**Real-binary/effect test:** `target/debug/fleet notify --transport terminal-jsonl`; desktop/provider delivery remains unverified unless an OS/provider ack is captured.

## 11. Mutation targets and anti-stub proof
| Function mutated | Mutant or fake implementation | Test that must fail | Why this proves behavior |
|---|---|---|---|
| `redact_pii` in `src/redact.rs` | Remove call to `redact_pii`; pass raw payload directly to outbox | `notify::tests::state_change_emits_redacted_notification` and `notify::tests::sensitive_fields_not_in_notification` | Notification cannot contain raw PII; a stub that copies the event verbatim fails |
| `emit_notification` in `src/outbox.rs` | Send to transport before committing to outbox (swap order of outbox write and transport send) | crash-before-send test | Durable intent must precede the side-effect; crash between write and send must be recoverable |
| `handle_delivery_failure` in `src/reconcile.rs` | Treat transport error as success (`DeliveryStatus::Delivered`) or call `unwrap()` | `notify::tests::failed_delivery_does_not_panic` | Transport failure must produce `Unknown`/logged error; not panic and not fabricated success |
| `enqueue` in `src/outbox.rs` | Omit idempotency key from outbox record | `notify::tests::duplicate_delivery_refused` | Retries cannot create duplicate notifications silently |
| `build_delivery_receipt` in `src/reconcile.rs` | Return `checked=0, total=0` vacuously | `notify::tests::zero_denominator_vacuous_proof_refused` | No vacuous delivery proof; nonzero denominator required |

Reviewer manually changes `handle_delivery_failure` to return `Delivered` on error; `notify::tests::failed_delivery_does_not_panic` must fail.

## 12. Verification recipe and denominators
```bash
cargo test -p notify --no-fail-fast
cargo clippy -p notify --all-targets -- -D warnings
find crates/notify -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
target/debug/fleet notify --transport terminal-jsonl --fixture tests/fixtures/blueprint-notify/meaningful-change.json
```
Expected: unit cases `checked=10,total=10`; property keys `checked=1000,total=1000`; terminal delivery `checked=1,total=1`; desktop/provider status is explicitly unverified.

## 13. Definition of done
All of the following must be true — each is checkable by inspection or command output, no subjective criteria:

- `cargo test -p notify --no-fail-fast` exits 0 with `test result: ok` in output.
- `notify::tests::state_change_emits_redacted_notification` appears in test output and passes.
- `notify::tests::sensitive_fields_not_in_notification` appears in test output and passes.
- `notify::tests::failed_delivery_does_not_panic` appears in test output and passes.
- `notify::tests::zero_denominator_vacuous_proof_refused` appears in test output and passes.
- `cargo clippy -p notify --all-targets -- -D warnings` exits 0 (zero warnings).
- `target/debug/fleet notify --transport terminal-jsonl --fixture tests/fixtures/blueprint-notify/meaningful-change.json` output contains `checked=1,total=1`.
- No source file under `crates/notify/src/` exceeds 80 lines (check with `wc -l`).
- Mutation floor: manually removing the `redact_pii` call causes at least one named test to fail (caught/total ≥ 80%).
- `Cargo.toml` pins `serde = { version = "1", ... }`, `thiserror = "2"`, `tokio = { version = "1", ... }` exactly as shown in §7.
- No provider crate (Slack SDK, email client, `notify-rust`) appears in `[dependencies]` of `crates/notify/Cargo.toml`; providers are feature-gated or in a separate adapter crate.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a timeout caused duplicate user messages → send status was treated as failed without readback → retain `Unknown`, reconcile with the same key, then retry only when safe.

1. What is the durable authority? 2. What does timeout mean? 3. Is desktop ack actually observed?
