# BLUEPRINT — store

## 1. Identity and LLD path

- Node id / label / tag: `store / SQLite WAL Store / deterministic`
- LLD authority: `docs/LLD/LLD.md §5, §16`, `docs/LLD/LLD-META-L8-ADDENDUM.md §A, §B, §C, §D, §E, §G`, and `docs/LLD/lld-full-detail.architecture.json:components[id=store]`
- Why this node exists: Solely durably store events, state, leases, reservations, receipts, outbox rows, artifacts, and migrations.
- Incoming edges: `ingest -> store` append durable; `control -> store` read/write state.
- Outgoing edges: committed event/state/outbox references to `control`, `route`, and later nodes.
- Build status: `partial`
- Exact workflow path: `user_cli/connectors -> ingest -> store/control -> intent -> route -> scan`.

## 2. Responsibility and non-goals

**Owns:** SQLite schema/migrations, WAL transactions, CAS revisions, inbox/outbox, receipt durability, artifact references, retention and backup.

**Does not own:** workflow policy, model selection, provider calls, reducer rules, or presentation.

## 3. Boundary and authority

Only the store port writes authoritative state. All mutations require expected revision and are transactional. SQLite WAL is local durability, not replication. Large content is materialized, hashed, fsynced, and referenced; a reference to incomplete content is illegal. Disk-full, lock, corruption, migration, and fsync failures are typed environment/invariant errors and never success.

## 4. Crate/package layout

```text
crates/store/
  Cargo.toml
  src/lib.rs                 # port exports, 35 lines
  src/schema.rs              # migrations/checks, 80 lines
  src/transaction.rs         # append/CAS/outbox transaction, 80 lines
  src/artifact.rs            # atomic blob publication, 80 lines
  src/retention.rs           # measured GC, 80 lines
  tests/crash_recovery.rs    # transaction and restore tests, 80 lines
```

> **Crate status:** `store` is a NEW crate at `crates/store/`. Do not confuse with the existing `crates/fleet-store/` — that crate uses redb while this blueprint targets SQLite WAL. The §7 references to fleet-store source are migration candidates.

## 5. Public API contract

```rust
pub trait Store { fn append_event(&mut self, expected: Revision, event: Event) -> Result<Commit, StoreError>; fn load(&self, key: Key) -> Result<Option<Record>, StoreError>; fn cas(&mut self, key: Key, expected: Revision, next: Record) -> Result<Revision, StoreError>; }
pub struct Commit { pub revision: u64, pub event_id: String, pub receipt_id: String }
pub fn open(path: &std::path::Path) -> Result<Connection, StoreError>;
pub fn migrate(conn: &mut Connection) -> Result<MigrationReport, StoreError>;
```

Every mutating operation validates revision, writes event/projection/effect rows, commits, then
returns. Addendum §Required persistence requires durable `config_snapshots`,
`capability_bundles`, `tool_calls`, `changesets`, `repo_operations`, `usage_observations`, and
`run_transition_events`, each with owner, schema version, digest, revision, idempotency key where
applicable, retention rule, and migration test. `checked,total` lives in every gate record. No
`unwrap_or(0)`.

## 6. Data model, invariants, and failure taxonomy

| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `events` | unique `(run_id,seq)` and event ID; digest chain valid | replay/tamper | 6/8 |
| `nodes` | revision increments only in CAS | stale writer | 8 |
| `inbox` | unique source/delivery ID plus digest | duplicate/conflict | 6 |
| `leases` | one generation-fenced attempt per node | stale worker effect | 6 |
| `outbox` | unique effect ID/idempotency key | double dispatch | 6 |
| migration | supported schema only, backup before change | silent coercion | 3/6 |

Single writer is enforced by SQLite; readers use transactions. Counts, bytes, tokens, money, and hashes are integers or fixed strings.

## 7. Reuse map and adoption decisions

| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-store/src/kv/store.rs:14-71` | local LV; current redb KV API | extraction candidates and tests | preserve typed get/put/delete semantics | migrate to authority schema |
| `crates/fleet-store/src/ledger/append.rs` | local package seam; ledger append tests | receipt/hash-chain behavior | existing chain fixtures | SQLite transaction integration |
| `rusqlite 0.32.1` | MIT; crates.io/repo and exact checksum in `_research/effects-operations.md` | SQLite WAL binding | mature direct binding; no custom engine — **0.40.2 is a research candidate only, do not adopt** (see `effects-operations.md:30`); Cargo.lock resolves 0.32.1 | exact-pin compile, WAL/crash probe |
| SQLite backup API | primary SQLite docs https://sqlite.org/backup.html | backups | provider primitive | restore and checksum proof |

Current `fleet-store` uses `redb`, so adopting SQLite is a deliberate partial migration, not an extraction claim. The research package probe was not locally run.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror  = "2"
tokio      = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix

### `append_event`, `cas`, `migrate`

| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | missing key/event refuses with receipt attempt; empty query returns `checked=0` and fails its gate |
| wrong type / Unicode | schema/type validation refuses; Unicode is UTF-8 and digest-stable |
| huge / negative | reject >1 MiB frame/reference missing blob; reject negative revision/count |
| duplicate / concurrent | unique event/idempotency returns existing commit; stale CAS returns typed conflict |
| partial I/O / timeout | rollback whole transaction; preserve emergency receipt path; return environment fault 3 |
| stale / unavailable | unsupported future schema refuses; locked DB retries bounded then returns 3 |

## 9. Tiny implementation steps

1. In `src/lib.rs` and `src/schema.rs`: Add rusqlite dependency and schema version table DDL; `cargo check -p store` exits 0.
2. In `src/schema.rs`: Implement `events`, `nodes`, `inbox`, `outbox`, and `leases` table DDL with typed migration runner; `cargo test -p store migration_round_trip` passes.
3. In `src/transaction.rs`: Implement `append_event` and `cas` with revision validation, duplicate detection, and whole-transaction rollback on failure; `cargo test -p store cas_rejects_stale_revision` passes.
4. In `src/artifact.rs`: Implement atomic blob publication with fsync, hash verification, and reference integrity; `cargo test -p store incomplete_blob_is_rejected` passes.
5. In `src/retention.rs` and `tests/crash_recovery.rs`: Implement measured GC and backup restore; assert durability through a real binary restart; `cargo test -p store crash_recovery_restores_committed_events` passes.

## 10. Test matrix

**Unit tests:** schema validation, revision arithmetic, digest chain, idempotency key parsing.

**Integration/contract tests:** ingest commit contains inbox/event/receipt; control transition contains state/effect/outbox in one transaction.

**Hidden tests:** killed writer before commit, disk-full simulation, WAL growth threshold, future schema, two concurrent CAS writers.

**Property tests:** 1,000 generated event sequences replay to one projection and valid chain; fixed seed.

**Differential tests:** existing redb KV/ledger fixtures versus SQLite adapter for get/put/delete and receipt semantics; differences must be listed in migration tests.

**Real-binary/effect test:** run `fleet` against a temporary state directory, restart after a committed event, and verify the same receipt is loaded.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `store::tests::migration_round_trip` | Call `migrate` on a fresh in-memory SQLite connection, then call it again on the same connection | `MigrationReport` with `checked=3,total=3` on first call; idempotent (no error) on second call | catches skipping migration logic; a no-op migrate returns wrong counts or breaks subsequent writes |
| `store::tests::cas_rejects_stale_revision` | Two sequential `cas` calls on the same key where the second call uses the original revision (now stale) | Second call returns `Err(StoreError::StaleRevision)` with the current revision in the error | catches ignoring the expected revision; a stale writer must not win |
| `store::tests::crash_recovery_restores_committed_events` | Write one event via `append_event`, drop the connection, reopen the same file path, and call `load` for the committed event | `Some(Record)` matching the original event's content and revision | lld durability contract — proves fsync/WAL commit is real; catches returning `Ok(())` without actually committing |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `append_event` in `src/transaction.rs` | Return `Ok(Commit { .. })` without calling SQLite `COMMIT` | `store::tests::crash_recovery_restores_committed_events` | Durability is observable — reopening the file after a fake commit returns `None`, failing the reload assertion |
| `cas` in `src/transaction.rs` | Skip the expected-revision check, accepting any revision | `store::tests::cas_rejects_stale_revision` | Stale writer cannot win; removing the revision guard allows a second writer to silently overwrite |
| receipt write in `src/transaction.rs` | Omit writing the receipt row when a transaction is refused (e.g. disk full) | hidden disk-full/refusal test | Audit invariant holds; a refusal without a receipt destroys the evidence trail |
| denominator check in `src/schema.rs` | Report `checked=0, total=0` on a successful migration and return `Ok` | `store::tests::migration_round_trip` | Denominator is enforced; zero coverage on a passing migration is an illegal state |

Reviewer manually removes the unique inbox constraint in a temporary schema; duplicate contract test must fail.

Safety mutation floor: `caught/total >= 80%` across transaction and artifact targets; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators

```bash
cargo test -p store --no-fail-fast
cargo clippy -p store --all-targets -- -D warnings
find crates/store -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-store --no-fail-fast
cargo test --test store_control_contract --no-fail-fast
```

Expected evidence: schema migrations `checked=3,total=3`; replay sequences `checked=1000,total=1000`; crash/restart receipts `checked=1,total=1`; mutation floor `caught/total >= 0.80` for authority predicates. `cargo info` plus temporary probe is required for rusqlite adoption.

## 13. Definition of done

All of the following must be true — each is checkable by inspection or command output, no subjective criteria:

- `cargo test -p store --no-fail-fast` exits 0 with `test result: ok` in output.
- `store::tests::migration_round_trip` appears in test output and passes.
- `store::tests::cas_rejects_stale_revision` appears in test output and passes.
- `store::tests::incomplete_blob_is_rejected` appears in test output and passes.
- `store::tests::crash_recovery_restores_committed_events` appears in test output and passes.
- `cargo clippy -p store --all-targets -- -D warnings` exits 0 (zero warnings).
- No source file under `crates/store/src/` exceeds 80 lines (`find crates/store/src -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0).
- `target/debug/fleet store --fixture tests/fixtures/blueprint-store/store-fixture.json` output contains `checked=1,total=1`.
- Mutation floor: `caught/total >= 80%` across transaction and artifact targets.

## 14. Failure stories and review questions

**Failure → Cause → Fix:** stale worker overwrote a newer state → write lacked expected revision → CAS and lease generation fence reject it with receipt.

1. Can the store prove restart durability? 2. What happens on disk full? 3. Is a local transaction being mistaken for external completion?
