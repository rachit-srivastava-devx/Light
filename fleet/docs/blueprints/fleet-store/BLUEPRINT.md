# BLUEPRINT — `fleet-store`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-store`
- **One-line purpose:** Own fleet's three durable stores — the append-only blake3 hash-chained
  ledger (source of truth), the rusqlite code-graph, and the hybrid FTS5/vector memory
  substrate — plus a redb-backed embedded KV, each behind a small, path-injected, typed-error Rust
  API, with zero scoring/ranking/decision logic layered on top of the raw rows.
- **Build branch:** `refactor+unify` (MIGRATION-PLAN §3 row 2). Three independently-evolved
  storage engines (one Rust monolith module, one Rust monolith module, one Python script) get
  collected into one crate behind per-domain traits/structs, not literally merged into one god
  type — see the "resists unification" note at the end of this file.
- **Imports:** `fleet-types` (`Receipt`, `ReceiptEvent`, `PrevHash`, `Blake3Hash`, `SchemaV1`,
  `ExitCode` — the ledger's rows are `fleet_types::Receipt` values, constructed here, never a
  parallel local copy).
- **Imported by:** `fleet-govern` (budget/quota/cooldown persistence — MIGRATION-PLAN row 11's
  "route.rs failover" state needs a durable home), `fleet-memory` (raw BM25/vector rows; it owns
  the RRF fusion and embedding computation built on top — see §2), `fleet-context` (the code-graph
  read/write path once it has parsed a project — see §5's graph.rs split), `fleet-verify` (the
  ledger-verify gate), `fleet-worker` (append receipts on run start/end/lane-status), `src/`
  (composition root: ledger CLI subcommands, receipt-writing on refusal that today's `route.rs`
  does inline — MIGRATION-PLAN row 4's divergence note already assigns that call here).

## 2. Responsibility & non-goals

**Owns:** the physical durability layer and nothing above it. Concretely: (a) the ledger — a
single-writer, advisory-locked, append-only JSONL file whose rows chain by blake3 hash over
`prev_hash + canonical row bytes`, plus the full-chain tamper walk (sequence continuity,
prev-hash linkage, fork detection, content-hash mismatch) that makes every row's presence
provable; (b) the code-graph store — a rusqlite schema of `projects/files/symbols/edges/aliases`
and the recursive "who depends on this symbol" query, fed a caller-supplied re-index batch it
writes transactionally; (c) the memory store's *raw* substrate — a `memories` row table, its FTS5
shadow index, and a vector table holding caller-supplied embedding bytes, exposing separate raw
BM25 match and raw vector k-NN as two unfused ranked lists; (d) a generic redb-backed embedded KV
for small state that needs neither a JSONL chain nor a SQL schema; (e) a shared retention layer —
one `RetentionPolicy`/`PruneReport`/`UsageReport` vocabulary (`src/retention.rs`) that the graph,
memory, and KV stores each implement as their own `usage()`/`prune()` pair, and that the ledger
implements only as a hard refusal (`Ledger::prune` always returns `LedgerError::LedgerRefused` —
an append-only hash-chained file cannot have rows deleted from it without breaking verifiability).

**Non-goals (the seam):**
- Does **not** decide which `ReceiptEvent` a caller means — today's `append_receipt` silently
  remaps the string `"note"` to `"gate_verdict"` (`main.rs:4368-4372`); that remapping is a
  business-vocabulary decision, not storage, so it moves to the caller (`fleet-worker`/`src/`),
  which must already hold a typed `fleet_types::ReceiptEvent` before calling `append`.
- Does **not** compute embeddings — no `fastembed`/model call anywhere in this crate
  (`memory_store.py:239-259`'s `embedder`/`embed_texts` do not get lifted). The caller
  (`fleet-memory`) computes vectors and passes `&[f32]`; this crate only stores and retrieves
  them.
- Does **not** fuse BM25 and vector rankings — no reciprocal-rank-fusion, no `benchmark` harness
  (`memory_store.py:349-405`'s `search`, `:432-552`'s `benchmark`). Per the assignment brief:
  this crate is the storage engine, `fleet-memory` owns the scoring built on top of the two raw
  ranked lists this crate returns.
- Does **not** parse source code — no tree-sitter, no git shell-outs (`graph.rs:696-1172`'s
  `scan_tree`/`parse_file`/`collect_nodes`/`definition`/`call`/`git_commit`/`git_renames`/
  `assign_symbol_ids`/`resolve_edges` stay in `fleet-context`). This crate receives an already-
  extracted `ReindexBatch` and only persists it.
- Does **not** decide budget/quota/cooldown policy — it persists whatever rows `fleet-govern`
  asks it to; no admission logic lives here.
- Does **not** shell out to a subprocess for vector search — `memory_store.py:197-221`'s
  `vector_bridge` (spawning a second Python process per query to load `sqlite-vec`) is replaced
  by loading the same installed `vec0.{dylib,so,dll}` artifact in-process via rusqlite's
  `load_extension` — see §5.
- Does **not** print CLI output, parse args, or decide exit codes — `src/` (composition root)
  translates this crate's typed errors into `fleet_types::ExitCode` and formats output.

## 3. Public API contract

```rust
//! Fleet's durable state layer: ledger, code-graph, hybrid memory substrate, embedded KV.
//!
//! Every store here takes its filesystem path(s) from the caller at construction time -- this
//! crate never derives a path from an environment variable or a default directory. Every
//! fallible operation returns a typed, per-domain error enum. No store in this crate ranks,
//! scores, fuses, or decides anything about the rows it holds; it stores and retrieves them
//! exactly as given.

use std::path::{Path, PathBuf};
use fleet_types::{Blake3Hash, ExitCode, PrevHash, Receipt, ReceiptEvent, SchemaV1};
use serde_json::Value;

// =====================================================================================
// Shared: filesystem-level failure, common to every store in this crate.
// =====================================================================================

/// A failure at the OS boundary (open/lock/read/write) -- never a domain-logic failure, which
/// each store's own error enum carries instead.
#[derive(Debug, thiserror::Error)]
pub enum IoFault {
    #[error("could not open {path}: {source}")]
    Open { path: PathBuf, source: std::io::Error },
    #[error("could not acquire exclusive lock on {path}: {source}")]
    Lock { path: PathBuf, source: std::io::Error },
    #[error("could not read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("could not write {path}: {source}")]
    Write { path: PathBuf, source: std::io::Error },
}

// =====================================================================================
// A. Ledger -- fleet/keel/fleet/src/main.rs:4302-4605 (append_receipt/ledger_verify/verify_rows)
// =====================================================================================

/// The two files the ledger needs, supplied by the caller -- this crate derives neither from
/// `$HOME`, a state-dir helper, nor any other ambient source (contra `main.rs:4302-4308`'s
/// `ledger_paths`, which reads `state_dir()`; that ambient lookup is now the caller's job).
pub struct LedgerPaths {
    pub chain: PathBuf,
    pub lock: PathBuf,
}

/// Why an append or a verify failed. Every variant names the offending row's sequence number so
/// a caller can report exactly what `verify_rows`'s `eprintln!`s did (`main.rs:4446-4497`), as
/// structured data instead of stderr text.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error(transparent)]
    Io(#[from] IoFault),
    #[error("row {seq} is not a well-formed receipt: {reason}")]
    MalformedRow { seq: u64, reason: String },
    #[error("row {seq} carries seq={found} but its chain position is {expected}")]
    SeqMismatch { seq: u64, found: u64, expected: u64 },
    #[error("row {seq} prev_hash does not match the prior row's hash -- the chain link is broken")]
    BrokenLink { seq: u64 },
    #[error("row {seq} reuses a prev_hash another row already claimed -- the chain forks")]
    Fork { seq: u64 },
    #[error("row {seq} hash does not match its recomputed content hash -- tampered after write")]
    Tampered { seq: u64 },
    #[error("the ledger is empty")]
    Empty,
    #[error("ledger pruning is refused: the chain is append-only and hash-chained; deleting rows would break verifiability")]
    LedgerRefused,
}

/// The outcome of a full chain walk. `checked == total` always holds when `Ok` is returned --
/// `verify` never returns `Ok` having skipped a row (mirrors `main.rs:4437`'s printed invariant).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedChain {
    pub checked: u64,
    pub total: u64,
}

/// A handle to one ledger. Opening does no IO -- IO happens per call, under the file lock, so a
/// `Ledger` value can be constructed freely and shared by reference across threads/tasks.
pub struct Ledger {
    paths: LedgerPaths,
}

impl Ledger {
    /// Construct a handle. Does not touch the filesystem; `chain`/`lock` need not exist yet.
    pub fn open(paths: LedgerPaths) -> Self { unimplemented!() }

    /// Append one receipt. Verifies the existing chain first (`main.rs:4365-4367`), refuses if
    /// it's already broken rather than extending a corrupt chain. Stamps `seq`/`prev_hash`/
    /// `ts_wall`/`hash` itself -- the caller supplies only the fields a worker is allowed to
    /// author (`event`, `body`, `actor`, `resolved_model`, `exit_code`).
    pub fn append(
        &self,
        event: ReceiptEvent,
        body: Value,
        actor: String,
        resolved_model: Option<String>,
        exit_code: Option<ExitCode>,
    ) -> Result<Receipt, LedgerError> { unimplemented!() }

    /// Every row in chain order. `allow_empty = false` mirrors `ledger_rows`'s default
    /// (`main.rs:4330-4338`): an empty chain is `Err(LedgerError::Empty)`, not `Ok(vec![])`,
    /// because most callers (`count`, `dump`, `verify`) treat "never initialized" as an
    /// invariant violation, not a valid zero state.
    pub fn rows(&self, allow_empty: bool) -> Result<Vec<Receipt>, LedgerError> { unimplemented!() }

    /// Walk the full chain and verify sequence continuity, prev-hash linkage, fork-freedom, and
    /// content-hash match, in that order, per row (`main.rs:4441-4501`'s `verify_rows`). Returns
    /// the first failure found -- never aggregates multiple corruptions into one report.
    pub fn verify(&self) -> Result<VerifiedChain, LedgerError> { unimplemented!() }

    /// Row count and on-disk byte size of the chain file.
    pub fn usage(&self) -> Result<UsageReport, LedgerError> { unimplemented!() }

    /// Always refused: the chain is append-only and hash-chained, so deleting any row would break
    /// verifiability. Callers wanting to shrink a chain must archive or rotate segments instead.
    pub fn prune(&self, policy: &RetentionPolicy) -> Result<PruneReport, LedgerError> { unimplemented!() }
}

// =====================================================================================
// B. Code-graph -- fleet/keel/fleet/src/graph.rs:612-694 (schema+reads), :390-484 (writes)
// =====================================================================================

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error(transparent)]
    Io(#[from] IoFault),
    #[error("sqlite: {0}")]
    Sql(String),
    #[error("depth must be >= 1, got {0}")]
    BadDepth(u64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRecord {
    pub project_id: String,
    pub root_path: PathBuf,
    pub tree_digest: String,
    pub indexed_commit: String,
    pub indexed_file_count: u64,
    pub floor: u64,
    pub files_skipped: u64,
    pub languages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileRecord { pub path: String, pub language: String, pub digest: String }

#[derive(Clone, Debug, PartialEq)]
pub struct SymbolRecord {
    pub symbol_id: String, pub path: String, pub name: String, pub arity: u64,
    pub kind: String, pub line: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeRecord { pub caller_id: String, pub callee_id: String }

#[derive(Clone, Debug, PartialEq)]
pub struct AliasRecord {
    pub path: String, pub name: String, pub arity: u64,
    pub from_commit: String, pub to_commit: String, pub symbol_id: String,
}

/// One project's full re-index, already computed by the caller (`fleet-context`'s tree-sitter
/// scan). This crate never parses; it only replaces one project's rows transactionally, mirroring
/// `graph.rs:390-484`'s delete-then-insert-under-one-transaction shape.
pub struct ReindexBatch {
    pub project: ProjectRecord,
    pub files: Vec<FileRecord>,
    pub symbols: Vec<SymbolRecord>,
    pub edges: Vec<EdgeRecord>,
    pub aliases: Vec<AliasRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DependentRecord {
    pub symbol_id: String, pub path: String, pub name: String, pub arity: u64,
    pub kind: String, pub line: u64, pub depth: u64,
}

pub struct GraphStore { /* wraps rusqlite::Connection, opened against a caller-given path */ }

impl GraphStore {
    /// Open (creating if absent) and ensure the schema exists (`graph.rs:612-651`'s `open_db`).
    pub fn open(path: &Path) -> Result<Self, GraphError> { unimplemented!() }

    /// The project row for this root path, if one was ever indexed (`graph.rs:653-674`).
    pub fn project(&self, root_path: &Path) -> Result<Option<ProjectRecord>, GraphError> { unimplemented!() }

    /// Every symbol currently stored for a project (`graph.rs:676-694`).
    pub fn symbols(&self, project_id: &str) -> Result<Vec<SymbolRecord>, GraphError> { unimplemented!() }

    /// Replace one project's `files`/`symbols`/`edges`/`aliases` and upsert its `projects` row,
    /// all inside one transaction (`graph.rs:390-484`). Either every row lands or none does.
    pub fn replace_project(&mut self, batch: ReindexBatch) -> Result<(), GraphError> { unimplemented!() }

    /// Resolve a name or bare symbol-id to every matching current symbol-id, folding in alias
    /// history (`graph.rs:1173-1200`'s `target_symbol_ids`).
    pub fn symbol_ids_for(&self, project_id: &str, symbol: &str) -> Result<Vec<String>, GraphError> { unimplemented!() }

    /// The recursive "who (transitively) calls any of `target_ids`" closure, bounded by `depth`
    /// (`graph.rs:1202-1249`'s `query_dependents`). `depth == 0` is a typed error, not a silently
    /// empty result -- `graph.rs`'s CLI clamps this before calling; this crate does not assume a
    /// caller will.
    pub fn dependents(
        &self,
        project_id: &str,
        target_ids: &[String],
        depth: u64,
    ) -> Result<Vec<DependentRecord>, GraphError> { unimplemented!() }

    /// Number of `projects` rows and the sqlite file's byte size.
    pub fn usage(&self) -> Result<UsageReport, GraphError> { unimplemented!() }

    /// Delete whole projects (and only whole projects, so no child table is ever left dangling)
    /// until every applicable limit in `policy` is satisfied. `max_age_secs` is ignored -- the
    /// graph schema has no timestamped column to age against.
    pub fn prune(&mut self, policy: &RetentionPolicy) -> Result<PruneReport, GraphError> { unimplemented!() }
}

// =====================================================================================
// C. Memory substrate (raw) -- fleet/registry-reference/.../memory_store.py:33-346 (storage half)
// =====================================================================================

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity { Critical, Important, Minor }

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryRow {
    pub id: String, pub title: String, pub body: String, pub source: String,
    pub keywords: Vec<String>, pub evidence: String, pub severity: Severity,
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error(transparent)]
    Io(#[from] IoFault),
    #[error("sqlite: {0}")]
    Sql(String),
    #[error("embedding has {found} dimensions, expected {expected}")]
    DimensionMismatch { expected: u32, found: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bm25Hit {
    pub id: String, pub source: String, pub title: String,
    pub severity: Severity, pub confirmed_count: u32, pub relevance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorHit { pub id: String, pub distance: f32 }

pub struct MemoryStore { /* wraps rusqlite::Connection with FTS5 + the loaded vec0 extension */ }

impl MemoryStore {
    /// Open (creating if absent), ensure `memories`/`memories_fts`/the vector table exist, and
    /// load the vec0 extension in-process (`memory_store.py:42-104`, minus the Python bridge).
    pub fn open(path: &Path, vector_dimensions: u32) -> Result<Self, MemoryError> { unimplemented!() }

    /// Insert-or-confirm one row by id: a fresh id inserts at `confirmed_count = 1`; an existing
    /// id updates content and increments `confirmed_count`, matching `remember`'s `ON CONFLICT`
    /// (`memory_store.py:408-423`). Returns the sqlite rowid new callers need for `set_embedding`.
    pub fn upsert(&mut self, row: MemoryRow) -> Result<u64, MemoryError> { unimplemented!() }

    /// Store (or replace) one row's embedding plus the content-hash it was computed from
    /// (`memory_store.py:262-306`'s write half, minus embedding computation itself).
    pub fn set_embedding(&mut self, rowid: u64, vector: &[f32], content_hash: &str) -> Result<(), MemoryError> { unimplemented!() }

    /// `(rowid, stored_content_hash)` for every row that has a stored embedding, so the caller
    /// (`fleet-memory`) can diff against freshly-computed hashes and decide what to re-embed --
    /// the caller-side half of `ensure_embeddings`'s staleness check (`memory_store.py:262-278`).
    pub fn embedding_hashes(&self) -> Result<Vec<(u64, String)>, MemoryError> { unimplemented!() }

    /// Raw BM25 match over pre-tokenized terms, unranked against anything but BM25 itself
    /// (`memory_store.py:326-346`'s `bm25_search`, minus the RRF fusion that used to sit next to
    /// it in the same function).
    pub fn bm25_search(&self, terms: &[String], limit: u32) -> Result<Vec<Bm25Hit>, MemoryError> { unimplemented!() }

    /// Raw vector k-NN, unranked against anything but distance (`memory_store.py:360-364`'s
    /// vector half of `search`, minus the fusion).
    pub fn vector_search(&self, query: &[f32], limit: u32) -> Result<Vec<VectorHit>, MemoryError> { unimplemented!() }

    /// Row count (`memory_store.py:99-100`).
    pub fn count(&self) -> Result<u64, MemoryError> { unimplemented!() }

    /// Current number of `memories` rows and the sqlite file's byte size.
    pub fn usage(&self) -> Result<UsageReport, MemoryError> { unimplemented!() }

    /// Delete memories (and their dependent FTS5/vector/vector-meta rows, since there is no
    /// DELETE trigger to cascade this) until every applicable limit in `policy` is satisfied.
    /// `max_age_secs` is checked against each row's `updated_at`, anchored by the caller-injected
    /// `now` -- not read from the system clock inside this crate.
    pub fn prune(&mut self, policy: &RetentionPolicy, now: std::time::SystemTime) -> Result<PruneReport, MemoryError> { unimplemented!() }
}

// =====================================================================================
// D. Embedded KV -- greenfield, backed by redb (see §7)
// =====================================================================================

#[derive(Debug, thiserror::Error)]
pub enum KvError {
    #[error(transparent)]
    Io(#[from] IoFault),
    #[error("redb: {0}")]
    Redb(String),
}

/// A single-file, ACID, embedded key/value store for state that doesn't need a JSONL chain or a
/// SQL schema (e.g. a scanner's resume cursor, a small feature-flag set). Every key is scoped to
/// a named table so unrelated callers can't collide.
pub struct KvStore { /* wraps redb::Database, opened against a caller-given path */ }

impl KvStore {
    pub fn open(path: &Path) -> Result<Self, KvError> { unimplemented!() }
    pub fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> { unimplemented!() }
    pub fn put(&self, table: &str, key: &[u8], value: &[u8]) -> Result<(), KvError> { unimplemented!() }
    /// Returns whether a value was actually removed.
    pub fn delete(&self, table: &str, key: &[u8]) -> Result<bool, KvError> { unimplemented!() }
    pub fn list_prefix(&self, table: &str, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KvError> { unimplemented!() }

    /// Total entry count across every table and the redb file's byte size.
    pub fn usage(&self) -> Result<UsageReport, KvError> { unimplemented!() }

    /// Delete entries, in a deterministic per-table/per-key order, until every applicable limit
    /// in `policy` is satisfied. `max_age_secs` is ignored -- entries carry no timestamp column.
    pub fn prune(&mut self, policy: &RetentionPolicy) -> Result<PruneReport, KvError> { unimplemented!() }
}

// =====================================================================================
// E. Retention -- greenfield shared vocabulary (`src/retention.rs`), implemented per-store above
// =====================================================================================

/// Per-store retention limits. A `None` field means "no limit" for that dimension.
#[derive(Clone, Debug, Default)]
pub struct RetentionPolicy {
    pub max_age_secs: Option<u64>,
    pub max_rows: Option<u64>,
    pub max_bytes: Option<u64>,
}

/// What was actually removed by a prune call. Never a silent bool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PruneReport {
    pub rows_removed: u64,
    pub bytes_reclaimed: u64,
}

/// Current on-disk size of a store, measured when the usage call is made.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsageReport {
    pub row_count: u64,
    pub byte_size: u64,
}

/// Why a retention operation failed. Each variant names the store it applies to; the ledger's own
/// `RetentionError::Ledger(LedgerError::LedgerRefused)` is the one prune call that is *always* an
/// error (see §2/§4).
#[derive(Debug, thiserror::Error)]
pub enum RetentionError {
    #[error("ledger pruning is refused: the chain is append-only and hash-chained; deleting rows would break verifiability")]
    LedgerRefused,
    #[error("graph store: {0}")]
    Graph(#[from] GraphError),
    #[error("memory store: {0}")]
    Memory(#[from] MemoryError),
    #[error("kv store: {0}")]
    Kv(#[from] KvError),
    #[error("ledger: {0}")]
    Ledger(#[from] LedgerError),
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `Ledger`/`LedgerPaths` | Both files are supplied by the caller at construction; no path is ever derived from `$HOME`/an env var inside this crate. | A test or a second tenant silently reading/writing the production ledger because a default path leaked in. |
| `Receipt` (via `Ledger::append`) | `seq` is always `rows().len()` at append time; `prev_hash` is always the prior row's `hash` (or `PrevHash::Genesis` for row 0); `hash` is always `blake3(prev_hash_bytes ++ canonical_row_bytes)` — never client-supplied. | A worker forging its own `seq`/`hash`/`prev_hash` — every one of those three fields is stamped by `append`, never accepted as an argument (§3's signature has no such parameters). |
| `VerifiedChain` | `checked == total` whenever `Ok` is returned; `verify` never returns `Ok` after skipping a row. | A caller trusting a "verified" result that silently only checked a prefix of the chain. |
| `GraphStore::replace_project` | Either every row of `files`/`symbols`/`edges`/`aliases` for one project lands, or (on any error) none do — the whole batch is one rusqlite transaction. | A crash mid-reindex leaving a project with symbols from the old scan and edges from the new one (a torn write). |
| `ReindexBatch` | Carries `Vec`s, not a lazy iterator — the whole batch must already be materialized (i.e. tree-sitter parsing already finished) before this crate is called; this crate does no partial/streaming ingestion. | A caller trying to stream rows in one at a time and getting inconsistent partial-project reads between them. |
| `MemoryStore::set_embedding` | `vector.len()` must equal the `vector_dimensions` given to `open` — checked before any SQL runs, returns `DimensionMismatch` rather than silently truncating/padding. | A 384-dim query vector compared against a corrupted 256-dim stored row without either side noticing. |
| `MemoryStore::upsert` | `confirmed_count` only ever increments on a re-`upsert` of the same `id`; a fresh `id` always starts at 1. | A duplicate `remember` call silently resetting confidence instead of reinforcing it. |
| `KvStore` tables | Keys/values are opaque `&[u8]` — this crate never interprets or validates their contents; that's entirely the caller's concern (it is a KV store, not a schema). | A caller assuming this crate enforces some structure on their blob and being surprised when it doesn't. |

**Money/precision:** no money type in this crate. Token/row counts (`indexed_file_count`,
`confirmed_count`, `checked`/`total`, `arity`, `line`, `depth`) are unsigned integers throughout —
never float. Vector components (`&[f32]`) are the one intentional float use in this crate, because
they are genuinely continuous embedding coordinates, not a count; this crate treats them as opaque
bytes for storage/distance purposes and performs no float arithmetic of its own beyond what
sqlite-vec's KNN operator does internally.

**Clock/RNG/IO injection points:** `Ledger::append` stamps `ts_wall` — this is the one place this
crate touches a clock, and it does so directly (`now_rfc3339`-equivalent), mirroring
`main.rs:4400`'s existing behavior exactly (not a new ambient read, the pre-existing one). No RNG
anywhere (project ids are supplied by the caller in `ProjectRecord`, not generated here — see §5's
`Uuid::new_v4` note). IO: every store's `open`/constructor takes a `&Path` from the caller; no
store reads an environment variable, a default directory, or a "state dir" helper internally. Tests
construct every store against a `tempdir()` path — never the repo tree.

## 5. Reuse map

Source: `fleet/keel/fleet/src/main.rs` (6460 lines; ledger section read 4279-4618, 2026-09-08),
`fleet/keel/fleet/src/graph.rs` (1351 lines; schema+query+write sections read 331-503, 612-694,
1173-1249), `fleet/registry-reference/registry/features/memory/memory_store.py` (592 lines, read
in full).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `main.rs:4302-4308` (`ledger_paths`) | Derives chain/lock paths from `state_dir()`. | no | The path derivation itself stays with the caller; this crate's `LedgerPaths` is the injected result, not a re-derivation. |
| `main.rs:4310-4328` (`FileLock`) | `fs2`-based advisory exclusive lock, unlocked on `Drop`. | yes | Ported verbatim as the lock primitive backing `Ledger::append`/`rows`/`verify`. |
| `main.rs:4330-4353` (`ledger_rows`, `read_rows`) | Reads the lock, reads all JSONL rows, `EXIT_INVARIANT` on empty-when-disallowed. | yes, logic | Return typed `Receipt`/`LedgerError` instead of `Value`/`i32`; `Ledger::rows(allow_empty)` is the direct port. |
| `main.rs:4355-4427` (`append_receipt`) | Verifies existing chain, builds the row, computes `blake3:{hex}` over `prev_hash_bytes ++ canonical_json`, appends, `sync_data`s. | yes, logic | Drop the `"note" -> "gate_verdict"` remap (§2 non-goal — caller decides) and the inline 8-string event whitelist (replaced by the caller already holding a typed `ReceiptEvent`); keep the hash computation byte-for-byte identical (existing ledger files must still verify). |
| `main.rs:4429-4501` (`ledger_verify`, `verify_rows`) | Full chain walk: per-row `validate_receipt_row`, seq continuity, prev-hash linkage, fork detection (`seen_prev` set), content-hash recompute-and-compare, first-failure-wins reporting via `eprintln!` + typed exit code. | yes, logic | Same walk, but each named failure becomes a `LedgerError` variant (`MalformedRow`/`SeqMismatch`/`BrokenLink`/`Fork`/`Tampered`) instead of an `eprintln!` + `EXIT_MISMATCH`; the caller formats/prints. |
| `main.rs:4503-4593` (`validate_receipt_row`) | Field-set/type validation per row (`ALLOWED` keys, `schema_version == "1.0"`, hash format, event whitelist). | yes, logic | Folded into `Ledger::verify`'s per-row check; event validity now comes from `ReceiptEvent`'s own `Deserialize` (an unknown event string simply fails to parse as a `Receipt`, which is `MalformedRow`) rather than a second hand-written whitelist. |
| `main.rs:4595-4605` (`valid_hash`, `valid_hash_or_genesis`) | `"blake3:" + 64 lowercase hex` shape check. | no — **already fleet-types's job** | `fleet_types::Blake3Hash::parse`/`PrevHash::parse` (its blueprint §3) own this validation; this crate calls those, it does not re-check the shape itself. |
| `main.rs:4980` (`blake3_hex`) | Hex-encodes a blake3 digest. | yes | Ported verbatim as a private helper behind `Ledger::append`/`verify`. |
| `graph.rs:612-651` (`open_db`) | Opens/creates the sqlite file, sets `busy_timeout`, runs the `CREATE TABLE IF NOT EXISTS` schema for `projects/files/symbols/edges/aliases` + two indexes. | yes | Near-verbatim `GraphStore::open`; swap `i32` errors for `GraphError`. |
| `graph.rs:653-694` (`load_project`, `load_symbols`) | Read the current project row / all symbol rows for a project. | yes | Direct port as `GraphStore::project`/`symbols`. |
| `graph.rs:390-484` (the transactional write half of `index_command`) | Upserts the `projects` row, deletes+reinserts `edges`/`files`/`symbols`, inserts aliases, all in one `connection.transaction()`. | yes, logic | Becomes `GraphStore::replace_project(ReindexBatch)`; the surrounding `index_command` (lines 331-389, 485-501: arg parsing, `scan_tree`, `parse_file`, `assign_symbol_ids`, `resolve_edges`, JSON printing) is **not** lifted — it is `fleet-context` (parsing) plus `src/` (CLI/printing), matching §2's non-goal. |
| `graph.rs:350` (`Uuid::new_v4()` for a fresh `project_id`) | Generates a new project id when none existed. | no | RNG stays with the caller — `ReindexBatch.project.project_id` arrives already decided; this crate never calls `Uuid::new_v4` itself (§4's "no RNG" claim depends on this). |
| `graph.rs:1173-1200` (`target_symbol_ids`) | Resolves a name/id to every matching current + aliased symbol-id. | yes | Direct port as `GraphStore::symbol_ids_for`. |
| `graph.rs:1202-1249` (`query_dependents`) | Recursive CTE over `edges`, bounded by `depth`, joined back to `symbols` for display fields. | yes | Direct port as `GraphStore::dependents`; add the `depth == 0` typed-error guard (§4) that today's caller enforces before calling, not the function itself. |
| `graph.rs:696-1172` (`scan_tree`, `parse_file`, `collect_nodes`, `definition`, `call`, `assign_symbol_ids`, `resolve_edges`, `git_commit`, `git_renames`) | Tree-sitter parsing + git introspection that produces the rows `replace_project` stores. | **no** | Stays in `fleet-context` per MIGRATION-PLAN row 7 — this is the "only part of the file qualifies" gap the router blueprint already flagged for `route.rs`; the same caution applies here. |
| `memory_store.py:42-104` (`connect`) | Opens the sqlite file, WAL/synchronous pragmas, creates `memories`/`memories_fts`/its 3 triggers/`memory_vector_meta`, rebuilds the FTS index if row counts disagree. | yes, logic | Ported as `MemoryStore::open`'s schema half; add loading the `vec0` extension in-process (see next row) instead of the Python bridge. |
| `memory_store.py:160-170` (`vector_extension`) | Locates the installed `sqlite_vec` package's compiled `vec0.{dylib,so,dll}`. | yes, as a path lookup | Same file-location logic, called from Rust via `rusqlite::Connection::load_extension` at `MemoryStore::open` time, instead of being handed to a spawned Python process. |
| `memory_store.py:197-221` (`vector_bridge`) | Spawns `python3 memory_vec.py ... {count,sync,knn}` per call and parses its stdout JSON. | **no** | This whole subprocess-per-query indirection disappears — once `vec0` is loaded in-process (previous row), `MemoryStore::vector_search`/`set_embedding` issue SQL directly against the loaded virtual table. |
| `memory_store.py:262-313` (`ensure_embeddings`) | Diffs stored vs current content-hash, calls the embedder, writes vectors + `memory_vector_meta`, and asserts the three counts (`memories`/vectors/`memory_vector_meta`) agree. | split | The *write* half (store vector + hash, delete stale) is `MemoryStore::set_embedding` + a stale-row delete path; the *decide what's stale* half (hashing, calling `embedder()`) is `fleet-memory`'s job — `embedding_hashes` gives it what it needs to decide. |
| `memory_store.py:326-346` (`bm25_search`) | FTS5 MATCH + `bm25()` ranking, one SQL statement. | yes, as the raw half | `MemoryStore::bm25_search`; the tokenization (`terms()`, `memory_store.py:316-323`) is a pure string function that also moves to `fleet-memory` since it's query-shaping, not storage — this crate takes already-tokenized terms. |
| `memory_store.py:349-405` (`search`) | BM25 + vector KNN + reciprocal-rank-fusion into one ranked list. | **no**, split | The two raw calls (`bm25_search`/`vector_search` above) are this crate's job; the RRF math (`rrf_score`, lines 386-395) is **not** lifted — `fleet-memory` owns it, per §2. |
| `memory_store.py:408-423` (`remember`) | `INSERT ... ON CONFLICT DO UPDATE` bumping `confirmed_count`. | yes, logic | Direct port as `MemoryStore::upsert`. |
| `memory_store.py:432-552` (`benchmark`) | Precision/recall harness comparing BM25 vs hybrid arms. | **no** | Evaluation tooling, not storage — belongs with `fleet-memory`'s test/eval surface if kept at all. |

## 6. Behavior spec

### `fn Ledger::append(&self, event, body, actor, resolved_model, exit_code) -> Result<Receipt, LedgerError>`

| Input dimension | Behavior |
|---|---|
| empty | `body = json!({})` (empty object) → accepted; the schema requires `body` to be an object, not a non-empty one (matches `validate_receipt_row`'s `is_object` check, `main.rs:4527`). `actor = String::new()` → accepted; non-empty `actor` is not enforced here (the schema names it required-present, not required-non-empty) — a caller wanting that must validate before calling. |
| null / `None` | `resolved_model: None`, `exit_code: None` → both serialize as JSON `null` on the `Receipt`, per `#[serde(skip_serializing_if = "Option::is_none")]` on those fields in `fleet_types::Receipt`. |
| wrong-type | Not reachable inside this crate — `event: ReceiptEvent` is an enum the caller must have already parsed/constructed; there is no stringly-typed event parameter to misuse (this is the direct fix for `main.rs:4373-4386`'s runtime string whitelist). |
| huge | `body` containing megabytes of JSON → appended as-is; this crate imposes no size cap (today's fleet doesn't either). The append is `O(chain length)` because it re-reads and re-verifies every prior row first (`main.rs:4365-4367`) — a multi-million-row chain makes every append proportionally slower; this is inherited behavior, not a regression, and is called out as a known cost, not silently hidden. |
| negative | n/a — no signed numeric input; `seq` is derived (`u64`), never supplied. |
| duplicate | Calling `append` twice with identical `(event, body, actor, ...)` produces two distinct rows with different `seq`/`prev_hash`/`hash`/`ts_wall` — receipts are not deduplicated by content, matching the ledger's append-only, always-accepting semantics. |
| concurrent | Two processes calling `append` against the same `LedgerPaths` serialize through the exclusive file lock (`FileLock::acquire`) — the second blocks until the first's guard drops; no torn or interleaved writes are possible. Two `Ledger` values (in-process, same paths) behave identically since the lock is filesystem-level, not in-memory. |
| unicode / non-ASCII | `actor`/`body` containing unicode text → written through untouched; JSON's own escaping handles it, and blake3 hashes the UTF-8 bytes of the canonical serialization exactly as produced, so verification is unaffected by any particular unicode content. |
| already-exists | n/a — every append is a new row; there is no identity to collide on. |
| partial-failure | If the process is killed between `write_all` and `sync_data` (`main.rs:4423-4425`), the next `append` or `verify` call re-reads the chain and either sees a complete last line (safe) or an incomplete last line (`read_rows`'s `serde_json::from_str` fails that line → surfaces as `LedgerError::MalformedRow`, forcing manual truncation rather than silently accepting a half-written row) — this crate does not attempt automatic recovery of a torn last line. |

### `fn Ledger::verify(&self) -> Result<VerifiedChain, LedgerError>`

| Input dimension | Behavior |
|---|---|
| empty | Zero rows → `Err(LedgerError::Empty)` (mirrors `ledger_rows(false)`'s `EXIT_INVARIANT`, `main.rs:4335`) — an uninitialized ledger is never reported as "verified, 0/0". |
| null / `None` | n/a — no optional input; `verify` takes no arguments beyond `&self`. |
| wrong-type | A row whose top-level JSON value isn't an object → `MalformedRow` at that row's seq, matching `main.rs:4445-4448`'s `as_object()` check. |
| huge | A chain of millions of rows → `verify` is `O(n)`, one pass, no unbounded in-memory duplication beyond the `Vec<Receipt>` already loaded by `rows()`. |
| negative | n/a — no numeric input. |
| duplicate | Two rows both claiming the same `prev_hash` → `Fork` at the second one, via the `seen_prev` set (`main.rs:4473-4478`) — this is the exact tamper class the chain design exists to catch. |
| concurrent | `verify` takes the same file lock as `append` — a `verify` running concurrently with an `append` sees either the pre- or post-append chain, never a torn mid-write read. |
| unicode / non-ASCII | Same as `append` — hashing operates on raw UTF-8 bytes; unicode content changes the hash exactly as any other content change would, no special-casing. |
| already-exists | n/a — verify is read-only and idempotent; calling it any number of times never mutates the chain. |
| partial-failure | A chain that verifies rows 0..k successfully but fails at row k+1 → returns `Err` naming row k+1 specifically (first-failure-wins, `main.rs:4454-4497`'s ordering: seq check, then prev-hash, then fork, then content-hash, in that order per row) — never a partial `Ok` for the rows that did pass. |

### `fn GraphStore::replace_project(&mut self, batch: ReindexBatch) -> Result<(), GraphError>`

| Input dimension | Behavior |
|---|---|
| empty | `batch.symbols` empty → still commits (this crate does not enforce "no symbols" as an error; `index_command`'s `if symbols.is_empty() { EXIT_INVARIANT }` check at `graph.rs:376-379` is call-site policy that stays with `fleet-context`, not storage). `batch.files`/`edges`/`aliases` empty → same, each table simply ends up empty for this project. |
| null / `None` | n/a — no optional fields; every `ReindexBatch` field is a required `Vec`/struct. |
| wrong-type | n/a — every field is already a typed Rust struct/enum by the time it reaches this fn; no stringly-typed or `Value` input crosses this boundary. |
| huge | Tens of thousands of symbols/edges → each row is one `INSERT` inside the single transaction (matching today's per-row loop, `graph.rs:422-453`); no batching/chunking is added in this version — flagged as a follow-up if profiling shows it matters, not assumed away here. |
| negative | n/a — `arity`/`line` are `u64`; a caller cannot construct a negative one. |
| duplicate | `INSERT OR IGNORE INTO edges` (`graph.rs:449`) → a duplicate `(caller_id, callee_id)` pair silently keeps the first insert, matching today's dedup-by-primary-key behavior exactly. A duplicate `symbol_id` (which the primary key forbids) → `GraphError::Sql`, since that would mean the caller's own `assign_symbol_ids` produced a collision — a `fleet-context` bug, correctly surfaced here rather than silently overwritten. |
| concurrent | Two `replace_project` calls against the same open `Connection`/project would race at the SQL layer; this crate's contract is one `GraphStore` per writer (documented in §11) — concurrent writers must coordinate externally (e.g. one indexing process at a time), same as today's CLI-invoked `index_command`. |
| unicode / non-ASCII | Symbol names / paths containing unicode → stored and compared as sqlite TEXT (UTF-8) with no special-casing, same as today. |
| already-exists | Re-indexing an already-known `root_path` → the `ON CONFLICT(root_path) DO UPDATE` (`graph.rs:395-397`) updates the existing project row in place rather than erroring or duplicating; this is the intended "re-index" path, not an error case. |
| partial-failure | Any single `INSERT`/`DELETE` failing mid-batch → the whole transaction rolls back (rusqlite's `Drop` behavior on an uncommitted `Transaction`), leaving the project's prior rows completely untouched — never a half-replaced project. |

### `fn MemoryStore::bm25_search` / `fn MemoryStore::vector_search`

| Input dimension | Behavior |
|---|---|
| empty | `terms: &[]` → `bm25_search` returns `Ok(vec![])` without issuing a `MATCH` query (an empty FTS5 `MATCH` string is invalid SQL, not "match everything" — mirrors `memory_store.py:327-329`'s early return). `query: &[]` (zero-length vector) → `DimensionMismatch` from the caller's own construction of that slice against `vector_dimensions`, surfaced the same way `set_embedding` would. |
| null / `None` | n/a — no `Option` parameters; both take required slices. |
| wrong-type | n/a — both take already-typed `&[String]`/`&[f32]`; no JSON/string parsing happens inside these two fns. |
| huge | `limit` far larger than the corpus → returns every matching row, capped at `limit` if that's smaller than available rows, never an error for "not enough rows" (matches SQL `LIMIT`'s natural behavior). |
| negative | n/a — `limit: u32` cannot be negative. |
| duplicate | Duplicate terms in `terms` (e.g. `["foo", "foo"]`) → the `OR`-joined MATCH query naturally dedupes at the FTS5 level (matching a term twice doesn't double-count); `bm25_search` does not pre-dedup its input, matching today's `terms()` which already dedupes before this point (`memory_store.py:316-323`) — that dedup responsibility stays with the caller. |
| concurrent | Read-only queries against a WAL-mode sqlite connection — safe to run concurrently with each other; concurrent with a `set_embedding`/`upsert` write is safe per sqlite's own WAL concurrent-reader guarantee, not something this crate adds locking for. |
| unicode / non-ASCII | FTS5's `porter unicode61` tokenizer (already configured at `open`, `memory_store.py:69`) handles unicode text; a term that doesn't tokenize to anything (e.g. pure punctuation) simply matches nothing, not an error. |
| already-exists | n/a — pure reads, no identity to collide on. |
| partial-failure | A malformed `MATCH` string (should be unreachable given `terms` are plain alphanumeric tokens, but if it happened) → surfaces as `MemoryError::Sql`, not a panic. |

### `fn KvStore::get` / `fn KvStore::put`

| Input dimension | Behavior |
|---|---|
| empty | `key: &[]` → a valid (if unusual) zero-length key, accepted by redb same as any other byte string; `value: &[]` on `put` → stores an empty value distinct from "absent" (`get` returns `Some(&[])`, not `None`). |
| null / `None` | `get` on an absent key → `Ok(None)`, never an error — absence is a normal outcome, not a fault. |
| wrong-type | n/a — both are opaque `&[u8]`; this crate never interprets the bytes. |
| huge | A multi-megabyte value → stored as-is; redb has no crate-imposed size cap here (its own page-size mechanics apply, not re-implemented by this crate). |
| negative | n/a — no numeric input. |
| duplicate | `put` on an existing key → overwrites the value (last-write-wins within one table), matching ordinary KV-store semantics; no history is kept. |
| concurrent | redb's own MVCC gives concurrent readers a consistent snapshot even during a concurrent writer; two concurrent `put`s to the same key serialize through redb's single-writer transaction model — never a torn value. |
| unicode / non-ASCII | n/a at this layer — keys/values are bytes, not strings; a caller storing UTF-8 text simply gets those bytes back unchanged. |
| already-exists | Same as "duplicate" above — `put` is an unconditional upsert, not a create-only operation; a caller wanting create-only compares `get`'s result first (no separate "insert-if-absent" API in this version). |
| partial-failure | A crash mid-`put` → redb's own crash-safety (it's a transactional, ACID engine) guarantees the table reflects either the pre- or post-write state on next open, never a torn write — this crate adds no additional guarantee beyond what redb already provides. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `rusqlite` | `0.32` with `features = ["bundled", "load_extension"]` (matches `fleet/keel/fleet/Cargo.toml:12`'s existing `0.32`/`bundled`; `load_extension` is added to load `vec0` in-process, replacing the Python subprocess bridge) | Backs `GraphStore` and `MemoryStore`. |
| `blake3` | `1.8.7` (pinned to `fleet/keel/Cargo.lock:117`'s already-resolved version) | The ledger's hash-chain digest, byte-for-byte compatible with existing chain files. |
| `fs2` | `0.4.3` (matches `fleet/keel/Cargo.lock:475`) | The advisory exclusive file lock backing `Ledger::append`/`rows`/`verify`, ported verbatim from `main.rs`'s `FileLock`. |
| `serde` | `1.0.229` (matches lock) | `Receipt` (from `fleet-types`) and this crate's own error/record types where useful. |
| `serde_json` | `1.0.151` (matches lock) | Ledger row bodies remain `serde_json::Value`, matching `fleet_types::Receipt.body`'s type exactly. |
| `thiserror` | `2.0.20` (matches lock) | Every typed error enum in §3. |
| `fleet-types` | workspace path (`{ path = "../fleet-types" }`) | `Receipt`/`ReceiptEvent`/`PrevHash`/`Blake3Hash`/`SchemaV1`/`ExitCode`. |
| `redb` | `2` in `Cargo.toml` (resolves to `2.6.3` in the workspace `Cargo.lock`) | Pure-Rust, single-file, ACID, MVCC embedded KV with no C dependency — fits "embedded KV where it fits" without pulling a second SQL engine in for state that's genuinely just key/value; actively maintained (cberner/redb), MIT/Apache-2.0 dual-licensed. |

`tempfile` (`3.27.0`, matching lock) is a dev-dependency only — every test in §9 constructs its
stores against a `tempdir()` path, never the repo tree.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** `lib.rs` is a thin hub; each store gets its own
> module tree so no single file mixes schema, reads, and writes.

```
crates/fleet-store/
  Cargo.toml
  src/
    lib.rs                 # 23 — module decls + re-exports (IoFault + the 4 store types + retention)
    io_fault.rs             # 17 — IoFault enum
    lock.rs                 # 32 — FileLock (fs2-backed), ported from main.rs:4310-4328
    retention.rs             # 41 — RetentionPolicy/PruneReport/UsageReport/RetentionError (shared
                              #      vocabulary; each store below implements usage()/prune() against it)
    ledger/
      mod.rs                 # 24 — re-exports, Ledger struct + open()
      types.rs                # 40 — LedgerPaths, LedgerError (incl. LedgerRefused), VerifiedChain
      canon.rs                  # 44 — canonical_bytes(): the exact byte form append/verify hash
      clock.rs                   # 27 — now_rfc3339(): the one clock read, no chrono/libc
      read.rs                     # 46 — rows(), read_rows() helper
      append.rs                    # 77 — append(): verify-then-extend, blake3 hash, fs write
      verify.rs                     # 70 — verify(): the chain walk + per-row checks
      usage.rs                       # 32 — usage() (row count + byte size) and prune() (always refuses)
    graph/
      mod.rs                  # 17
      types.rs                 # 67 — ProjectRecord/FileRecord/SymbolRecord/EdgeRecord/AliasRecord/GraphError
      batch.rs                  # 24 — ReindexBatch, DependentRecord
      schema.rs                  # 51 — open(): connection + CREATE TABLE/INDEX
      read.rs                     # 79 — project(), symbols(), symbol_ids_for()
      write.rs                     # 69 — replace_project() transaction
      dependents.rs                 # 55 — the recursive-CTE query
      prune.rs                       # 71 — usage()/prune(): whole-project deletion, no age column
    memory/
      mod.rs                   # 18
      types.rs                  # 71 — MemoryRow/Severity/Bm25Hit/VectorHit/MemoryError
      schema.rs                  # 68 — open(): pragmas, CREATE TABLE/FTS5/triggers, load vec0
      write.rs                    # 64 — upsert(), set_embedding(), embedding_hashes()
      search.rs                    # 65 — bm25_search(), vector_search(), count()
      prune.rs                      # 58 — usage()/prune(): age/row/byte-bounded row deletion
      prune_rows.rs                  # 58 — row lookup/delete helpers used by prune.rs
      prune_delete.rs                 # 42 — cascades a memories-row delete to FTS5/vector/vector-meta
    kv/
      mod.rs                   # 9
      types.rs                  # 47 — KvError (+ From impls for redb's five error types)
      store.rs                   # 72 — open()/get()/put()/delete()/list_prefix() via redb
      prune.rs                    # 71 — usage()/prune(): row/byte-bounded entry deletion, no age column
  tests/
    ledger_chain.rs            # 60 — append/rows happy path, seq/hash/prev_hash stamping
    ledger_tamper.rs             # 66 — reorder/fork/content-mismatch each caught with the right variant
    ledger_prune.rs                # 38 — prune() always returns LedgerRefused
    graph_reindex.rs                 # 71 — replace_project idempotence, rollback-on-error
    graph_dependents.rs                # 57 — recursive closure at depth 1/2/N, depth=0 rejected
    graph_prune.rs                       # 32 — whole-project deletion under max_rows/max_bytes
    memory_search.rs                       # 66 — upsert/bm25_search/vector_search, dimension mismatch
    memory_prune.rs                          # 57 — age/row/byte-bounded pruning, FTS5/vector cascade
    kv_roundtrip.rs                            # 35 — put/get/delete/list_prefix, absent-key semantics
    kv_prune.rs                                  # 37 — row/byte-bounded entry pruning
    support/mod.rs                                 # 68 — shared test fixtures (tempdir stores, etc.)
```
> If any file above still projects over 80 lines once bodies land, split again (e.g. `verify.rs`
> → `verify_chain.rs` + `verify_row.rs`). The file-size gate (§10) is run before Opus review.
> Line counts above are the real file lengths (`wc -l`) as built, not the pre-build estimates this
> section originally carried.

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-store"
version = "0.1.0"
edition = "2021"

[dependencies]
rusqlite = { version = "0.32", features = ["bundled", "load_extension"] }
blake3 = "1.8"
fs2 = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
redb = "2"
fleet-types = { path = "../fleet-types" }

[dev-dependencies]
tempfile = "3.27"
```

## 9. Test plan

**Unit tests** (per module, `#[cfg(test)]`):
- `append_stamps_seq_prev_hash_and_recomputable_hash` — three sequential appends against a fresh
  tempdir ledger; assert `seq == 0,1,2`, `prev_hash` chains correctly, and recomputing
  `blake3(prev_hash ++ canonical_bytes)` by hand equals the stored `hash`.
- `append_refuses_to_extend_an_already_broken_chain` — hand-corrupt row 0's `hash` on disk, then
  call `append`; assert `Err(LedgerError::Tampered { seq: 0 })`, not a silently-appended row 1.
- `verify_empty_chain_is_an_error_not_a_vacuous_pass` — `verify()` on a freshly-opened, never-
  appended ledger returns `Err(LedgerError::Empty)`.
- `graph_replace_project_upserts_on_matching_root_path` — index the same `root_path` twice with
  different symbol sets; assert `project_id` is stable and `symbols()` reflects only the second
  batch (old rows gone).
- `graph_dependents_rejects_zero_depth` — `dependents(..., depth: 0)` → `Err(BadDepth(0))`.
- `memory_upsert_increments_confirmed_count_on_repeat` — `upsert` the same `id` twice with
  different `body`; assert `confirmed_count == 2` and the stored `body` matches the second call.
- `memory_set_embedding_rejects_dimension_mismatch` — `open(..., 384)` then
  `set_embedding(rowid, &[0.0; 128], "hash")` → `Err(DimensionMismatch { expected: 384, found: 128 })`.
- `kv_get_on_absent_key_is_ok_none_not_error` — `get` on a never-`put` key returns `Ok(None)`.

**Integration tests** (`tests/`, calling only the public API):
- `ledger_chain.rs::full_chain_survives_process_restart` — append 5 receipts, drop and reopen a
  new `Ledger` against the same paths, `verify()` succeeds with `checked == total == 5`.
- `ledger_tamper.rs::each_tamper_class_is_caught_with_the_right_variant` — for reordered rows,
  forked `prev_hash`, and a hand-edited `body` post-write, assert `verify()` returns
  `SeqMismatch`/`Fork`/`Tampered` respectively — never a generic catch-all error.
- `graph_reindex.rs::a_failing_insert_rolls_back_the_whole_batch` — construct a `ReindexBatch`
  with a duplicate `symbol_id` (primary-key violation) among otherwise-valid rows; assert
  `replace_project` returns `Err` AND the project's prior (pre-call) rows are still fully intact.
- `graph_dependents.rs::recursive_closure_stops_at_depth` — a 4-hop call chain, query with
  `depth: 2`, assert only the first 2 hops' callers are returned, with correct `MIN(depth)` per row.
- `memory_search.rs::bm25_and_vector_return_independently_ranked_unfused_lists` — insert rows
  where BM25 and vector search would each independently rank differently; assert `bm25_search`
  and `vector_search` return their own orderings and neither list carries the other's ranking
  signal (no `rrf_score`-shaped field exists on either `Bm25Hit`/`VectorHit` — a type-level check
  as much as a behavioral one).
- `kv_roundtrip.rs::list_prefix_returns_only_matching_keys_across_tables` — put keys under two
  different table names sharing a byte-prefix; assert `list_prefix` for one table never returns
  the other table's rows.

**Mutation-testing targets** (`cargo mutants -p fleet-store`):
- Flipping `seq != expected_seq` to `seq == expected_seq` (or removing the check) in `verify` must
  be killed by `ledger_tamper.rs`'s reorder case.
- Flipping the `seen_prev.insert(...)` fork check's negation must be killed by the forked-`prev_hash`
  case in the same test.
- Deleting the `sync_data()`/lock-release ordering in `append` (or the chain-verify-before-append
  step) must be killed by `append_refuses_to_extend_an_already_broken_chain`.
- Changing `replace_project`'s transaction to auto-commit-per-statement (defeating atomicity) must
  be killed by `a_failing_insert_rolls_back_the_whole_batch`.
- Flipping `closure.depth < ?` to `<=` in the recursive CTE must be killed by
  `recursive_closure_stops_at_depth`'s exact-boundary assertion.
- Deleting the `DimensionMismatch` check in `set_embedding` must be killed by
  `memory_set_embedding_rejects_dimension_mismatch`.

**Property tests** (`proptest`, recommended for the ledger given how much of this crate's value is
"the chain never lies"):
- *Any prefix of a valid chain is itself a valid chain*: for randomly generated sequences of
  `(event, body, actor)` appended in order, `verify()` on every prefix length `0..=n` succeeds (for
  `n >= 1`) or is the documented `Empty` case (`n == 0`) — never a spurious failure on an untampered
  prefix. Minimum 50 cases, random append counts 1-30.

## 10. Verification recipe

```bash
cd crates/fleet-store
cargo test -p fleet-store --all-targets
cargo clippy -p fleet-store --all-targets -- -D warnings
cargo mutants -p fleet-store
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration tests pass, 0 skipped — publish as `<passed>/<total>` (e.g.
`24/24`, not "tests pass"). Clippy: 0 warnings. Mutants: every target named in §9 caught; publish
`<caught>/<total mutants>` — given the ledger's tamper-detection is this crate's core trust
property, the floor is **100% of viable mutants caught** on `ledger/verify.rs` and
`ledger/append.rs` specifically; the graph/memory/kv modules may carry a slightly lower floor if a
genuinely equivalent mutant is found and documented, never lowered by default.

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`LedgerError`/`GraphError`/`MemoryError`/
      `KvError`) — none swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code.
- [ ] Clock/RNG/IO are injected: every store's path(s) come from the caller at construction; the
      one clock read (`Ledger::append`'s `ts_wall`) is documented in §4, not hidden; no RNG anywhere
      in this crate (project ids, if fresh, are the caller's `Uuid::new_v4`, not this crate's — §5).
- [ ] Thread-safety documented: `Ledger` — safe to share across threads/processes, serialized by
      the OS-level `FileLock`, not an in-memory mutex. `GraphStore`/`MemoryStore` — one writer per
      open `Connection` is this crate's contract (§6); concurrent readers are safe via sqlite's own
      WAL mode. `KvStore` — safe for concurrent readers/writers via redb's own MVCC transactions.
- [ ] No float used for money, tokens, or any precision-sensitive count — the one float use
      (`&[f32]` embedding vectors) is a continuous embedding coordinate, not a count, and is
      documented as the deliberate exception in §4.
- [ ] No self-grading: verification runs `cargo mutants`, not just this crate's own unit tests;
      denominator published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10, template) — restate
      the real numbers in the PR once the crate is built (mark done then).
- [ ] Tests that touch the filesystem write only under `tempdir()`/`TempDir` — every test in §9
      constructs its `Ledger`/`GraphStore`/`MemoryStore`/`KvStore` against a fresh tempdir path,
      never the repo tree or `$HOME`.
- [ ] Every non-goal in §2 is actually absent from the code: no `fastembed`/model call, no RRF
      fusion math, no tree-sitter/`git` shell-out, no `Command::new`/subprocess spawn anywhere in
      `crates/fleet-store/src/` — enforce with `grep -rn 'Command::new\|fastembed\|tree_sitter' crates/fleet-store/src/` returning nothing.
- [ ] No source file exceeds 80 lines — §8's tree is the target; verified by the §10 `wc -l ...
      awk '$1>80'` gate before Opus review.

## 12. Definition of Done

`fleet-store` is DONE when: §10's four commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught, file-size gate silent) run from `crates/fleet-store/`;
every unchecked box in §11 is checked with its real numbers; a round-trip smoke test confirms an
existing production `chain.jsonl` file (copied into a tempdir, not the live one) produced by
today's `main.rs::append_receipt` still passes this crate's `Ledger::verify` byte-for-byte (proving
the hash computation truly is unchanged, not just unit-tested in isolation); `registry/services/
REGISTRY.md` lists the crate (infrastructure, per C1/L2); and Opus has re-derived the hash-chain
append/verify contract from this blueprint alone (without re-reading `main.rs`), reproduced the
fork-detection mutation by hand, and driven one real `append` → `verify` → tamper → `verify` cycle
end-to-end confirming the tamper is caught with the specific variant this blueprint claims.

---

## Divergence from MIGRATION-PLAN / for Opus

1. **`ReceiptEvent` (fleet-types) has no `Rollback` variant, but fleet's own ledger does.**
   `main.rs:4373-4386` and `:4561-4572`'s whitelist of storable events includes `"rollback"`
   alongside the 7 events `fleet-types`'s `BLUEPRINT.md` §3 enumerates on `ReceiptEvent`
   (`RunStart`/`ArtifactFrozen`/`Attested`/`Refusal`/`GateVerdict`/`RunEnd`/`LaneStatus`). This
   blueprint's `Ledger::append(event: ReceiptEvent, ...)` signature can only accept what
   `fleet-types` defines — as written today, this crate **cannot** append a rollback receipt, a
   real regression versus current fleet behavior. This needs an 8th `ReceiptEvent::Rollback`
   variant added to `fleet-types` (same crate, not invented locally here per the brief's "don't
   invent a local copy" rule) — flagging for Opus rather than silently adding it to this crate's
   own enum.

2. **MIGRATION-PLAN §3 row 2's evidence column reads as if `graph.rs` (rusqlite) is cited wholesale**,
   the same class of gap already caught for `route.rs` (row 4) and `roles.rs` (row 1, per §7's
   teach-back log). In fact only `graph.rs:612-694` (schema+reads) and the transactional write half
   of `index_command` (`graph.rs:390-484`) belong in `fleet-store`; the tree-sitter/git parsing
   majority of the file (`graph.rs:696-1172`, well over half the file) belongs to `fleet-context`
   (already correctly scoped there in row 7, but row 2's own wording doesn't say so). Worth a line
   in row 2 cross-referencing row 7, the same way row 4 now cross-references `fleet-govern`/`src/`.

3. **`memory_store.py`'s single `search()` function does three jobs MIGRATION-PLAN's row 2 doesn't
   separate**: raw BM25, raw vector KNN, and RRF fusion, plus a `benchmark()` harness. This
   blueprint follows the assignment brief's own explicit note ("this is the storage engine;
   fleet-memory §10 owns the scoring on top") and splits accordingly — but MIGRATION-PLAN's row 2
   text ("`memory_store.py` (sqlite-vec + FTS5 vector store)") reads as if the whole file including
   fusion moves here. Worth updating row 2 to name the split explicitly, the way this file does in
   §5's reuse-map rows for `search`/`ensure_embeddings`/`benchmark`.

4. **The Python `vector_bridge` subprocess-per-query pattern is deliberately not preserved.**
   `memory_store.py:197-221` spawns a whole Python interpreter per `count`/`sync`/`knn` call —
   acceptable overhead in a script invoked a few times per session, unacceptable per-query cost in
   a Rust store meant to serve interactive queries. This blueprint instead loads the same `vec0`
   extension artifact in-process via `rusqlite::Connection::load_extension`. This is a real
   behavior change (not just a language port) worth Opus explicitly signing off on, since it means
   `MemoryStore::open` now depends on the `vec0.{dylib,so,dll}` file existing at a path this crate
   can locate (today, only inside the `sqlite_vec` Python package's install directory,
   `memory_store.py:160-170`) — the composition root (`src/`) needs to supply or discover that
   path and pass it to `MemoryStore::open`, which is not yet reflected in any crate's `Imports`
   list.

5. **Does any of the three stores resist unification behind one trait? Yes — by design, not by
   oversight.** `Ledger` (sequential, single hash-chained file, append-only), `GraphStore`
   (relational, transactional replace-by-project), and `MemoryStore`/`KvStore` (row/blob CRUD +
   independent ranked retrieval) have almost no operations in common beyond "open a path" and
   "return a typed error" — forcing a single `trait Store { fn get(...) fn put(...) }` surface
   over them would either (a) collapse the ledger's append-only/hash-chain contract into a generic
   `put` that no longer expresses "you cannot overwrite row 3," or (b) collapse the graph's
   transactional multi-table replace into a shape that can't express "all four tables or none."
   This blueprint's actual unification is: one crate, one shared `IoFault`, one convention (typed
   errors, injected paths, `tempdir()`-only tests, ≤80-line files) — not one behavioral trait. If
   Opus wants a literal shared trait, the only honest one is something like
   `trait Store { type Error; fn open(path: &Path) -> Result<Self, Self::Error>; }` (construction
   only) — flagging this as the recommended resolution rather than silently picking it.
