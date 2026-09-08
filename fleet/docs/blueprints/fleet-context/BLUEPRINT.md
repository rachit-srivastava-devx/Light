# BLUEPRINT — `fleet-context`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-context`
- **One-line purpose:** Given a task description and a token budget, build a ranked repo map
  (tree-sitter symbols + call edges, scored by PageRank) and retrieve the most relevant slice of
  the codebase — hybrid BM25 + vector search, fused, then compacted to fit the budget with real
  token counts — with no persistence of its own and no LLM call of its own.
- **Build branch:** `partial+build` (MIGRATION-PLAN §3 row 7) — the tree-sitter parsing half of
  `fleet/keel/fleet/src/graph.rs` (1351 lines, read in full 2026-09-08) is a near-exact match for
  this crate's repo-map layer; everything else (BM25, vector embedding, rank fusion, PageRank, and
  the compactor) is absent in fleet today and must be built. **Caveat (read §5): `graph.rs` splits
  the same way `route.rs` did for `fleet-router` — this crate takes the pure tree-sitter parsing
  half; the rusqlite persistence half stays with `fleet-store` (MIGRATION-PLAN row 2, which already
  cites `graph.rs`). This is a file split, not a file move — see the divergence note at the end.**
- **Imports:** `fleet-types` (for `Tokens` — the token-budget/count type, so this crate does not
  invent a second one). **Deliberately not `fleet-store`** — see §2's non-goals and the divergence
  note: this crate defines the storage/vector-search seam as its own trait ports
  (`GraphStore`, `VectorIndex`) rather than taking a compile dependency on `fleet-store`'s
  (currently unwritten) concrete types. Whoever wires `src/` implements those traits against
  `fleet-store`; this keeps the two crates buildable in parallel and this blueprint self-contained.
- **Imported by:** `fleet-plan` (assembles a module brief and needs the relevant code slice to cite
  in it), `fleet-worker` (fetches context for a builder/verifier before dispatch), `src/`
  (composition root — `fleet context retrieve --task ... --budget ...` CLI command, and the
  indexing pipeline that feeds `build_repo_map`/`TantivyIndex::index`/`OrtEmbedder::embed_batch`
  results into `fleet-store`'s persistence).

## 2. Responsibility & non-goals

**Owns:** turning already-read source text into a symbol/call graph (tree-sitter, reused from
`graph.rs`), scoring each symbol's structural importance (PageRank over the call graph, greenfield),
building and querying a BM25 full-text index over a document corpus (tantivy, greenfield), defining
the seam for dense embeddings over that same corpus (the `VectorIndex` trait; **as built, no
`fastembed`/`ort` embedder ships in this pass** — see §7's note and the divergence note at the end
of this file — `retrieve_context` degrades to BM25-only via the `NoVectorIndex` no-op until a later
pass wires a real one), fusing ranked lists into one (reciprocal rank fusion, hand-rolled — see §5/§7
on why the crate literally named `rank-fusion` is not used), and compacting the fused, ranked
candidate set down to a hard token budget using real tokenizer counts (tiktoken-rs) — greedily
including what fits, and optionally asking an injected summarizer to shrink a candidate that would
otherwise be dropped. Given the same corpus, query, and budget, `retrieve_context` returns the same
`ContextSlice` every time. **As built**, the crate also owns a `conventions` capability (discovering
and folding a repo's `AGENTS.md`/`CLAUDE.md`/PR-template/`CONTRIBUTING.md` layering into this same
token-budget compactor) that is not part of this blueprint's original scope — see §8's file-layout
note.

**Non-goals (the seam):**
- Does **not** walk the filesystem, run `git`, or read files itself (today's `graph.rs`
  `scan_tree`/`visit_tree`/`git_commit`/`git_renames`, `graph.rs:696-765,1037-1126`) — the caller
  (the indexing orchestrator in `src/`, or `fleet-store`) reads files and passes this crate a
  `Vec<SourceFile>` already in memory. Rename-preserving symbol-identity tracking across commits is
  explicitly out of scope too (see §4's `SymbolId` note) — that needs persisted history, which is
  `fleet-store`'s job.
- Does **not** open a database connection, create a table, or write a row anywhere — the durable
  symbol graph and the durable vector store are `fleet-store`'s job (today's `graph.rs` rusqlite
  schema/queries, `graph.rs:612-694,1128-1247`, and `memory_store.py`'s sqlite-vec table). This
  crate's `GraphStore`/`VectorIndex` are trait **ports** a caller wires to `fleet-store`; this crate
  never constructs a concrete storage backend.
- Does **not** call an LLM, `llm-gateway`, or any network endpoint under any circumstance — the
  hierarchical-summarize step of compaction takes an injected `Summarizer` trait; the caller
  performs the actual model call (through `fleet-router`'s routing decision and `fleet-govern`'s
  cost metering, per C9/C12). This crate never links an HTTP client.
- Does **not** decide which model/adapter handles a task — `fleet-router`'s job. This crate answers
  "what code is relevant," never "who should look at it."
- Does **not** download or manage the ONNX embedding model file. `OrtEmbedder::load` takes a local
  `&Path` the caller already resolved; `fastembed`'s optional `hf-hub` network-download feature is
  explicitly **disabled** in this crate's `Cargo.toml` (§7) so no code path in this crate can reach
  the network, ever, even by a future contributor's oversight.
- Does **not** implement a SCIP protobuf ingestion pipeline in this pass. The task brief lists SCIP
  as an absent, to-be-built piece; having read `graph.rs` in full, the existing tree-sitter
  symbol+call extraction already produces a real (if coarser-grained) symbol graph across
  rust/python/bash, and building a genuine SCIP consumer (external per-language indexer
  invocation + protobuf parsing) is materially larger than this pass's budget. This blueprint
  instead defines `build_repo_map`'s symbol extraction behind a seam (§3's `SymbolSource`-shaped
  free function, not a trait, because there is exactly one implementation today) so a SCIP-backed
  second implementation can be added later without changing `RepoMap`'s shape. **Flagged explicitly
  for Opus** — see the divergence note.

## 3. Public API contract

```rust
//! Retrieval and compaction on top of a repo map. No persistence, no LLM calls, no filesystem
//! walk, no network I/O -- every fact about the world (source text, prior symbols, index storage,
//! vector storage, the summarizing model) arrives through a parameter or an injected trait that
//! the caller supplies.

use fleet_types::Tokens;
use std::path::Path;

// =====================================================================================
// A. Shared data types
// =====================================================================================

/// One already-read source file. The caller resolves the path and reads the bytes; this crate
/// never touches the filesystem.
pub struct SourceFile {
    /// Repo-relative, forward-slash-normalized path (matches `graph.rs`'s existing convention).
    pub path: String,
    pub language: Language,
    pub source: String,
}

/// The three languages this crate's tree-sitter grammars cover today. Mirrors `graph.rs:767-774`'s
/// `language_for`, typed instead of a bare `&'static str` match.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Language { Rust, Python, Bash }

/// A stable identifier for one symbol (function/definition), derived deterministically from
/// `(path, name, arity)` via blake3 -- NOT a random UUID (unlike `graph.rs:938-975`'s
/// `assign_symbol_ids`, which used `Uuid::new_v4()` per unmatched symbol because it had a
/// persisted prior-run table to diff against). This crate has no persisted prior run, so identity
/// must be a pure function of content-addressable facts, not "new random id unless a caller-
/// supplied history says otherwise." Two calls to `build_repo_map` over identical source therefore
/// always assign the same `SymbolId` to the same symbol -- required for `retrieve_context`'s
/// determinism claim (§2).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SymbolId(String);

impl SymbolId {
    /// `blake3(path || "\0" || name || "\0" || arity_as_decimal)`, hex-encoded. Two symbols with
    /// the same name+arity in the same file are genuinely ambiguous under this scheme -- see §6's
    /// "duplicate" row for `build_repo_map`.
    pub fn derive(path: &str, name: &str, arity: u64) -> Self { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}

/// One extracted symbol and the fact PageRank attaches to it.
#[derive(Clone, Debug)]
pub struct SymbolRef {
    pub id: SymbolId,
    pub path: String,
    pub name: String,
    pub arity: u64,
    pub line: u64,
}

/// The full parsed-and-scored repo map. `importance` has exactly one entry per `symbols` entry --
/// never fewer (an "unscored" symbol is an invariant violation, see §4).
#[derive(Clone, Debug)]
pub struct RepoMap {
    pub symbols: Vec<SymbolRef>,
    /// Caller -> callee edges, deduplicated (mirrors `graph.rs:1003-1034`'s `BTreeSet` edge set).
    pub edges: Vec<(SymbolId, SymbolId)>,
    /// PageRank score per symbol, always summing to `symbols.len() as f64` (mass-conserving; see
    /// §4) so scores are comparable across repo maps of different sizes only after the caller
    /// normalizes by `symbols.len()` if it needs to -- this crate does not hide that division.
    pub importance: std::collections::BTreeMap<SymbolId, f64>,
}

/// Every fallible operation in this crate returns one of these -- never `String`, never
/// `anyhow::Error`, never a bare `bool`/`Option` standing in for an error.
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("tree-sitter grammar failed to load for {0:?}")]
    GrammarLoad(Language),
    #[error("source for {path:?} contains a parse error tree-sitter could not recover from")]
    ParseError { path: String },
    #[error("tantivy index operation failed: {0}")]
    Bm25Index(String),
    #[error("embedding backend failed: {0}")]
    Embed(String),
    #[error("summarizer returned {actual} tokens, over the {requested} token target")]
    SummarizeOverBudget { requested: u32, actual: u32 },
    #[error("budget of {budget} tokens cannot fit even the single smallest candidate ({smallest} tokens)")]
    BudgetTooSmall { budget: u64, smallest: u64 },
}

