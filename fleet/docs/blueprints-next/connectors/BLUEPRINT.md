# BLUEPRINT — connectors

## 1. Identity and LLD path

- Node id / label / tag: `connectors / GitHub / Gmail / deterministic`
- LLD authority: `docs/LLD/LLD.md §16`, `docs/LLD/LLD-META-L8-ADDENDUM.md §F, §H`; `docs/LLD/lld-full-detail.architecture.json:components[id=connectors]`
- Why this node exists: Pull authorized external provider observations and hand untrusted native payloads to `ingest`.
- Incoming edges: operator configuration and provider responses.
- Outgoing edges: `connectors -> ingest` authorized event.
- Build status: `partial`
- Exact workflow path: `user_cli/connectors -> ingest -> store/control -> intent -> route -> scan`.

## 2. Responsibility and non-goals

**Owns:** provider transport, auth-token injection through a parent-owned port, pagination/cursor handling, retry-after interpretation, and native-to-envelope mapping.

**Does not own:** event IDs, deduplication, durable cursors, workflow state, policy, grants, or outbound effects. Those belong to `ingest`, `store`, `control`, and `broker`.

## 3. Boundary and authority

Provider payloads and actor fields are untrusted. A connector may read only credentials supplied by an authorized parent and may not persist them or write Fleet state. GitHub adoption is installation-scoped App auth; Gmail adoption is OAuth with narrow scopes. The current source contradicts this target: `crates/fleet-events/src/adapters/github.rs:22-76` uses a bearer token and REST events, while `gmail.rs:24-80` uses IMAP username/password. Treat both as extraction fixtures, not compliant runtime behavior.

## 4. Crate/package layout

```text
crates/connectors/
  Cargo.toml                 # provider dependencies and feature flags
  src/lib.rs                 # adapter traits and exports, 40 lines
  src/github.rs              # Octocrab transport and mapping, 80 lines
  src/gmail.rs               # io-gmail transport and mapping, 80 lines
  src/auth.rs                # credential/token ports, 65 lines
  tests/mock_provider.rs     # scripted HTTP fault cases, 80 lines
```

No connector calls `store`; it returns `NativeEvent` to `ingest`.

## 5. Public API contract

```rust
pub trait CredentialPort { fn token(&self, provider: Provider) -> Result<SecretRef, ConnectorError>; }
pub trait ProviderClient { fn poll(&mut self, cursor: Option<&str>) -> Result<PollPage, ConnectorError>; }
pub struct ConnectorIdentity { pub source_namespace: String, pub provider: String, pub principal_ref: String, pub credential_ref: String, pub scopes: Vec<String>, pub config_digest: String }
pub struct NativeEventMapping { pub source_namespace: String, pub native_kind: String, pub delivery_id: String, pub object_version: String, pub external_actor: String, pub payload_digest: String }
pub struct ConnectorEnvelope { pub source: String, pub delivery_id: String, pub object_version: String, pub schema_version: u64, pub actor: String, pub payload_ref: String, pub payload_digest: String, pub auth_metadata_ref: String }
pub struct NativeEvent { pub source: String, pub delivery_id: String, pub object_version: String, pub actor: String, pub payload: serde_json::Value }
pub struct PollPage { pub events: Vec<NativeEvent>, pub next_cursor: Option<String>, pub retry_after_seconds: Option<u64> }
pub fn pull_once<C: ProviderClient>(client: &mut C, cursor: Option<&str>) -> Result<PollPage, ConnectorError>;
```

The connector never returns a credential value in `NativeEvent`. Unknown provider completion is `ConnectorError::UnknownCompletion`, not success. `NativeEvent` is an internal provider observation; `poll` must map it to the canonical `ConnectorEnvelope` before returning the `connectors -> ingest` payload.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `NativeEvent` | nonempty source, delivery ID, object version, actor | ambiguous dedup and audit | 7 |
| `PollPage` | cursor advances only from provider response | skipped page or replay loop | 8 |
| credential | opaque borrowed secret, never serialized | credential leakage | 6 |
| retry signal | bounded provider delay, no infinite retry | hot loop/rate-limit abuse | 3 |

At-least-once delivery is expected. Concurrent pulls for one provider are rejected by an adapter lease; separate providers may run concurrently. All counts are integers.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-events/src/adapters/github.rs:47-76` | local LV, current REST/ETag mapping | fixtures and compatibility cases | preserve known event mapping tests | replace bearer auth and prove App auth |
| `crates/fleet-events/src/adapters/gmail.rs:46-80` | local LV, IMAP mapping | parsing fixtures only | do not retain password IMAP | OAuth/history proof |
| `octocrab 0.54.1` | MIT OR Apache-2.0; crates.io/repo links in `_research/foundation-control.md` | GitHub typed transport | maintained pagination/retry | exact checksum probe and approved live grant |
| `io-gmail 0.3.0` | MIT OR Apache-2.0; crates.io/repo links in research | Gmail REST/OAuth transport | avoids custom OAuth/HTTP | exact pin compile and test mailbox |

The exact Octocrab upstream commit is unresolved in the research record; adoption remains an external proof gate.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
# octocrab: pin version from §7 table
octocrab = "0.54.1"
# io-gmail: pin version from §7 table
io-gmail = "0.3.0"
```

## 8. Behavior matrix

### `pull_once`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | missing cursor means provider baseline; empty page returns `events=0` and is not an ingest success until the gate reports checked input |
| wrong type / Unicode | malformed provider JSON is typed protocol refusal; Unicode actor/body is preserved |
| huge / negative | cap page at 1000 events and 256 KiB payload; negative retry-after refuses |
| duplicate / concurrent | duplicate delivery IDs are returned for ingest dedup; same-provider concurrent pull refuses |
| partial failure / timeout | return cursor unchanged and retry metadata; never advance cursor locally |
| stale / unavailable | expired cursor returns `CursorExpired`; auth 401 returns `AuthRequired`; 429 honors provider delay |

## 9. Tiny implementation steps

1. In `src/lib.rs`: Define `NativeEvent`, `PollPage`, `CredentialPort`, `ProviderClient`, and `ConnectorError` types; re-export all public items; `cargo check -p connectors` exits 0.
2. In `src/github.rs`: Implement scripted GitHub page parser for pagination, 429 retry-after, malformed event, and ETag; `cargo test -p connectors github_pull_returns_nonempty_page` exits 0.
3. In `src/gmail.rs`: Implement scripted Gmail history parser for cursor expiry, edited/deleted message, and OAuth refusal; `cargo test -p connectors gmail_cursor_expiry_returns_cursor_expired` exits 0.
4. In `src/auth.rs`: Add `CredentialPort` implementation keeping the secret opaque; `cargo test -p connectors credential_never_appears_in_native_event` exits 0.
5. In `tests/mock_provider.rs`: Wire all three named integration tests end-to-end using scripted HTTP faults; `cargo test -p connectors --no-fail-fast` exits 0.

## 10. Test matrix

**Unit tests:** native mapping, cursor monotonicity, retry-after bounds, auth scope validation.

**Integration/contract tests:** `connector_output_is_accepted_by_ingest` — one GitHub and one Gmail envelope through the real `ingest` trait.

**Hidden tests:** same delivery ID with different payload digest, provider page gap, own-bot event, and credential appearing in serialized output.

**Property tests:** 1,000 generated native events map to nonempty IDs/version fields or typed refusal; fixed seed recorded.

**Differential tests:** old GitHub/Gmail fixture mappings versus new adapters; allowed divergence is only source/auth metadata and explicit unsupported event kinds.

**Real-binary/effect test:** `fleet ingest --once` with a local scripted provider process; live provider test is blocked until approved credentials and endpoint metadata exist.