// =====================================================================================
// B. Repo map -- reused tree-sitter parsing (graph.rs) + greenfield PageRank
// =====================================================================================

/// Parse every file, extract symbols + call edges, resolve ambiguous callees the same way
/// `graph.rs:977-1035`'s `resolve_edges` does (unique-name-and-arity match, else unique-name-only
/// match, else drop the edge -- never guess between two candidates), then run PageRank over the
/// resulting call graph. Deterministic: identical `files` always yields an identical `RepoMap`
/// (field order included) -- required because `retrieve_context` composes on top of this.
pub fn build_repo_map(files: &[SourceFile]) -> Result<RepoMap, ContextError> { unimplemented!() }

/// Pure PageRank over a caller -> callee edge list. `damping` is conventionally `0.85`; runs a
/// fixed `iterations` power-iteration (never "until convergence" -- an unbounded loop is not
/// something a pure fn should have) and returns a score per node reachable from `nodes`, including
/// zero-in-degree and zero-out-degree (dangling) nodes at the floor score, never omitted.
pub fn pagerank(
    nodes: &[SymbolId],
    edges: &[(SymbolId, SymbolId)],
    damping: f64,
    iterations: u32,
) -> std::collections::BTreeMap<SymbolId, f64> { unimplemented!() }

// =====================================================================================
// C. BM25 (tantivy) -- greenfield
// =====================================================================================

/// One document handed to the BM25 index: a symbol's id plus the text tantivy should tokenize
/// (typically the symbol's source span, caller-extracted).
pub struct IndexDoc<'a> { pub id: SymbolId, pub text: &'a str }

/// Where a `TantivyIndex` keeps its segment files. `Ram` never touches the filesystem (used by
/// this crate's own tests); `Path` is the real, caller-chosen on-disk location -- never guessed,
/// never a hardcoded tempdir, matching the injected-IO rule (§4).
pub enum IndexLocation<'a> { Ram, Path(&'a Path) }

pub struct TantivyIndex { /* private: tantivy::Index + schema field handles */ }