### Named integration tests (these three must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `connectors::tests::github_pull_returns_nonempty_page` | `MockProviderClient` returning a scripted page of 2 `NativeEvent` records | `PollPage` with `events.len() == 2`; `next_cursor` is `Some`; `retry_after_seconds` is `None` | `pull_once` returning an empty page unconditionally fails — proves the event-count path is exercised |
| `connectors::tests::credential_never_appears_in_native_event` | `MockCredentialPort` returning `SecretRef("supersecret")`; `NativeEvent` produced via the mock client | `NativeEvent.payload` serialized as JSON does not contain the string `"supersecret"` | Any stub serializing the credential into the payload fails; proves the opaque-secret boundary is enforced |
| `connectors::tests::gmail_cursor_expiry_returns_cursor_expired` | `MockProviderClient` configured to return an expired-cursor error on the first poll | `ConnectorError::CursorExpired` is returned; cursor value is NOT advanced | Any stub that advances the cursor on error fails — proves cursor monotonicity; catches the branch that confuses expiry with an empty page |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `pull_once` in `src/github.rs` | Always return `PollPage { events: vec![], next_cursor: None, retry_after_seconds: None }` | `connectors::tests::github_pull_returns_nonempty_page` | Test asserts `events.len() == 2`; an empty-page constant fails immediately; proves the provider page is actually read |
| `map_to_native_event` in `src/github.rs` | Include the raw credential string in `NativeEvent.payload` | `connectors::tests::credential_never_appears_in_native_event` | Test serializes the event and checks for the secret substring; catching credential leakage is the primary boundary proof |
| `advance_cursor` in `src/gmail.rs` | Advance cursor even when the provider returns an expiry error | `connectors::tests::gmail_cursor_expiry_returns_cursor_expired` | Test asserts the cursor is not advanced on error; cursor monotonicity is the replay-safety guarantee |
| `enforce_retry_after` in `src/github.rs` | Return `retry_after_seconds: None` for every 429 response | 429 timing assertion (hidden) | Rate-limit signals must be observable and honored; a constant-None stub hides provider back-pressure |

Safety mutation floor: `caught/total >= 75%`; reviewer manually deletes the Gmail cursor-expiry branch and confirms the named test goes red.

## 12. Verification recipe and denominators

```bash
cargo test -p connectors --no-fail-fast
cargo clippy -p connectors --all-targets -- -D warnings
find crates/connectors -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-events --test adapters_cli --no-fail-fast
cargo test --test connector_ingest_contract --no-fail-fast
cargo run --bin fleet -- ingest --once --provider scripted
# Mutation floor: caught/total >= 75%
```

Expected evidence: provider fixtures `checked=4,total=4`; generated events `checked=1000,total=1000`; mock connector pages `checked=2,total=2`; live GitHub/Gmail remains an explicitly named external proof gate, not a local pass.

## 13. Definition of done

- `cargo test -p connectors --no-fail-fast` exits 0 with `test result: ok`.
- `connectors::tests::github_pull_returns_nonempty_page` appears in test output and passes.
- `connectors::tests::credential_never_appears_in_native_event` appears in test output and passes.
- `connectors::tests::gmail_cursor_expiry_returns_cursor_expired` appears in test output and passes.
- `cargo clippy -p connectors --all-targets -- -D warnings` exits 0.
- `find crates/connectors -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- Mutation floor: `caught/total >= 75%` for the 4 named mutation targets in §11.
- `grep -rn "bearer\|imap_password" crates/connectors/src/` exits 1 (zero hits — no legacy auth path remains).
- Live provider gate: an external authorized receipt with `checked>0` is produced and recorded before claiming GitHub/Gmail provider compliance.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** Gmail replayed messages after a crash → cursor advanced before ingest commit → return cursor as a candidate and persist it only after store commit.

1. Which auth is actually proven? 2. What prevents page loss? 3. Can an empty stub pass the connector gate?