impl TantivyIndex {
    pub fn open(location: IndexLocation<'_>) -> Result<Self, ContextError> { unimplemented!() }
    /// Replaces the entire index content with `docs` (idempotent full rebuild -- this crate does
    /// not attempt incremental upsert in this pass; see the divergence note).
    pub fn index(&mut self, docs: &[IndexDoc<'_>]) -> Result<(), ContextError> { unimplemented!() }
    /// Returns up to `k` `(SymbolId, bm25_score)` pairs, descending by score, ties broken by
    /// `SymbolId` ordering (deterministic; tantivy's own tie order is not guaranteed stable).
    pub fn search(&self, query: &str, k: u32) -> Result<Vec<(SymbolId, f32)>, ContextError> { unimplemented!() }
}

// =====================================================================================
// D. Embeddings (fastembed/ort) -- greenfield
// =====================================================================================

pub struct OrtEmbedder { /* private: fastembed::TextEmbedding */ }

impl OrtEmbedder {
    /// Loads the ONNX model from `model_path` (already resolved and present on disk -- this fn
    /// never downloads). Fails typed if the file is missing/unreadable/not a valid ONNX graph.
    pub fn load(model_path: &Path) -> Result<Self, ContextError> { unimplemented!() }
    /// Embeds every text in `texts`, preserving order and count (`result.len() == texts.len()`
    /// always, or the whole call fails -- no partial-batch success). Empty input returns `Ok(vec![])`.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, ContextError> { unimplemented!() }
}

/// A `SymbolId`-keyed nearest-neighbor query surface. This crate never implements this trait
/// itself -- `fleet-store`'s sqlite-vec adapter does, wired in `src/` (see §1's Imports note).
/// This crate only calls `nearest`/`upsert`; it never opens a connection.
pub trait VectorIndex {
    fn upsert(&mut self, id: &SymbolId, vector: &[f32]) -> Result<(), ContextError>;
    /// Up to `k` `(SymbolId, cosine_similarity)` pairs, descending by similarity.
    fn nearest(&self, query_vector: &[f32], k: u32) -> Result<Vec<(SymbolId, f32)>, ContextError>;
}

// =====================================================================================
// E. Fusion -- hand-rolled reciprocal rank fusion (NOT the `rank-fusion` crate -- see §5/§7)
// =====================================================================================

/// Reciprocal Rank Fusion: `score(id) = sum over rankings containing id of 1 / (k + rank)`, where
/// `rank` is the 1-based position within that ranking. `k` defaults to `60.0` (the RRF literature's
/// standard constant; a caller wanting a different value passes it explicitly -- no hidden global).
/// A `SymbolId` absent from a ranking contributes `0` from that ranking, never a penalty. Ties in
/// the output are broken by `SymbolId` ordering, matching every other sort in this crate.
pub fn fuse_rrf(rankings: &[Vec<(SymbolId, f32)>], k: f64) -> Vec<(SymbolId, f64)> { unimplemented!() }

// =====================================================================================
// F. Compaction -- real token counts (tiktoken-rs) + greedy salience fit + optional summarize
// =====================================================================================

/// One candidate chunk of code text, already scored and ready to place into the budget.
pub struct ScoredChunk {
    pub id: SymbolId,
    pub path: String,
    pub text: String,
    /// The fused score from `fuse_rrf` (or a bare BM25/vector score if only one ran).
    pub score: f64,
}

/// A chunk actually placed into the returned context, and whether it was summarized to fit.
pub struct PlacedChunk { pub id: SymbolId, pub path: String, pub text: String, pub compacted: bool }

pub struct ContextSlice {
    pub chunks: Vec<PlacedChunk>,
    pub tokens_used: Tokens,
    pub tokens_budget: Tokens,
    /// Ids that were scored but did not make it in, in the order they were considered.
    pub dropped: Vec<SymbolId>,
}

/// The token model `count_tokens` counts against -- pins the tiktoken-rs encoding by name instead
/// of a magic string at every call site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenModel { Cl100kBase, O200kBase }

/// Real BPE token count for `text` under `model` (wraps `tiktoken-rs`; pure, deterministic, no IO).
pub fn count_tokens(text: &str, model: TokenModel) -> u32 { unimplemented!() }

/// Shrinks `text` toward `target_tokens`. The caller's implementation performs the actual model
/// call (through `fleet-router`/`fleet-govern`, per C9/C12) -- this crate only defines the
/// contract and never constructs an implementation of it.
pub trait Summarizer {
    fn summarize(&self, text: &str, target_tokens: u32) -> Result<String, ContextError>;
}

/// Greedily places `candidates` (assumed pre-sorted descending by `score`; `compact_to_budget`
/// does not re-sort, so an unsorted input silently degrades to first-fit-in-input-order, not an
/// error -- see §6) into `budget`, counting real tokens via `model`. Does **not** stop at the first
/// candidate that would overflow -- it keeps scanning the remaining, possibly-smaller candidates
/// (first-fit, not first-overflow-halts) so the tail of the budget is not wasted. When a candidate
/// would overflow and `summarizer` is `Some`, asks it to shrink that one candidate to the exact
/// remaining headroom; if the summarizer's own output still doesn't fit, the candidate is dropped
/// (never truncated blindly mid-token).
pub fn compact_to_budget(
    candidates: Vec<ScoredChunk>,
    budget: Tokens,
    model: TokenModel,
    summarizer: Option<&dyn Summarizer>,
) -> Result<ContextSlice, ContextError> { unimplemented!() }

// =====================================================================================
// G. Orchestration
// =====================================================================================

pub struct RetrievalQuery<'a> {
    pub task_text: &'a str,
    pub budget: Tokens,
    pub top_k_bm25: u32,
    pub top_k_vector: u32,
    pub rrf_k: f64,
    pub token_model: TokenModel,
}

/// The full pipeline: embed the query, run BM25 + vector search, fuse with `fuse_rrf`, look up
/// each fused id's text from `repo_map` (via `chunk_text`, caller-supplied because only the caller
/// knows how a symbol's line maps back to a source span), and compact to `query.budget`.
pub fn retrieve_context(
    query: &RetrievalQuery<'_>,
    repo_map: &RepoMap,
    bm25: &TantivyIndex,
    vectors: &dyn VectorIndex,
    embedder: &OrtEmbedder,
    chunk_text: &dyn Fn(&SymbolId) -> Option<String>,
    summarizer: Option<&dyn Summarizer>,
) -> Result<ContextSlice, ContextError> { unimplemented!() }
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `SymbolId` | Always `blake3(path\|\|name\|\|arity)`, never a random UUID, never caller-constructable from an arbitrary string. | Two calls over identical source assigning different ids to the same symbol — which would make `retrieve_context` non-deterministic and would silently break any cross-call cache keyed on `SymbolId`. |
| `RepoMap.importance` | Exactly one entry per `RepoMap.symbols` entry, values always sum to `symbols.len() as f64` (PageRank's mass-conservation property, checked by a unit test, §9). | A symbol present in the map with no importance score — a caller ranking by `importance[&id]` panicking or silently defaulting to zero for an arbitrarily-chosen subset of symbols. |
| `ContextSlice` | `tokens_used <= tokens_budget` always (enforced by `compact_to_budget`'s own accounting, never by a post-hoc caller check). | A "compacted" result that actually blew the budget — the one property every caller of this crate leans on. |
| `Tokens` (from `fleet-types`) | Reused, not redefined — `u64`, `checked_add`/`checked_sub`, never float. | A second, incompatible token-counting type existing in this crate that silently disagrees with `fleet-govern`'s cost metering (C12) about what a "token" is. |
| `ScoredChunk`/`PlacedChunk` | `text` is always the literal span handed in by `chunk_text`/the caller — this crate never truncates a chunk's `text` field itself; a chunk that doesn't fit is either summarized (via `Summarizer`, producing a *new*, shorter `text`) or dropped whole, never silently sliced mid-token. | A dropped-mid-sentence code snippet that looks complete but silently isn't — a correctness hazard for whatever reads `PlacedChunk.text` downstream (a diff reviewer, a builder prompt). |
| `IndexDoc`/`TantivyIndex` | `TantivyIndex::index` fully replaces prior content — there is no partially-indexed state observable from `search` (either the old full index answers, or the new full index answers, never a mix). | A caller searching mid-rebuild and silently getting some documents from the old generation and some from the new — a torn read. |

**Money/precision:** no money type in this crate. Token counts are `fleet_types::Tokens` (`u64`)
end-to-end; PageRank/BM25/cosine-similarity scores are `f64`/`f32` because they are *measurement
statistics* (a rank, not a currency or a count), the same carve-out `fleet-types`' blueprint
documents for Wilson-score rates — never used for anything that must not silently round.

**Clock/RNG/IO injection points:**
- `TantivyIndex::open`/`index`/`search` touch the filesystem **only** at the caller-supplied
  `IndexLocation::Path`, never an ambient tempdir; tests use `IndexLocation::Ram` and touch no disk.
- `OrtEmbedder::load` reads exactly one caller-supplied file path; `embed_batch` afterward is pure
  in-memory inference — no network call is reachable from this crate (`hf-hub` feature disabled,
  §7).
- `Summarizer`, `VectorIndex` are injected traits — this crate constructs neither; both are
  implemented by the caller.
- `build_repo_map`, `pagerank`, `fuse_rrf`, `count_tokens`, `compact_to_budget` are pure: no clock,
  no RNG, no IO. Every tie-break uses `SymbolId`/`String` ordering, never `HashMap` iteration order
  or a random draw.

## 5. Reuse map

Source read in full: `fleet/keel/fleet/src/graph.rs` (1351 lines, 2026-09-08). Fixtures reused
verbatim from `fleet/keel/tests/graph-fixtures/` (`known-callers.rs`, `false-positive.rs`,
`known-python.py`, `known-bash.sh`, `rename-before.rs`/`rename-after.rs`).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `graph.rs:767-774` (`language_for`) | `&Path -> Option<&'static str>` by extension. | yes, retyped | Returns `Option<Language>` (this crate's enum) instead of a bare `&'static str`; same three extensions (`rs`/`sh`,`bash`/`py`). |
| `graph.rs:797-824` (`parse_file`) | tree-sitter parser setup per language, calls `collect_nodes`, builds `ParsedFile`. | yes, logic | `source_digest`/uuid-per-symbol machinery dropped (no persisted prior run here, §3's `SymbolId::derive` replaces it); tree-sitter parse/error handling kept verbatim. |
| `graph.rs:826-861` (`collect_nodes`) | Recursive tree walk collecting `SymbolDef`/`CallRef`. | yes, verbatim | None — this is the crate's core extraction logic, unchanged. |
| `graph.rs:863-890` (`definition`) | Per-language "is this node a function definition" + arity. | yes, verbatim | None. |
| `graph.rs:892-931` (`call`) | Per-language "is this node a call" + callee name + arity, with the `if`/`for`/`while`/`case` false-positive filter. | yes, verbatim | None — this exact filter is what `false-positive.rs`'s fixture test (§9) guards. |
| `graph.rs:933-936` (`node_text`) | UTF-8 text extraction helper. | yes, verbatim | None. |
| `graph.rs:938-975` (`assign_symbol_ids`) | Reuses a symbol's id across runs via an old-symbols table + git-rename map; falls back to `Uuid::new_v4()`. | **no — reshaped** | This crate has no persisted "old symbols" table to diff against (that lives in `fleet-store`), so identity becomes a pure deterministic hash (`SymbolId::derive`) instead of a stateful uuid-or-reuse decision. Rename-preserving identity across commits is explicitly dropped — see §2's non-goals. |
| `graph.rs:977-1035` (`resolve_edges`) | Resolves each call's callee: exact `(name, arity)` match if unique, else unique-name-only match, else drop. | yes, logic verbatim | Operates on this crate's `SymbolId`/`SymbolRef` types instead of `graph.rs`'s private structs; same disambiguation heuristic, same "never guess between two candidates" rule. |
| `graph.rs:696-765` (`scan_tree`/`visit_tree`) | Filesystem walk + digest. | **no** | IO — caller's job (§2 non-goals); this crate takes `&[SourceFile]` already read. |
| `graph.rs:1037-1126` (`git_commit`/`git_renames`/`scan_relative`) | Subprocess `git` calls for commit id + rename detection. | **no** | Subprocess/IO — not this crate; rename-tracking is out of scope per §2. |
| `graph.rs:612-694` (`open_db`, `load_project`, `load_symbols`) | rusqlite schema + queries for the persisted symbol/edge graph. | **no — belongs to `fleet-store`** | MIGRATION-PLAN row 2 already cites `graph.rs` for `fleet-store`'s reuse; this is that half. Same file, two owners, exactly the `route.rs`/`fleet-router` precedent — flagged in the divergence note. |
| `graph.rs:1128-1247` (`check_freshness`, `target_symbol_ids`, `query_dependents`) | Staleness check + recursive-CTE dependents query. | **no** | Same as above — persistence-layer query logic, `fleet-store`'s job, not retrieval/ranking. |
| PageRank | n/a in fleet today. | greenfield | Standard power-iteration PageRank; see §3 B. No existing fleet code computes symbol importance. |
| BM25 index | n/a in fleet today (`memory_store.py` has BM25 in Python, not reusable from Rust). | greenfield | `tantivy` (§7). |
| Embeddings | n/a in fleet's Rust code (`memory_store.py` embeds via a Python library, not reusable). | greenfield | `fastembed`/`ort` (§7). |
| Reciprocal rank fusion | n/a. | greenfield, hand-rolled | The crate literally named `rank-fusion` (crates.io) exists but **all 17 of its published versions are yanked** (`max_version: 0.0.0`, withdrawn 2026-01-14 per crates.io's own API) — it cannot be depended on. RRF itself is ~15 lines of arithmetic (§3 E); hand-rolling it removes a dependency entirely rather than reaching for a broken one. |
| Compactor (token-budget fit) | n/a. | greenfield | `tiktoken-rs` for real counts (§7) + a first-fit-by-score placement loop (§3 F) — no existing fleet code does budget-aware compaction. |

## 6. Behavior spec

### `fn build_repo_map(files: &[SourceFile]) -> Result<RepoMap, ContextError>`

| Input dimension | Behavior |
|---|---|
| empty | `files: &[]` → `Ok(RepoMap { symbols: vec![], edges: vec![], importance: BTreeMap::new() })` — an empty repo is not an error, matching `pagerank`'s own empty-input case below. |
| null / `None` | n/a — `files` is a slice, never `Option`; a caller with no files passes `&[]` (the empty case above). |
| wrong-type | n/a — `Language` is a closed enum set by the caller from real extensions; no stringly-typed language name crosses this boundary. |
| huge | 100,000 files: this crate parses each independently (no cross-file state during parsing) so cost is `O(total source bytes)`, bounded only by tree-sitter's own parse cost; `resolve_edges`'s by-name/arity lookup is a `BTreeMap` build + lookup, `O(n log n)`, not quadratic. |
| negative | n/a — no numeric input. |
| duplicate | Two symbols sharing `(path, name, arity)` in the *same* file (e.g. two `#[cfg]`-gated definitions of the same function) → both real symbols hash to the *same* `SymbolId` by construction (§4) — this is a documented, deliberate collision, not a crash: `RepoMap.symbols` will contain two `SymbolRef` entries with equal `id`, and `importance` holds one score for that shared id (their call edges merge). A future caller needing to disambiguate them needs more identity than `(path, name, arity)` gives — flagged as a known limit, not silently "fixed" by inventing a counter suffix that would break determinism across re-parses in a different node order. |
| concurrent | Pure function over an owned `&[SourceFile]`, no shared mutable state — safe to call from multiple threads with different `files` slices; not internally parallelized in this pass (see §11 thread-safety). |
| unicode / non-ASCII | Non-ASCII identifiers (e.g. Cyrillic function names) are valid tree-sitter identifiers in Rust/Python and round-trip through `node_text`'s UTF-8 extraction unchanged; `SymbolId::derive` hashes the raw UTF-8 bytes, so two non-ASCII names that are byte-identical collide exactly like ASCII ones do, and no two distinct non-ASCII names ever collide (blake3 is not lossy). |
| already-exists | Calling `build_repo_map` twice on the same `files` is not an error — required to be idempotent (same `RepoMap`, field-for-field, both times; a determinism unit test asserts this, §9). |
| partial-failure | One file with a tree-sitter-unrecoverable parse error (`root.has_error()` analog) → the *whole* call returns `Err(ContextError::ParseError { path })` naming that file — this crate does not silently skip bad files and return a partial map (a caller wanting best-effort partial coverage pre-filters `files` itself before calling). |

### `fn pagerank(nodes: &[SymbolId], edges: &[(SymbolId, SymbolId)], damping: f64, iterations: u32) -> BTreeMap<SymbolId, f64>`

| Input dimension | Behavior |
|---|---|
| empty | `nodes: &[]` → `BTreeMap::new()`, regardless of `edges`/`iterations` (no node to score). |
| null / `None` | n/a — no `Option` params. |
| wrong-type | n/a — no stringly-typed input. |
| huge | 1,000,000 nodes: cost is `O(iterations * (nodes.len() + edges.len()))` — linear per iteration, bounded because `iterations` is a fixed, caller-chosen `u32`, never "loop until converged" (an unbounded loop is excluded by construction). |
| negative | `damping` outside `[0.0, 1.0)` (e.g. `-0.1` or `1.0`) is not rejected by a typed error in this pass — documented as a caller contract (`damping` is a probability, the caller must pass a sane value; a future version could add a `damping: Damping` newtype clamped at construction, flagged as a possible follow-up, not built now to keep this fn a plain, cheap-to-call primitive). |
| duplicate | An edge `(a, b)` appearing twice in `edges` is treated as one edge (this fn dedupes internally via a `BTreeSet` pass before iterating) — a caller passing a raw, possibly-duplicated edge list (e.g. from `RepoMap.edges`, which is itself already deduplicated at the source, §4) never gets an inflated score from a repeated edge. |
| concurrent | Pure fn, no shared state, `&`-only inputs — trivially safe from any number of threads. |
| unicode / non-ASCII | `SymbolId`'s ordering is byte-wise `String` ordering — a unicode symbol name orders correctly (if not necessarily "alphabetically" in a human sense), same non-normalizing behavior documented elsewhere in this file (§4/other crates' blueprints make the same call). |
| already-exists | Idempotent: identical `(nodes, edges, damping, iterations)` always yields a bit-identical `BTreeMap` (fixed iteration count, no randomness, no floating hash-map iteration order feeding the arithmetic — see §9's determinism test). |
| partial-failure | n/a — pure computation, cannot fail partially; always returns a complete map covering every node in `nodes` (dangling/isolated nodes included at the floor score, never omitted — §3's doc comment). |

### `fn fuse_rrf(rankings: &[Vec<(SymbolId, f32)>], k: f64) -> Vec<(SymbolId, f64)>`

| Input dimension | Behavior |
|---|---|
| empty | `rankings: &[]` → `vec![]`. A ranking that is itself empty (`vec![]` inside `rankings`) contributes nothing to any id's score — not an error. |
| null / `None` | n/a — no `Option` params. |
| wrong-type | n/a. |
| huge | 100 rankings of 10,000 ids each: cost is `O(total (id, rank) pairs)` building one `BTreeMap<SymbolId, f64>` accumulator, then one sort — no quadratic comparison across rankings. |
| negative | `k <= 0.0` is accepted (not rejected) but documented as producing degenerate/inflated scores (division by a small-or-negative denominator) — same "caller contract, not a typed error" call as `pagerank`'s `damping`, because RRF's `k` has no natural closed range the way a probability does; §9 includes a test pinning the conventional `k = 60.0` behavior, not a validated-range test. |
| duplicate | The same `SymbolId` appearing more than once *within a single ranking* (a caller bug — a ranking should list each id once) has its later occurrence's rank silently override the earlier one's contribution from that same ranking (last-write-wins into the same `BTreeMap` slot for that ranking's pass) rather than double-counting — documented, not treated as a hard error, since this fn cannot distinguish "caller bug" from "caller intentionally re-scored." |
| concurrent | Pure fn — trivially safe. |
| unicode / non-ASCII | Tie-break ordering is `SymbolId`'s byte-wise `String` ordering, same non-normalizing behavior as elsewhere in this file. |
| already-exists | Idempotent — identical inputs always yield an identical, identically-ordered output vector (ties broken deterministically, never by insertion/hash order). |
| partial-failure | n/a — pure, cannot fail partially. |

### `fn compact_to_budget(candidates: Vec<ScoredChunk>, budget: Tokens, model: TokenModel, summarizer: Option<&dyn Summarizer>) -> Result<ContextSlice, ContextError>`

| Input dimension | Behavior |
|---|---|
| empty | `candidates: vec![]` → `Ok(ContextSlice { chunks: vec![], tokens_used: Tokens::ZERO, tokens_budget: budget, dropped: vec![] })`. |
| null / `None` | `summarizer: None` → any candidate that doesn't fit is dropped outright (added to `dropped`), never summarized — this is a valid, common call shape (compaction without a model available), not an error. |
| wrong-type | n/a — no stringly-typed input; `TokenModel` is a closed enum. |
| huge | 100,000 candidates, `budget = Tokens::new(1)`: this fn still scans every candidate (first-fit, not first-overflow-halts, §3's doc comment) — cost is `O(candidates.len())` token-counts plus at most one `summarizer.summarize` call per candidate that almost fits; it terminates, does not loop unboundedly, and does not allocate more than one `PlacedChunk`/dropped-id per input candidate. |
| negative | n/a — `Tokens`/`u32` are unsigned throughout; a negative budget is not representable. |
| duplicate | Two `ScoredChunk`s sharing the same `id` are each considered independently and both may be placed (or one dropped) — this fn does not deduplicate by `id`; a caller wanting at-most-one-chunk-per-symbol must dedupe `candidates` itself before calling (documented contract, not silently enforced here, since a caller might legitimately want two spans from the same symbol, e.g. a doc comment span and a body span). |
| concurrent | Takes `candidates` by value (owned), `summarizer` by shared `&dyn` reference assumed side-effect-free from this fn's perspective (its `summarize` may itself do IO, but that IO is the caller's implementation's concern, not a shared-mutable-state hazard for this fn) — safe to call from independent threads with independent `candidates`. |
| unicode / non-ASCII | `count_tokens` counts real BPE tokens via `tiktoken-rs`, which is unicode-aware (a single emoji or CJK character can be multiple BPE tokens) — this fn never approximates by character or byte count, exactly because that would silently under- or over-count non-ASCII text against the real budget. |
| already-exists | n/a — no persisted state; calling twice with identical input is idempotent (deterministic placement order, since `candidates` is scanned in the order given, §3's doc comment on pre-sorted input). |
| partial-failure | `summarizer.summarize(...)` returning `Err` for one candidate → that one candidate is treated as "does not fit," added to `dropped`, and the loop continues to the next candidate — one summarizer failure never aborts the whole compaction. `summarizer.summarize(...)` returning `Ok(text)` whose own real token count (re-checked by this fn, never trusted blindly) still exceeds the requested `target_tokens` → `Err(ContextError::SummarizeOverBudget { .. })` propagates and aborts the whole call, since a summarizer that cannot honor its own contract is a caller-side defect this fn should not paper over by truncating its output. |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | path dep (workspace) | `Tokens` — the token-budget/count type, reused rather than redefined (§4). |
| `tree-sitter` | `0.24.7` (matches `fleet/keel/Cargo.lock`) | Already fleet's parser; `graph.rs`'s reused extraction logic depends on this exact API. |
| `tree-sitter-rust` | `0.23.3` (matches lock) | Rust grammar, reused. |
| `tree-sitter-python` | `0.23.6` (matches lock) | Python grammar, reused. |
| `tree-sitter-bash` | `0.23.3` (matches lock) | Bash grammar, reused. |
| `blake3` | `1.8.7` (matches lock) | `SymbolId::derive`'s deterministic hash — already a fleet dependency (`graph.rs`'s `digest_items`/`source_digest`), reused for the same reason (fast, well-audited, already in the lockfile). |
| `tantivy` | `0.26.1` (crates.io `max_stable_version`, 17.7M downloads — mature, actively released as of 2026-04) | BM25 full-text index (§3 C). New dependency — absent from fleet today. |
| `tiktoken-rs` | `0.12.0` (crates.io `max_stable_version`, 15.9M downloads) | Real BPE token counts for `count_tokens`/`compact_to_budget` — this crate's entire reason for existing is *not* approximating a budget by character count. New dependency. |
| `thiserror` | `2.0.20` (matches lock) | `ContextError` derive — every fallible op returns this, never `String`/`anyhow`. |
| `serde` | `1.0.229` (matches lock) | `ContextSlice`/`PlacedChunk` serialize for `src/`'s JSON output, same convention as `fleet-router`/`fleet-types`. |

**As built, `fastembed`/`ort` are NOT a dependency of this crate — the risk flagged below was
resolved by *not taking it*, not by shipping it.** `Cargo.toml` has no `fastembed`/`ort` line;
`embed.rs` defines only the `VectorIndex` trait port plus a `NoVectorIndex` no-op implementation
(`nearest` always returns empty), so `retrieve_context` degrades cleanly to BM25-only until a later
pass wires a real embedder and a `fleet-store`-backed `VectorIndex`. See `crates/fleet-context/src/
embed.rs`'s own doc comment and `lib.rs`'s "v1 scope" note, and the divergence note at the end of
this file (item 5) for the risk this decision resolves.

**Dev-dependencies, as built** (not listed in this section's original `Cargo.toml` sketch below):
`tempfile = "3.14"` (filesystem-touching tests use a real tempdir, per §11) and `proptest = "1.5"`
(the §9 property tests).

**Explicitly NOT a dependency:** the crate named `rank-fusion` on crates.io — all 17 published
versions are yanked (`max_version: 0.0.0`, withdrawn 2026-01-14 per crates.io's registry API,
queried 2026-09-08). RRF is hand-rolled instead (§3 E, §5) — see the divergence note.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.**

> **As built**, this tree diverges from the sketch below in three ways: (1) `parse/extract.rs` was
> split further, exactly as this section's own footnote anticipated, into `extract.rs` +
> `extract_call.rs` + `extract_definition.rs`; (2) `repomap.rs` similarly split into `repomap.rs` +
> `repomap_edges.rs` + `repomap_parse.rs`, and `bm25.rs` gained a sibling `bm25_search.rs`; (3)
> `embed.rs` ships only the `VectorIndex` trait + `NoVectorIndex` stub, not an `OrtEmbedder` (§7's
> note); and (4) a whole new `conventions/` module was added, out of this blueprint's original
> scope — discovering a repo's `AGENTS.md`/`CLAUDE.md` layering, PR/issue templates, and
> `CONTRIBUTING.md`, then folding them into this crate's token-budget compactor, IO injected via a
> `ConventionFs` port. This module's API is not specified anywhere in this blueprint; see
> `crates/fleet-context/src/conventions/mod.rs` for its real public shape. The list below is the
> real `find crates/fleet-context -name '*.rs'` output:

```
crates/fleet-context/
  Cargo.toml
  src/
    lib.rs               # module decls + re-exports only
    error.rs             # ContextError
    types.rs             # SourceFile, Language, SymbolId, SymbolRef, ScoredChunk, PlacedChunk, ContextSlice
    parse/
      mod.rs              # re-exports
      language.rs         # language_for (graph.rs:767-774, retyped)
      extract.rs          # shared extraction helpers (graph.rs:826-936, ported verbatim)
      extract_call.rs     # call-site extraction, split out of extract.rs
      extract_definition.rs # definition extraction, split out of extract.rs
      symbol_id.rs        # SymbolId::derive (blake3 hash, replaces graph.rs:938-975's uuid path)
    repomap.rs            # build_repo_map: orchestrates parse + resolve_edges + pagerank
    repomap_parse.rs      # per-file parse step, split out of repomap.rs
    repomap_edges.rs      # resolve_edges (graph.rs:977-1035, ported), split out of repomap.rs
    pagerank.rs           # pagerank() power iteration, greenfield
    bm25.rs               # TantivyIndex (open/index), IndexDoc, IndexLocation
    bm25_search.rs        # TantivyIndex::search, split out of bm25.rs
    embed.rs              # VectorIndex trait + NoVectorIndex no-op stub only -- no OrtEmbedder/fastembed (§7)
    fuse.rs               # fuse_rrf, hand-rolled RRF
    tokens.rs             # TokenModel, count_tokens (tiktoken-rs wrapper)
    compact.rs            # Summarizer trait, compact_to_budget
    retrieve.rs           # RetrievalQuery, retrieve_context orchestration
    conventions/           # NEW, out of this blueprint's original scope -- see the note above
      mod.rs                # re-exports
      types.rs              # ConventionDoc, ConventionSet, DocKind
      fs_port.rs             # ConventionFs trait (injected IO) + StdConventionFs
      discover.rs             # discover_conventions
      fold.rs                  # fold_conventions
      fold_types.rs             # ConventionFold, PlacedConventionDoc, TrimmedConventionDoc
      templates.rs               # PR/issue template handling
  tests/
    repomap_fixtures.rs         # reuses fleet/keel/tests/graph-fixtures/* (known-callers, false-positive, known-python, known-bash)
    fixtures/                   # known-callers.rs, false-positive.rs -- fixture data, not test code
    pagerank_properties.rs      # mass-conservation, dangling-node, determinism
    fuse_rrf.rs                 # RRF arithmetic, tie-break, duplicate-within-ranking
    compact_budget.rs           # first-fit-not-first-halt, budget-never-exceeded
    compact_budget_property.rs  # proptest: budget-monotonic placement (§9)
    compact_summarizer_overbudget.rs # summarizer fallback / SummarizeOverBudget rejection, split out of compact_budget.rs
    conventions_discover.rs     # discover_conventions/fold_conventions behavior-spec cases (NEW module, see above)
    retrieve_pipeline.rs        # end-to-end with fake TantivyIndex(Ram)/NoVectorIndex/fake Summarizer
```
> If any file above still projects over 80 lines once bodies land, split again (e.g. `extract.rs` →
> `extract_definition.rs` + `extract_call.rs`). The file-size gate (§10) runs before Opus review.

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-context"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-types = { path = "../fleet-types" }
tree-sitter = "0.24.7"
tree-sitter-rust = "0.23.3"
tree-sitter-python = "0.23.6"
tree-sitter-bash = "0.23.3"
blake3 = "1.8.7"
tantivy = "0.26.1"
tiktoken-rs = "0.12.0"
thiserror = "2.0.20"
serde = { version = "1.0.229", features = ["derive"] }

[dev-dependencies]
serde_json = "1.0.151"
tempfile = "3.14"
proptest = "1.5"
```
(as built — no `fastembed`/`ort` line; see §7's note above.)

## 9. Test plan

**Unit tests:**
- `symbol_id_is_deterministic_across_repeated_derivation` — `SymbolId::derive` called twice with
  identical `(path, name, arity)` yields equal ids; different `arity` yields different ids.
- `known_callers_fixture_returns_non_zero_edges` — ported from `graph.rs`'s own test, reusing
  `fleet/keel/tests/graph-fixtures/known-callers.rs` verbatim: `build_repo_map` over that one file
  produces at least one edge from `known_caller` to `callee`.
- `false_positive_control_has_zero_edges` — ported from `graph.rs`, reusing
  `false-positive.rs`: an unindexed external call never becomes a graph edge.
- `all_required_languages_produce_symbols` — ported from `graph.rs`, reusing `known-python.py` and
  `known-bash.sh`: each produces at least one `SymbolRef`.
- `pagerank_mass_is_conserved` — for a fixed small graph, `sum(pagerank(...).values())` equals
  `nodes.len() as f64` within floating-point epsilon, for at least 3 different `iterations` values
  (20/50/200) — the fixed-point property holds regardless of how many iterations run past
  convergence.
- `pagerank_is_deterministic_across_runs` — 10 repeated calls with identical input yield bit-
  identical output maps.
- `fuse_rrf_favors_items_ranked_highly_in_multiple_lists` — an id ranked #1 in one list and #3 in
  another outscores an id ranked #1 in only one list and absent from the other, at `k = 60.0`.
- `fuse_rrf_ties_break_by_symbol_id` — two ids with manufactured-equal fused scores sort in
  `SymbolId` order, not insertion order.
- `count_tokens_matches_known_cl100k_example` — a fixed string with a documented, hand-verified
  cl100k token count (from `tiktoken-rs`'s own published examples) asserts the exact count, not
  "roughly N."

**Integration tests:**
- `compact_never_exceeds_budget` — property-style: for 50 randomly-sized (but seeded, not
  ambient-random — see §11) candidate sets and budgets, `ContextSlice.tokens_used <=
  ContextSlice.tokens_budget` always holds.
- `compact_is_first_fit_not_first_halt` — a candidate list `[huge, small, medium]` where `huge`
  alone exceeds the budget but `small` fits: asserts `small` is placed even though it is scanned
  after the overflowing `huge` (the property this fn's doc comment explicitly claims).
- `compact_falls_back_to_drop_when_summarizer_absent` — `summarizer: None`, an overflowing
  candidate → appears in `dropped`, not in `chunks`.
- `compact_uses_summarizer_when_it_fits` — a fake `Summarizer` that shrinks text to exactly
  `target_tokens` → the candidate appears in `chunks` with `compacted: true`.
- `compact_rejects_a_summarizer_that_oversells_its_output` — a fake `Summarizer` returning text
  whose real token count exceeds `target_tokens` → `Err(SummarizeOverBudget)`.
- `retrieve_pipeline_end_to_end` — a small fixture repo (the graph-fixtures files), a fake
  `VectorIndex`/`Summarizer`, and a real `TantivyIndex::open(IndexLocation::Ram)`: asserts
  `retrieve_context` returns a non-empty `ContextSlice` whose `tokens_used <= budget` and whose
  `chunks` include the symbol most directly matching the query text.

**Mutation-testing targets (`cargo mutants -p fleet-context`):**
- Flipping `<=` to `<` in `compact_to_budget`'s running-total-vs-budget check must be killed by
  `compact_never_exceeds_budget`.
- Deleting the "keep scanning after an overflow" branch (making compaction stop at the first
  candidate that doesn't fit) must be killed by `compact_is_first_fit_not_first_halt`.
- Flipping RRF's `1.0 / (k + rank)` to `1.0 / (k * rank)` must be killed by
  `fuse_rrf_favors_items_ranked_highly_in_multiple_lists`.
- Deleting the dangling-node floor-score assignment in `pagerank` must be killed by
  `pagerank_mass_is_conserved` (the sum would no longer equal `nodes.len()`).
- Flipping `resolve_edges`'s uniqueness check (`ids.len() == 1`) to `>= 1` (silently picking an
  ambiguous candidate) must be killed by `false_positive_control_has_zero_edges` and a dedicated
  `ambiguous_callee_never_guessed` test with two same-named candidates.

**Property tests (`proptest`):**
- *RRF is monotonic in rank*: for any two ids in the same single ranking, the one with the better
  (lower) rank never scores lower after fusion, holding all other rankings fixed — N = 200 cases.
- *Compaction is budget-monotonic*: increasing `budget` (all else fixed) never decreases the number
  of chunks placed — N = 100 cases, generating candidate sets via a seeded RNG (never
  `thread_rng`/ambient randomness, per §11).

## 10. Verification recipe

```bash
cd crates/fleet-context
cargo test -p fleet-context --all-targets
cargo clippy -p fleet-context --all-targets -- -D warnings
cargo mutants -p fleet-context
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration + property tests pass, 0 skipped — publish as `<passed>/<total>`.
Clippy: 0 warnings. Mutants: publish `<caught>/<total mutants>` against the §9 target list — floor
is every named mutation-testing target caught; a survivor gets a new test, not a lowered floor.
`fastembed`/`ort`-dependent tests (`OrtEmbedder`) require a real ONNX model file on disk to run
meaningfully — those tests are `#[ignore]`d by default with a `--ignored` note in the test file
pointing at a fixture model path, and are NOT counted in the published denominator unless that
fixture model is committed or fetched by a documented, separate step (this crate itself never
fetches it, per §2's non-goals) — record which denominator (with or without the ignored tests) is
being published.

## 11. L8 checklist

- [ ] Every fallible path returns `ContextError` — none swallowed into `bool`/`Option`/`String`/
      `.unwrap()` in non-test code (mark done once the real crate exists and this is grepped for).
- [ ] Clock/RNG/IO are injected: `TantivyIndex`'s disk location and `OrtEmbedder`'s model path are
      both caller-supplied parameters, never ambient; `VectorIndex`/`Summarizer` are injected
      traits; property tests use a seeded RNG (`proptest`'s own, not `rand::thread_rng`) — no
      ambient randomness anywhere in this crate's own logic.
- [ ] Thread-safety documented: `build_repo_map`/`pagerank`/`fuse_rrf`/`count_tokens`/
      `compact_to_budget` take owned or `&`-borrowed inputs and return owned outputs with no
      interior mutability — safe to call concurrently with distinct inputs. `TantivyIndex`/
      `OrtEmbedder` wrap a library handle each; document their `Send`/`Sync` bounds explicitly
      once built (tantivy's `Index` is `Send + Sync`; `fastembed::TextEmbedding` is not
      guaranteed `Sync` across threads without checking its docs at build time — call this out,
      don't assume).
- [ ] No float used for money or token *budgets* — `Tokens` (`u64`) throughout for anything a
      caller must not silently round; PageRank/BM25/cosine/RRF scores are `f64`/`f32` measurement
      statistics, not currency, and are never compared for exact equality in non-test code.
- [ ] No self-grading — `cargo mutants` runs, denominator published, not just the unit suite.
- [ ] The verify command's pass/fail denominator is stated in this file (§10) and restated in the
      PR once built.
- [ ] Filesystem-touching tests use `tempdir()`/`TempDir` or `IndexLocation::Ram` — never the repo
      tree or `$HOME`; `OrtEmbedder` tests point at a fixture path under a tempdir or are `#[ignore]`d.
- [ ] Every non-goal in §2 is actually absent from the code: no `fs::read_dir`/`std::process::
      Command` (filesystem walk, git) anywhere in `crates/fleet-context/src/`; no `reqwest`/`hf-hub`/
      any HTTP client dependency; no `rusqlite` dependency (that's `fleet-store`'s). Enforceable via
      `grep -rn 'std::process::Command\|reqwest\|rusqlite' crates/fleet-context/src/` returning nothing.
- [ ] **No source file exceeds 80 lines** — §8's layout, verified by §10's `wc -l ... awk` gate.

## 12. Definition of Done

`fleet-context` is DONE when: §10's four commands all pass with a published denominator (tests
`N/N` — with the ignored-`OrtEmbedder`-test caveat stated explicitly, clippy clean, mutants `M/M`
caught against §9's named targets) run from `crates/fleet-context/`; every unchecked box in §11 is
checked with its real numbers; `registry/services/REGISTRY.md` lists the crate; and Opus has
re-derived the retrieval pipeline (repo map → BM25+vector → RRF fuse → budget compaction) from this
blueprint alone, reproduced the "first-fit-not-first-halt" mutation by hand, and driven one real
`retrieve_context` call end-to-end confirming `tokens_used <= tokens_budget` as claimed in §4 —
and has adjudicated the two flagged divergences below (the `graph.rs` split with `fleet-store`, and
whether `fleet-context` should take a real Cargo dependency on `fleet-store` once its blueprint
exists instead of the current trait-port-only design).

---

## Divergence from MIGRATION-PLAN (for Opus)

1. **`graph.rs` is claimed by two rows.** MIGRATION-PLAN row 2 (`fleet-store`) cites `graph.rs
   (rusqlite)`; row 7 (`fleet-context`) cites `graph.rs (tree-sitter rust/python/bash)`. Both
   citations are correct — they are two disjoint halves of the same 1351-line file, exactly the
   `route.rs`/`fleet-router` precedent (MIGRATION-PLAN §7's own teach-back entry for that crate).
   This blueprint takes the pure tree-sitter parsing + call-resolution half (`graph.rs:767-936,
   977-1035`); the rusqlite schema/queries/staleness-check half (`graph.rs:612-694,1128-1247`) and
   the filesystem-walk/git half (`graph.rs:696-765,1037-1126`) stay with whichever crate ends up
   owning indexing orchestration (`fleet-store` per row 2, or possibly `fleet-scan`/`src/` for the
   walk-and-git-diff half specifically — MIGRATION-PLAN doesn't currently say which). **Worth a
   line in MIGRATION-PLAN §3 rows 2 and 7 making the split explicit**, the same way row 4's
   divergence note asked for `route.rs`.

2. **This blueprint does not take a Cargo dependency on `fleet-store`**, even though the task brief
   framing ("vector storage is fleet-store's job — this crate does the retrieval/scoring on top")
   reads as implying one. Instead, `fleet-context` defines `VectorIndex` as its own trait port and
   never references a `fleet-store` type. Rationale: `fleet-store` has no `BLUEPRINT.md` yet (it
   is `todo` in MIGRATION-PLAN §5's status table) — hard-depending this blueprint on unwritten
   concrete types would make it non-self-contained, violating the brief's own "hand-codeable from
   this file alone" bar. The trait-port design also means `fleet-context` and `fleet-store` can be
   built in parallel by two different agents with zero compile-order coupling; the composition root
   (`src/`) wires a `fleet-store`-backed `impl VectorIndex` once both exist. **Flagged for Opus**:
   confirm this is the preferred shape, or direct a real `fleet-store` path dependency once that
   crate's blueprint lands.

3. **SCIP is deliberately not built in this pass.** The task brief lists it as absent-and-to-be-
   built; this blueprint substitutes the existing tree-sitter extraction (already proven across
   rust/python/bash in `graph.rs`, with real fixture tests) and defers a SCIP-backed symbol source
   as a later addition behind the same `build_repo_map` signature. This is a scope-narrowing call,
   not an oversight — flagged explicitly for Opus to confirm or override.

4. **`rank-fusion` (the crate) is dead.** All 17 published versions are yanked as of this writing
   (crates.io API, queried 2026-09-08: `max_version: 0.0.0`, withdrawn 2026-01-14). RRF is hand-
   rolled instead (§3 E) — flagged so a future contributor doesn't "helpfully" re-add the named
   crate as a dependency without checking its yank status first.

5. **`ort`'s pre-1.0 status is a real, named risk**, not a formality — see §7's dependency table
   entry. If Opus judges the native-binary/API-churn cost too high for this pass, the fallback
   (ship BM25 + repo map + compaction now, add `OrtEmbedder`/`VectorIndex` in a follow-up crate
   version) is named in §7 rather than silently decided here.
