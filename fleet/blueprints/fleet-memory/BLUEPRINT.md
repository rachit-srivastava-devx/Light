# BLUEPRINT — `fleet-memory`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-memory`
- **One-line purpose:** Own memory scoring (recency/importance/relevance fusion), dedup-on-write
  merge decisions, a Thompson-sampling model-selection bandit, and lesson→gate promotion — as pure,
  injectable-IO decision logic sitting on top of storage/search ports the caller supplies.
- **Build branch:** `partial+build` (MIGRATION-PLAN §3 row 10) — `memory.sh` + `memory_store.py`
  already implement hybrid BM25+vector recall and a durable SQLite store; this crate does **not**
  re-implement that storage/search engine (that's `fleet-store`'s unify target, MIGRATION-PLAN row
  2). What's absent and built fresh here: the episodic/semantic/procedural split, dedup-on-write
  (merge cos > τ instead of appending), a Thompson-sampling bandit over models (~50 LOC per the
  brief), and a lesson→gate promoter. See §5 for exactly what's reused vs. new.
- **Imports:** `fleet-types` (for `GateRefusal`, the generic `{code, message}` refusal shape
  `promote_lesson`'s gate output reuses instead of inventing a fourth refusal struct in the roster —
  see fleet-types §2's own words, "every other refusal site uses" this type).
- **Imported by:** `fleet-govern` (consumes `pick_arm` for adaptive model selection as a signal
  alongside `fleet-router`'s deterministic pipeline — the two are complementary, not competing:
  `fleet-router` answers "who is allowed and available," this crate's bandit answers "who has
  performed best so far among the allowed set"), `fleet-verify` (consumes `check_added_line`'s
  `GateRefusal` output as one input to a diff gate, replacing `memory-check.sh`'s bespoke grep
  logic), `src/` (composition root: wires concrete `fleet-store`/`fleet-context` adapters to this
  crate's five injected ports and calls `retrieve`/`write`/`pick_arm`/`promote_lesson`).
  > **Divergence flag (see end-of-file note):** MIGRATION-PLAN's DAG puts `store` above the
  > `{siblings}` layer, implying `fleet-memory` could compile-depend on `fleet-store`/`fleet-context`
  > directly. Neither crate has a written blueprint yet (`blueprints/fleet-store/` and
  > `blueprints/fleet-context/` are empty as of this writing). This blueprint therefore does NOT take
  > a compile dependency on either — it defines its own SPI traits (§3) as the seam, exactly as
  > `fleet-router`'s Opus-approved blueprint defined `RuntimeState` instead of depending on whatever
  > probes cooldowns. Opus should decide whether these traits later move to `fleet-types` as shared
  > SPI once `fleet-store`/`fleet-context` are blueprinted (they will very likely want the same
  > `LexicalSearch`/`VectorSearch` shape).

## 2. Responsibility & non-goals

**Owns:** the decision layer above memory storage — how a stored item's age/importance/match-
quality combine into one ranked `Score` (`score`); whether a newly-observed correction is a genuine
new memory or a near-duplicate that should merge into an existing row instead of bloating the store
(`write`'s dedup decision); which model/adapter arm to prefer next given each arm's observed
success/failure history, via Bayesian (Thompson-sampling) exploration rather than a fixed rule
(`pick_arm`); and whether a memory that has been confirmed enough times should become an
enforceable diff-pattern gate, plus whether one added source line actually violates a promoted
pattern (`promote_lesson` / `check_added_line`).

**Non-goals (the seam):**
- Does **not** run FTS5/BM25 or a vector index itself, and does **not** open a SQLite connection —
  that engine is `memory_store.py`'s today and `fleet-store`'s tomorrow (MIGRATION-PLAN row 2). This
  crate calls out through `LexicalSearch`/`VectorSearch`/`NearestNeighborLookup` traits (§3); it
  never touches a file path, a connection string, or `sqlite_vec`.
- Does **not** compute embeddings (no `fastembed`/`BAAI/bge-small-en-v1.5` model loading) — a
  `query_embedding: &Embedding` always arrives pre-computed from the caller (today `memory_vec.py`'s
  job, tomorrow `fleet-context`'s).
- Does **not** compile or evaluate regular expressions itself — `check_added_line` takes a
  `&dyn PatternMatcher` the caller supplies (today `memory-check.sh`'s `grep -E`, tomorrow
  `fleet-verify`'s regex engine of choice). This mirrors `memory-check.sh`'s own stated boundary:
  "It owns no supervision or gate orchestration."
- Does **not** append receipts or write to any ledger (`fleet-store`'s job) and does **not** decide
  *which* diff a gate runs against or when (`fleet-verify`'s job) — it only answers "does this one
  line match this one promoted pattern."
- Does **not** read a clock or an RNG ambiently — `Timestamp`/`RandomSource` are always caller-
  supplied (§4).
- Does **not** query mem0, Letta, or MegaMemory, or depend on any of the three in its decision path
  — see §7's note. If a future adapter wants to feed their output in as additional retrieval
  candidates, it does so by implementing this crate's `LexicalSearch`/`VectorSearch` ports from the
  outside; this crate never imports or links against any of the three.

## 3. Public API contract

```rust
//! Pure decision logic for fleet's memory subsystem: scoring, dedup, model-selection bandit, and
//! lesson promotion. Storage and search are ports (traits) this crate defines and the caller
//! implements against `fleet-store`/`fleet-context` — this crate opens no file, no socket, no
//! process, and reads no ambient clock or RNG. Every fact about "now" or "random" arrives as a
//! parameter.

use fleet_types::GateRefusal;
use std::collections::BTreeMap;

// =====================================================================================
// A. Identifiers & the episodic/semantic/procedural split
// =====================================================================================

/// Non-empty stable identifier for one memory row. Local to this crate (not `fleet-types`)
/// because, unlike `TaskId`/`NodeId`/`LaneId`, no crate outside `fleet-memory`/`fleet-store` needs
/// to name a memory by id today — revisit if a second consumer appears (same rule fleet-router
/// applied to `RoleCheck`).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct MemoryId(String);

/// A `MemoryId` was constructed from an empty or all-whitespace string.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("memory id must not be empty")]
pub struct EmptyMemoryId;

impl MemoryId {
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyMemoryId> { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}

/// The three-way split ABSENT from today's `memory_store.py` (every row there is one flat
/// `memories` table, `memory_store.py:52-64`). `Episodic` = a dated event ("build broke on
/// 2026-09-03"); `Semantic` = a durable fact/correction (today's only kind, `memory.sh remember`'s
/// entire payload); `Procedural` = a reusable how-to/recipe. Nothing downstream is told to
/// interpret this differently per kind yet (that's `fleet-govern`/`fleet-verify`'s future call) —
/// this crate only carries the tag through scoring/dedup/promotion so it is never lost.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind { Episodic, Semantic, Procedural }

// =====================================================================================
// B. Embeddings & similarity — fixed-dimension vectors, validated, never raw f32 slices
// =====================================================================================

/// A dense embedding vector. Non-empty by construction. Dimension is NOT hardcoded to 384 (today's
/// `memory_store.py:26`'s `VECTOR_DIMENSIONS`/`BAAI/bge-small-en-v1.5`) — a future model change
/// must not require a type change here; instead `cosine`/`nearest` compare two `Embedding`s and
/// return a typed error the moment lengths disagree (see `DimensionMismatch` below), rather than
/// silently truncating or panicking.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Embedding(Vec<f32>);

/// `Embedding::new` was given an empty vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("embedding must have at least one dimension")]
pub struct EmptyEmbedding;

/// Two embeddings being compared (`cosine`, a `NearestNeighborLookup` result) have different
/// lengths — never silently zero-padded or truncated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("embeddings have different dimensions: {a} vs {b}")]
pub struct DimensionMismatch { pub a: usize, pub b: usize }

/// Cosine similarity, clamped to `[-1.0, 1.0]` by construction (floating-point drift on
/// near-parallel vectors can otherwise yield `1.0000000002`, which would break a `> 1.0`-cannot-
/// happen assumption downstream).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct CosineSimilarity(f64);

impl Embedding {
    pub fn new(values: Vec<f32>) -> Result<Self, EmptyEmbedding> { unimplemented!() }
    pub fn dim(&self) -> usize { unimplemented!() }
    /// Pure dot-product-over-norms computation; never IO, never a model call.
    pub fn cosine(&self, other: &Embedding) -> Result<CosineSimilarity, DimensionMismatch> { unimplemented!() }
}

impl CosineSimilarity {
    pub fn get(self) -> f64 { self.0 }
}

// =====================================================================================
// C. The stored item & the caller-supplied clock
// =====================================================================================

/// Caller-supplied wall-clock reading, seconds since Unix epoch. This crate never calls
/// `SystemTime::now()` — every scoring/dedup call takes `now` as a parameter so tests can freeze it
/// and so this crate stays deterministic given identical inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Timestamp(u64);
impl Timestamp {
    pub fn from_unix_secs(secs: u64) -> Self { Timestamp(secs) }
    pub fn unix_secs(self) -> u64 { self.0 }
}

/// A `[0.0, 1.0]`-clamped importance rating. Never money, never a token count — a measurement
/// statistic like fleet-types' Wilson-score rates, so `f64` is the correct representation (see §4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Importance(f64);
/// `Importance::new` was given a value outside `[0.0, 1.0]` or non-finite (`NaN`/`inf`).
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
#[error("importance {0} is not finite and within [0.0, 1.0]")]
pub struct ImportanceOutOfRange(pub f64);
impl Importance {
    pub fn new(value: f64) -> Result<Self, ImportanceOutOfRange> { unimplemented!() }
    pub fn get(self) -> f64 { self.0 }
}

/// One durable memory row — the shape `write`/`score`/`promote_lesson` all operate over. Mirrors
/// `memory_store.py`'s `memories` table (`memory_store.py:52-64`) plus the kind split (§A) it
/// lacks; persistence itself stays `fleet-store`'s job (§2).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryItem {
    pub id: MemoryId,
    pub kind: MemoryKind,
    pub text: String,
    pub embedding: Embedding,
    pub importance: Importance,
    pub created_at: Timestamp,
    pub last_confirmed_at: Timestamp,
    /// Mirrors `memory_store.py:60`'s `confirmed_count`, incremented on `remember` of an existing
    /// id (`memory_store.py:418`'s `confirmed_count = memories.confirmed_count + 1`).
    pub confirmed_count: u32,
}

// =====================================================================================
// D. score — score(item, now) = alpha*recency + beta*importance + gamma*relevance
// =====================================================================================

/// A retrieval-time relevance signal for one item against one query, produced by `retrieve`'s
/// fusion of lexical + vector hits (§E) — never computed by `score` itself, since relevance is a
/// per-query fact, not a property of the stored item alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relevance(f64);
impl Relevance {
    pub fn new(value: f64) -> Result<Self, ImportanceOutOfRange> { unimplemented!() }
    pub fn get(self) -> f64 { self.0 }
}

/// The three fusion weights. Caller-supplied (a scoring-policy choice, not hardcoded here) so a
/// future tuning pass changes a config value, not this crate's code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoreWeights { pub alpha: f64, pub beta: f64, pub gamma: f64 }

/// The fused score. Not clamped to any range (a weighted sum of three `[0,1]` inputs with
/// caller-chosen weights can legitimately exceed 1.0 if weights don't sum to 1.0) — callers that
/// want a normalized score enforce `alpha + beta + gamma == 1.0` themselves; this fn does not
/// silently renormalize weights it wasn't asked to renormalize.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Score(f64);
impl Score { pub fn get(self) -> f64 { self.0 } }

/// Recency half-life, in seconds: `recency = 0.5 ^ (age_secs / RECENCY_HALF_LIFE_SECS)`, so an item
/// confirmed exactly one half-life ago scores recency `0.5`, one confirmed `now` scores `1.0`, and
/// recency asymptotically approaches (never reaches) `0.0` — it is never negative, so a corrupt
/// item with `last_confirmed_at` in the future (age negative) is clamped to recency `1.0`, treated
/// as "as fresh as possible," never as an error (§6).
pub const RECENCY_HALF_LIFE_SECS: u64 = 14 * 24 * 3600;

/// Pure, total, never panics. `score(item, now, relevance, weights)` = `weights.alpha * recency +
/// weights.beta * item.importance.get() + weights.gamma * relevance.get()`, where recency decays
/// exponentially from `item.last_confirmed_at` per `RECENCY_HALF_LIFE_SECS` above.
pub fn score(item: &MemoryItem, now: Timestamp, relevance: Relevance, weights: ScoreWeights) -> Score {
    unimplemented!()
}

// =====================================================================================
// E. retrieve — hybrid fusion over injected lexical/vector ports, never raw cosine alone
// =====================================================================================

/// One lexical (BM25/FTS5) hit, as `fleet-store` would return it (mirrors `bm25_search`'s row
/// shape, `memory_store.py:326-346`: `id`, `bm25_rank`).
#[derive(Clone, Debug)]
pub struct LexicalHit { pub id: MemoryId, pub bm25_rank: usize }

/// One vector (kNN) hit, as `fleet-context` would return it (mirrors `search`'s `vector_by_id`
/// shape, `memory_store.py:374-377`: rank + raw distance, converted here, not there).
#[derive(Clone, Debug)]
pub struct VectorHit { pub id: MemoryId, pub vector_rank: usize, pub distance: f64 }

/// The injected lexical-search port — `fleet-store`'s eventual implementation wraps its FTS5/BM25
/// query behind this trait; this crate never sees SQL.
pub trait LexicalSearch {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<LexicalHit>, RetrieveError>;
}
/// The injected vector-search port — `fleet-context`'s eventual implementation wraps its kNN index
/// behind this trait; this crate never sees `sqlite_vec` or an embedding model.
pub trait VectorSearch {
    fn knn(&self, query_embedding: &Embedding, limit: usize) -> Result<Vec<VectorHit>, RetrieveError>;
}

/// Either injected port failed (the underlying store/index error, opaque here — the caller's port
/// implementation is responsible for producing a more specific error type if it needs one; this
/// crate only needs to know "IO through this port failed").
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("memory retrieval port failed: {0}")]
pub struct RetrieveError(pub String);

/// One retrieved item, ranked and ready to hand to the caller.
#[derive(Clone, Debug)]
pub struct RetrievedItem { pub id: MemoryId, pub score: Score, pub relevance: Relevance }

/// Reciprocal-rank-fusion constant, matching `memory_store.py:27`'s `RRF_K = 60` — kept identical
/// so this crate's fused ranking is comparable to today's Python fusion during the migration.
pub const RRF_K: f64 = 60.0;

/// Fuse `lexical.search`/`vector.knn` hits via reciprocal-rank fusion into a `Relevance` per id
/// (mirrors `search`'s `rrf_score` formula, `memory_store.py:386`: `1/(RRF_K+bm25_rank) +
/// 1/(RRF_K+vector_rank)`, each term `0` if that port didn't return the id at all), then combines
/// each surviving id's stored `MemoryItem` (looked up by the caller-supplied `items` map — this fn
/// takes already-fetched items, it does not itself own a lookup-by-id store) with `score` (§D) and
/// returns the top `limit` by `Score` descending, ties broken by `MemoryId` ascending (deterministic).
pub fn retrieve(
    query: &str,
    query_embedding: &Embedding,
    items: &BTreeMap<MemoryId, MemoryItem>,
    now: Timestamp,
    limit: usize,
    weights: ScoreWeights,
    lexical: &dyn LexicalSearch,
    vector: &dyn VectorSearch,
) -> Result<Vec<RetrievedItem>, RetrieveError> {
    unimplemented!()
}

// =====================================================================================
// F. write — dedup-on-write: merge cos > tau, never blind-append
// =====================================================================================

/// A not-yet-persisted candidate memory, as the caller (e.g. `memory.sh remember`'s successor)
/// would construct it before deciding insert vs. merge.
#[derive(Clone, Debug)]
pub struct NewMemory { pub kind: MemoryKind, pub text: String, pub embedding: Embedding, pub importance: Importance }

/// The injected nearest-neighbor port `write` consults to find the closest existing item, if any,
/// before deciding. `fleet-store`'s eventual implementation wraps its vector index behind this.
pub trait NearestNeighborLookup {
    fn nearest(&self, embedding: &Embedding) -> Result<Option<(MemoryId, CosineSimilarity)>, RetrieveError>;
}

/// The dedup threshold tau: a `nearest` hit with similarity `>= tau` is a duplicate to merge, never
/// a coincidence to insert alongside. Caller-supplied (a tuning knob, §4), not hardcoded — the
/// brief's target improvement (existing corpus bloat shrinking materially once dedup lands) is a
/// property of choosing tau well, not of this crate picking one number forever.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DedupThreshold(f64);
impl DedupThreshold {
    pub fn new(value: f64) -> Result<Self, ImportanceOutOfRange> { unimplemented!() }
}

/// The pure outcome of a `write` call — persistence itself is the caller's job (§2); this crate
/// only decides which of the two shapes the caller should execute.
#[derive(Clone, Debug)]
pub enum WriteDecision {
    /// No existing item is within `tau` — insert `candidate` as a brand-new row.
    Insert(MemoryItem),
    /// An existing item at `into` is within `tau` — merge instead of appending: bump
    /// `confirmed_count`, refresh `last_confirmed_at` to `now`, and raise `importance` to the max
    /// of the two (never lower it — a corroborated fact is at least as important as either single
    /// observation), mirroring `remember`'s real `ON CONFLICT ... confirmed_count = ... + 1`
    /// (`memory_store.py:414-420`) but as a similarity-triggered merge instead of an id-triggered one.
    Merge { into: MemoryId, similarity: CosineSimilarity, new_importance: Importance, new_confirmed_count: u32 },
}

/// Pure and total. Looks up `existing.nearest(&candidate.embedding)`; if it returns `Some((id, sim))`
/// with `sim >= tau`, returns `Merge`; otherwise (`None`, or `Some` below `tau`) returns `Insert` of
/// a freshly-minted `MemoryItem` (`id` construction is the caller's job — this crate does not
/// generate ids, mirroring `memory.sh`'s own id derivation staying in the shell layer,
/// `memory.sh:65`).
pub fn write(
    id: MemoryId,
    candidate: NewMemory,
    now: Timestamp,
    tau: DedupThreshold,
    existing: &dyn NearestNeighborLookup,
) -> Result<WriteDecision, RetrieveError> {
    unimplemented!()
}

// =====================================================================================
// G. pick_arm — Thompson sampling over models (Beta-Bernoulli bandit, ~50 LOC)
// =====================================================================================

/// One bandit arm's observed history. `id` matches a `fleet-router` `CandidateSpec::id` string
/// (`"codex"`, `"sonnet"`, ...) by convention, but this crate does not import `fleet-router` (a
/// sibling-to-sibling edge the DAG doesn't grant) — it is passed the same `&'static str` values.
#[derive(Clone, Copy, Debug)]
pub struct ArmStats { pub id: &'static str, pub successes: u64, pub failures: u64 }

/// `pick_arm` was called with an empty `arms` slice — there is nothing to choose among.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("pick_arm requires at least one arm")]
pub struct NoArms;

/// The injected uniform-random source, `(0.0, 1.0)` exclusive both ends. This crate never reads a
/// thread-local or ambient RNG (§4) — callers pass a concrete `RandomSource` (e.g. wrapping
/// `fastrand`, already resolved in `fleet/keel/Cargo.lock`) so tests can inject a seeded/fixed
/// sequence and reproduce a specific draw.
pub trait RandomSource { fn uniform(&mut self) -> f64; }

/// Thompson sampling: draw one `Beta(successes + 1, failures + 1)` sample per arm (a Bayesian
/// posterior over each arm's success rate, Jeffreys-style `+1` prior avoiding a degenerate
/// `Beta(0, .)`), and return the `id` of the arm with the highest draw. Ties (identical `f64`
/// draws, vanishingly unlikely but not impossible) break toward the earlier arm in `arms`, for
/// determinism given a fixed `RandomSource` sequence. Never panics; the only failure mode is
/// `NoArms`.
pub fn pick_arm(arms: &[ArmStats], rng: &mut dyn RandomSource) -> Result<&'static str, NoArms> {
    unimplemented!()
}

// =====================================================================================
// H. promote_lesson / check_added_line — lesson -> enforceable diff-pattern gate
// =====================================================================================

/// A caller-supplied, not-yet-compiled diff-matching pattern (an ERE string, mirroring
/// `PROMOTIONS.jsonl`'s `pattern` field consumed by `memory-check.sh:141-149`'s
/// `normalize_pattern` + `grep -E`). This crate validates only non-emptiness — compiling/running the
/// pattern is the injected `PatternMatcher`'s job (§2).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffPattern(String);
impl DiffPattern {
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyMemoryId> { unimplemented!() }
    pub fn as_str(&self) -> &str { unimplemented!() }
}

/// Mirrors `memory-check.sh:175`'s `[ "$scope" = source ]` gate: today only `source` scope is
/// enforced against added lines; other scopes are recorded but not enforced. Kept as a closed enum
/// so a future scope is a reviewed addition, not a silently-ignored string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromotionScope { Source, Other }

/// A promoted, enforceable lesson — the pure data this crate hands to a gate (`fleet-verify`),
/// mirroring one row of `PROMOTIONS.jsonl` (`category`, `pattern`, `scope`, `what_failed`/`artifact`
/// folded into `message`).
#[derive(Clone, Debug)]
pub struct PromotedLesson { pub category: String, pub pattern: DiffPattern, pub scope: PromotionScope, pub message: String }

/// Why a memory did not qualify for promotion.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PromotionRefusal {
    #[error("memory confirmed {actual} time(s), promotion requires at least {required}")]
    NotConfirmedEnough { required: u32, actual: u32 },
    #[error("promotion pattern must not be empty")]
    EmptyPattern,
    #[error("promotion category must not be empty")]
    EmptyCategory,
}

/// Pure and total. Refuses promotion if `item.confirmed_count < min_confirmations` (the "has this
/// actually recurred" bar — a once-seen correction should not become an enforced gate, matching the
/// spirit of `memory-check.sh`'s promotion-ledger-driven design, which only ever enforces rows
/// someone already promoted, never a first-sighting) or if `pattern`/`category` are empty;
/// otherwise builds a `PromotedLesson`.
pub fn promote_lesson(
    item: &MemoryItem,
    pattern: &str,
    category: &str,
    scope: PromotionScope,
    min_confirmations: u32,
) -> Result<PromotedLesson, PromotionRefusal> {
    unimplemented!()
}

/// The injected pattern-matching port — wraps whatever regex engine the caller chooses (today
/// `grep -E`'s ERE dialect via subprocess in `memory-check.sh:180`; tomorrow likely the `regex`
/// crate in `fleet-verify`). This crate links no regex engine itself.
pub trait PatternMatcher {
    fn is_match(&self, pattern: &DiffPattern, line: &str) -> Result<bool, PatternError>;
}
/// The injected `PatternMatcher` failed to evaluate (e.g. the pattern doesn't compile in the
/// caller's regex dialect — mirrors `memory-check.sh:172`'s `signature_unparseable` exit).
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("pattern match failed: {0}")]
pub struct PatternError(pub String);

/// Pure orchestration over one injected call: if `matcher.is_match(&lesson.pattern, line)` is
/// `Ok(true)`, returns `Ok(Some(refusal))` where `refusal` is a `fleet_types::GateRefusal` whose
/// `code` is `lesson.category` and whose `message` names the offending line (mirrors
/// `memory-check.sh:181`'s rejection message shape); `Ok(false)` returns `Ok(None)` (line is clean);
/// the matcher's own error propagates unchanged.
pub fn check_added_line(lesson: &PromotedLesson, line: &str, matcher: &dyn PatternMatcher) -> Result<Option<GateRefusal>, PatternError> {
    unimplemented!()
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `MemoryId` | Non-empty after trimming, same rule as `fleet-types`' identifier newtypes. | An empty-string id silently colliding with every other empty id in a dedup lookup. |
| `Embedding` | Non-empty `Vec<f32>`; any two compared embeddings must match length or the op returns `DimensionMismatch` — never silently zero-padded/truncated. | A model-version change silently corrupting cosine similarity by comparing vectors of two different lengths as if they matched. |
| `CosineSimilarity` | Always in `[-1.0, 1.0]`, clamped at construction inside `Embedding::cosine`. | Floating-point drift producing `1.0000000002`, which would break a "similarity can never exceed 1.0" assumption in `write`'s threshold comparison. |
| `Importance` / `Relevance` | Always finite and in `[0.0, 1.0]`; `NaN`/`inf`/out-of-range values are rejected at construction, never silently clamped. | A `NaN` importance propagating into `score`'s weighted sum and turning the whole `Score` into `NaN`, which would then sort unpredictably relative to every other item. |
| `Timestamp` | An opaque `u64` of unix seconds; never derived from `SystemTime::now()` inside this crate. | This crate's output silently depending on wall-clock time at test-run time instead of the `now` a test explicitly passed in. |
| `MemoryItem.confirmed_count` | `u32`, incremented only through `write`'s `Merge` decision or the caller's own logic — never decremented by this crate. | A confirmation count silently shrinking, which would misrepresent how many times a correction was actually seen. |
| `WriteDecision` | Exactly one of `Insert`/`Merge` per call — never both, never neither (enforced by `write`'s single `if`/`else`, not by convention; a unit test asserts this, §9). | A caller receiving an ambiguous "maybe insert, maybe merge" outcome and guessing. |
| `ArmStats` | `successes`/`failures` are `u64`, never negative (unsigned) and never combined via unchecked arithmetic inside `pick_arm` (it only reads them, `+1` for the prior is `u64 + 1` and cannot overflow at any realistic count without exhausting memory first). | A negative failure count silently making a Beta distribution's shape parameter negative (undefined) rather than the caller's bug being visible. |
| `PromotedLesson` / `DiffPattern` | `pattern`/`category` non-empty by construction (`promote_lesson` refuses otherwise); `scope` is a closed 2-variant enum, not a bare string, so an unenforced third scope must be a reviewed addition to this enum. | A promotion silently created with an empty pattern that would then match every line (an empty ERE matches everything), turning a targeted lesson into a blanket false-positive generator. |

**Money/precision:** no money type in this crate. `Importance`/`Relevance`/`Score`/
`CosineSimilarity` are `f64` — measurement statistics (like fleet-types' Wilson-score rates), never
currency or token counts. `confirmed_count` and bandit `successes`/`failures` are unsigned integer
counts, never float.

**Clock/RNG/IO injection points:** `Timestamp` is the sole clock-adjacent value, always caller-
supplied — this crate calls `SystemTime::now()` nowhere. `RandomSource` (§3.G) is the sole RNG
injection point, always caller-supplied — no `rand::thread_rng()`/ambient seed anywhere. IO:
`LexicalSearch`, `VectorSearch`, `NearestNeighborLookup`, and `PatternMatcher` are the four injected
ports; every one is a trait object the caller implements against the real store/index/regex engine,
and this crate calls none of `std::fs`, `std::net`, or `std::process` anywhere in `src/`.

## 5. Reuse map

Source read in full: `fleet/registry-reference/registry/features/memory/memory.sh` (148 lines) and
`.../memory/memory_store.py` (593 lines).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `memory_store.py:52-64` (`memories` table columns) | Flat row: no kind split, `confirmed_count`, `severity`. | reshaped | Becomes `MemoryItem` (§3.C) plus the new `MemoryKind` split (§3.A) `memory_store.py` never had. `severity` (critical/important/minor, `memory.sh:61-64`) is not carried into this crate's model — it's a storage-layer concern for `fleet-store` to keep or fold into `Importance`, not this crate's decision. |
| `memory_store.py:326-346` (`bm25_search`) | FTS5 MATCH query, `bm25_rank` assigned by enumeration. | **no** | Lives in `fleet-store`'s `LexicalSearch` implementation; this crate only consumes the `(id, bm25_rank)` shape as `LexicalHit`. |
| `memory_store.py:349-405` (`search`, hybrid path) | Runs BM25 + vector kNN, fuses via RRF (`rrf_score` line 386), sorts, truncates. | logic yes, IO no | The RRF formula (`1/(RRF_K+bm25_rank) + 1/(RRF_K+vector_rank)`, `RRF_K=60`) is ported verbatim into `retrieve` (§3.E) as the relevance fusion; the BM25 query and the `vector_bridge("knn", ...)` subprocess call (lines 362-364) are **not** ported — those become the caller's `LexicalSearch`/`VectorSearch` implementations. |
| `memory_store.py:239-247` (`embedder`) | Loads `fastembed`'s `BAAI/bge-small-en-v1.5`, offline. | **no** | Model loading/inference is `fleet-context`'s job; this crate only ever receives an already-computed `Embedding`. |
| `memory_store.py:408-423` (`remember`) | `INSERT ... ON CONFLICT(id) DO UPDATE ... confirmed_count = confirmed_count + 1`. | logic partially reused | The "corroboration bumps `confirmed_count` and refreshes timestamps" idea is the direct model for `write`'s `Merge` variant (§3.F) — but today's trigger is `id` equality (same caller-supplied id); this crate's trigger is **cosine similarity above tau**, the dedup behavior the brief says is absent. The actual `INSERT`/`UPDATE` SQL stays `fleet-store`'s job. |
| `memory.sh:49-75` (`cmd_remember`) | Derives an id from a slug + content hash when none given (`memory.sh:65`), calls `store remember`, appends a receipt. | **no** | Id derivation, receipt-append (`record_receipt`, `memory.sh:39-45`), and CLI parsing all stay outside this crate (composition root + `fleet-store`) — this crate's `write` takes an already-decided `MemoryId` as a parameter. |
| `memory.sh:102-124` (`cmd_probe`) | Hit/miss check: is a given lesson id present in a `search` result for a context string, exit 0/6. | **not reused directly** | This is a *testing/eval* tool (T7-style probe), not part of the scoring/dedup/bandit/promotion decision path this crate owns — no fn here mirrors it; if fleet wants a "probe" concept again it composes `retrieve` + an id-membership check at the caller, not inside this crate. |
| (absent anywhere in fleet) episodic/semantic/procedural split | n/a | greenfield | `MemoryKind` (§3.A) — the brief names this ABSENT; no fleet source to cite. |
| (absent anywhere in fleet) dedup-on-write / merge cos > tau | n/a | greenfield | `write`/`WriteDecision` (§3.F) — the brief names this ABSENT (the "44%→14% bloat fix" framing). A repo-wide grep for the literal figures `44%`/`14%` and for "bloat" found no fleet source or doc citing them (see the divergence note) — the dedup design is justified directly from the brief's stated problem (unbounded append growth), not from a verified historical number. |
| (absent anywhere in fleet) Thompson-sampling bandit | n/a | greenfield | `pick_arm`/`ArmStats`/`RandomSource` (§3.G) — the brief names this ABSENT and sizes it at ~50 LOC; this blueprint's §8 splits it across `bandit/random.rs` + `bandit/thompson.rs` to stay under the 80-line-per-file cap while landing close to that budget in aggregate. |
| `memory-check.sh:1-14,79-95,141-190` (rung-3 promotion enforcement) | Reads `PROMOTIONS.jsonl`, confirms presence in the memory store (SQLite or JSON fallback), normalizes the pattern, `grep -E`s added diff lines, dies with `ERR_INVARIANT` on a match. | reshaped | `promote_lesson`/`check_added_line`/`PromotedLesson` (§3.H) are the pure core of this: "is this memory confirmed enough to promote" and "does this line match the promoted pattern" become typed fns; the diff-parsing (`awk` extraction, lines 97-101), the promotions-ledger read/dedup-by-category (`jq` pipeline, lines 105-117), and the actual `grep -E` execution stay outside this crate (caller + injected `PatternMatcher`). |

## 6. Behavior spec

### `fn score(item: &MemoryItem, now: Timestamp, relevance: Relevance, weights: ScoreWeights) -> Score`

| Input dimension | Behavior |
|---|---|
| empty | `weights = {0.0, 0.0, 0.0}` → `Score(0.0)` regardless of `item`/`relevance` — a legal, if useless, weighting; not an error. |
| null / `None` | n/a — every parameter is a required value type, no `Option` in this signature. |
| wrong-type | n/a — no stringly-typed input; `Importance`/`Relevance` are already validated before this fn ever sees them. |
| huge | `weights.alpha = 1e300` → `Score` can legitimately be astronomically large or overflow to `f64::INFINITY`; this fn does not clamp the output (only the three *inputs* are range-checked, per §4) — a caller wanting a bounded final score enforces bounded weights itself. |
| negative | Negative weights are accepted (not range-checked, unlike `Importance`/`Relevance`) — a policy that wants to *penalize* recency, say, can pass a negative `alpha`; this is a deliberate design choice documented here, not an oversight. |
| duplicate | Calling `score` twice with byte-identical arguments yields byte-identical `Score` — pure fn, no hidden state. |
| concurrent | Pure value fn, `&`-refs only — trivially safe from any number of threads. |
| unicode / non-ASCII | n/a — `item.text` is never read by this fn; only numeric fields (`importance`, `last_confirmed_at`) participate. |
| already-exists | n/a — no persisted/identity concept here. |
| partial-failure | n/a — no IO, always returns a complete `Score` synchronously. |

### `fn retrieve(...) -> Result<Vec<RetrievedItem>, RetrieveError>`

| Input dimension | Behavior |
|---|---|
| empty | `query = ""` → `terms()`-style tokenization (mirroring `memory_store.py:316-323`) yields no lexical hits; `lexical.search` and `vector.knn` are still called (this fn does not special-case empty queries) and may legitimately return `Ok(vec![])`, which `retrieve` returns as `Ok(vec![])`, never an error. `items` empty → `Ok(vec![])` regardless of what the ports return (nothing to look up). |
| null / `None` | n/a — no `Option` params; `limit: 0` → `Ok(vec![])` (zero requested, zero returned, not an error). |
| wrong-type | n/a — no stringly-typed/deserialized input at this boundary. |
| huge | `limit = 1_000_000` with a small `items` map → returns everything available (bounded by `items.len()`, never panics reading past the end); a port returning far more hits than `items` contains (a caller bug) → this fn only scores ids present in `items`, silently ignoring any hit id absent from the map (documented here as the defined behavior, not a special error, since "the caller's `items` map is stale relative to the port" is the caller's inconsistency to fix, not this fn's to detect). |
| negative | n/a — `limit: usize` cannot be negative. |
| duplicate | The same `MemoryId` appearing in both `lexical`'s and `vector`'s results is the expected, common case — RRF sums both terms for that one id exactly once (mirrors `memory_store.py:378-395`'s `set(bm25_by_id) | set(vector_by_id)` union-by-id, never double-counted). |
| concurrent | `retrieve` takes `&`-refs (including `&dyn LexicalSearch`/`&dyn VectorSearch`) and returns an owned `Vec` — safe to call from multiple threads provided the trait-object implementations themselves are `Sync`, which is the caller's contract to uphold, stated here as the crate's assumption. |
| unicode / non-ASCII | `query` containing emoji/non-ASCII: tokenization/matching is entirely the injected `LexicalSearch`'s concern (this fn passes `query` through unchanged); this fn's own string handling is limited to passing `query`/`query_embedding` by reference, so no unicode-specific bug surface exists here. |
| already-exists | Calling `retrieve` twice with identical inputs (including a fixed `now`) is required to be idempotent — same ports, same map, same result, always (a determinism property asserted in §9). |
| partial-failure | If `lexical.search` returns `Err`, `retrieve` returns that `Err` immediately without calling `vector.knn` at all (fail-fast, never a partial fusion over only one port's results presented as if it were complete) — same for `vector.knn` failing first. |

### `fn write(...) -> Result<WriteDecision, RetrieveError>`

| Input dimension | Behavior |
|---|---|
| empty | `candidate.text = ""` is accepted (this fn does not validate text non-emptiness — that's a caller/`fleet-store` schema concern); `existing.nearest` returning `Ok(None)` (an empty store) → always `Insert`. |
| null / `None` | `existing.nearest` returning `Ok(None)` is the explicit "no candidate" case, handled as `Insert` — never confused with an error. |
| wrong-type | n/a — no stringly-typed/deserialized input. |
| huge | A `tau` of `1.0` (only an exact-duplicate embedding merges) vs. `-1.0` (everything merges into whatever `nearest` returns, if anything) are both legal, if extreme, threshold choices — this fn does not reject a value at either end of `[-1.0, 1.0]`. |
| negative | `DedupThreshold::new` rejects non-finite values via the same `ImportanceOutOfRange`-shaped check as `Importance`/`Relevance` (§3.F) but does allow negative `tau` (a valid, if permissive, cosine threshold) — only `NaN`/`inf` are refused. |
| duplicate | Two calls to `write` with the same `candidate` embedding both consulting `existing.nearest` independently: the *first* call's `Insert` is not visible to the *second* call unless the caller has already persisted it and `existing`'s next `nearest` reflects that — `write` itself has no memory of prior calls (pure fn, no internal state), so "duplicate insert" prevention across repeated calls is entirely a function of how faithfully the caller's `NearestNeighborLookup` reflects what's actually been persisted so far. |
| concurrent | Pure fn over `&`-refs; safe to call from multiple threads, but the *port* (`existing`) must itself handle concurrent `nearest` calls safely if the caller's system is concurrent — this fn makes no promise about two concurrent `write` calls racing to insert two near-duplicates when the underlying store hasn't picked up the first insert yet (a `fleet-store`-level concern, documented here as out of this fn's control). |
| unicode / non-ASCII | `candidate.text` containing non-ASCII is passed through unchanged into the `Insert`ed `MemoryItem` — this fn does no text normalization. |
| already-exists | A `nearest` hit exactly at `sim == tau` → `Merge` (`>=`, not `>` — the boundary is inclusive, a specific mutation target in §9). |
| partial-failure | `existing.nearest` returning `Err` → `write` returns that `Err` immediately; no partial `WriteDecision` is ever constructed. |

### `fn pick_arm(arms: &[ArmStats], rng: &mut dyn RandomSource) -> Result<&'static str, NoArms>`

| Input dimension | Behavior |
|---|---|
| empty | `arms = []` → `Err(NoArms)`, never a panic on an out-of-bounds "best index" computation. |
| null / `None` | n/a — no `Option` params. |
| wrong-type | n/a — `ArmStats` fields are already-typed integers. |
| huge | `successes`/`failures` in the billions: the Beta-sampling implementation (Gamma-ratio method, §8's `bandit/random.rs`) is `O(1)` expected work per arm regardless of the magnitude of `successes + 1`/`failures + 1` (it does NOT use an order-statistic method that would need `successes + failures` uniform draws) — this is the specific reason Gamma-ratio sampling is chosen over the simpler "kth order statistic of n uniforms" approach, which would be `O(n)` and unbounded for a long-lived arm. |
| negative | n/a — `successes`/`failures: u64` cannot be negative; `+ 1` for the Bayesian prior cannot underflow. |
| duplicate | Two arms sharing the same `id` string in `arms` is not this fn's error to catch (it has no uniqueness requirement on `id`) — whichever one happens to draw the higher sample wins; a caller wanting uniqueness enforces it upstream (mirrors `fleet-router`'s `ORDER` distinctness being a *test-time* assertion on the caller's static table, not a runtime check here). |
| concurrent | `rng: &mut dyn RandomSource` requires exclusive access — `pick_arm` cannot be called concurrently with the *same* `rng` from two threads (a `&mut` reference is not `Sync`-shareable); two threads with two independent `RandomSource` instances are trivially safe and independent. |
| unicode / non-ASCII | `id: &'static str` values containing non-ASCII round-trip unchanged — this fn never inspects `id`'s contents, only compares sampled `f64`s. |
| already-exists | Calling `pick_arm` twice with the same `arms` but a `rng` that has already advanced (not reset) legitimately returns a different arm on the second call — this is Thompson sampling's whole point (explore proportional to uncertainty), not a bug; determinism is asserted only for a *fixed, reset* `RandomSource` sequence (§9). |
| partial-failure | n/a — no IO; always returns synchronously. |

### `fn promote_lesson(...) -> Result<PromotedLesson, PromotionRefusal>`

| Input dimension | Behavior |
|---|---|
| empty | `pattern = ""` → `Err(EmptyPattern)`; `category = ""` → `Err(EmptyCategory)`, checked before the confirmation-count check so a caller sees the cheapest-to-fix problem first (deterministic ordering, asserted in §9). |
| null / `None` | n/a — no `Option` params; `min_confirmations = 0` → every item qualifies (even `confirmed_count = 0`, e.g. an item that was inserted but never separately confirmed) — `0` is a legal, if permissive, threshold, not rejected. |
| wrong-type | n/a — no stringly-typed/deserialized input beyond the two `&str`s already validated. |
| huge | `min_confirmations = u32::MAX` → refuses essentially every real item with `Err(NotConfirmedEnough { required: u32::MAX, actual: item.confirmed_count })` — no overflow, since the comparison is `<`, never an arithmetic combination of the two. |
| negative | n/a — `confirmed_count`/`min_confirmations: u32` cannot be negative. |
| duplicate | Calling `promote_lesson` twice for the same `item`/pattern/category is idempotent — returns an equal `PromotedLesson` both times (no hidden counter incremented by this fn itself). |
| concurrent | Pure value fn over `&`-refs — trivially safe. |
| unicode / non-ASCII | `category`/`pattern` containing non-ASCII pass through unchanged into `PromotedLesson` — no normalization performed here (matches `memory-check.sh`'s own byte-oriented `grep -E` treatment downstream). |
| already-exists | n/a — no persisted "already promoted" state this fn consults; a caller wanting to avoid re-promoting the same category is expected to check the promotions ledger itself (that ledger's dedup-by-category, `memory-check.sh:106-114`'s `group_by(.category) | map(.[-1])`, stays outside this crate). |
| partial-failure | n/a — no IO; always returns synchronously. |

### `fn check_added_line(lesson: &PromotedLesson, line: &str, matcher: &dyn PatternMatcher) -> Result<Option<GateRefusal>, PatternError>`

| Input dimension | Behavior |
|---|---|
| empty | `line = ""` → passed to `matcher.is_match` unchanged; whether an empty line ever matches a given pattern is the matcher's/pattern's business, not special-cased here. |
| null / `None` | n/a — no `Option` params. |
| wrong-type | n/a — both inputs are already-typed. |
| huge | `line` of 1MB (a pathological diff line) → passed through by reference, no copy beyond what the matcher itself makes; this fn allocates nothing proportional to `line`'s size. |
| negative | n/a — no numeric input. |
| duplicate | Calling `check_added_line` twice with the same `lesson`/`line` is idempotent (assuming a deterministic `matcher`, which is the caller's contract) — same `Option<GateRefusal>` both times. |
| concurrent | Pure orchestration over `&`-refs (`matcher` itself must be `Sync` if called concurrently — the caller's contract, as with `retrieve`'s ports). |
| unicode / non-ASCII | `line` containing non-ASCII passes through unchanged; ERE matching semantics over unicode are entirely the injected `matcher`'s responsibility. |
| already-exists | n/a — no persisted state. |
| partial-failure | `matcher.is_match` returning `Err(PatternError)` (mirrors `memory-check.sh:172`'s `signature_unparseable`) propagates immediately — never silently treated as "no match." |

## 7. Dependencies

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace-path (`{ path = "../fleet-types" }`) | Supplies `GateRefusal`, the `{code, message}` refusal shape `check_added_line` reuses instead of a fifth bespoke refusal struct in the roster. |
| `serde` | `1.0.229` (matches `fleet/keel/Cargo.lock`) | `MemoryItem`/`Embedding`/`MemoryKind` derive `Serialize`/`Deserialize` so `fleet-store` can persist and re-hydrate them without this crate knowing the storage format. |
| `thiserror` | `2.0.20` (matches `fleet/keel/Cargo.lock`) | Every fallible fn returns a `thiserror`-derived typed error, per this crate's (and the template's) hard rule against `String`/`anyhow`/bare `bool`/`Option`. |

No `regex`, no `rand`, no `sqlite`/`rusqlite`, no `fastembed`/ML crate: all four are non-goals this
crate deliberately pushes to its injected ports and callers (§2). `RandomSource`'s Beta-sampling
math (§8's `bandit/random.rs`) is hand-rolled (Marsaglia–Tsang Gamma + Box–Muller normal, both
closed-form given only uniform draws) specifically to avoid a `rand`/`rand_distr` dependency for
~50 lines of math; `fastrand` (already resolved in `fleet/keel/Cargo.lock:428`) is the crate a
concrete `RandomSource` implementation would wrap at the composition root, but it is not this
crate's own dependency — it depends only on the trait.

**Considered, not depended upon (per the brief):** `mem0`, `Letta`, and `MegaMemory` were named in
the brief as optional external memory sidecars this crate should be aware of but never call into. A
repo-wide grep found no existing fleet integration with any of the three. This blueprint does not
add one: if a future adapter wants their output considered during retrieval, it implements this
crate's `LexicalSearch`/`VectorSearch` traits from outside `fleet-memory` (e.g. a `fleet-context`
adapter that queries mem0 as one of several backing sources and returns ordinary `LexicalHit`/
`VectorHit` values) — this crate's decision path (`score`/`retrieve`/`write`/`pick_arm`/
`promote_lesson`) never imports, links against, or has any special-case branch for any of the three
by name.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.**

```
crates/fleet-memory/
  Cargo.toml
  src/
    lib.rs            # ~20 — module decls + re-exports only
    ident.rs           # ~30 — MemoryId, EmptyMemoryId, MemoryKind
    embedding.rs       # ~45 — Embedding, CosineSimilarity, EmptyEmbedding, DimensionMismatch, cosine()
    item.rs            # ~55 — Timestamp, Importance, ImportanceOutOfRange, MemoryItem
    score.rs           # ~55 — Relevance, ScoreWeights, Score, RECENCY_HALF_LIFE_SECS, score()
    retrieve.rs        # ~75 — LexicalHit/VectorHit/LexicalSearch/VectorSearch/RetrieveError/RetrievedItem, retrieve()
    dedup.rs           # ~60 — NewMemory, NearestNeighborLookup, DedupThreshold, WriteDecision, write()
    bandit/
      mod.rs           # ~10 — module decls + re-exports
      arm.rs           # ~15 — ArmStats, NoArms
      random.rs        # ~50 — RandomSource trait, gamma_sample(), normal_sample() (Marsaglia-Tsang + Box-Muller)
      thompson.rs      # ~40 — beta_sample() (ratio of two gamma_sample calls), pick_arm()
    promote.rs         # ~75 — DiffPattern, PromotionScope, PromotedLesson, PromotionRefusal, promote_lesson(), PatternMatcher, PatternError, check_added_line()
  tests/
    score_recency.rs        # ~50 — score() edge cases (§9)
    retrieve_fusion.rs       # ~70 — RRF fusion + fail-fast port errors (§9)
    dedup_threshold.rs       # ~55 — write() Insert/Merge boundary cases (§9)
    bandit_thompson.rs       # ~70 — pick_arm() determinism + exploration cases (§9)
    promote_and_check.rs     # ~60 — promote_lesson()/check_added_line() cases (§9)
```
> If any file above still projects over 80 lines once bodies land, split again (e.g. `retrieve.rs`
> → `retrieve.rs` + `fusion.rs`). The file-size gate (§10) runs before Opus review.

`Cargo.toml` sketch:
```toml
[package]
name = "fleet-memory"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0.229", features = ["derive"] }
thiserror = "2.0.20"
fleet-types = { path = "../fleet-types" }

[dev-dependencies]
serde_json = "1.0.151"
```

## 9. Test plan

**Unit tests:**
- `recency_decays_by_half_at_one_half_life` — `score` with `alpha=1, beta=0, gamma=0` and
  `now - last_confirmed_at == RECENCY_HALF_LIFE_SECS` asserts the returned `Score` is `0.5` within
  float tolerance; at `now == last_confirmed_at`, asserts `1.0`.
- `recency_clamps_future_last_confirmed_to_one` — `last_confirmed_at > now` (a corrupt/clock-skewed
  item) asserts recency `1.0`, never negative age, never a panic on `u64` subtraction underflow.
- `embedding_cosine_rejects_dimension_mismatch` — a 3-dim and a 4-dim `Embedding` → `Err(DimensionMismatch { a: 3, b: 4 })`.
- `embedding_cosine_is_clamped_to_valid_range` — two embeddings pathologically close to parallel
  (floating-point-drift-prone) never produce a `CosineSimilarity` outside `[-1.0, 1.0]`.
- `write_merges_at_exact_threshold_boundary` — a `nearest` result with `sim == tau` exactly →
  `Merge`, not `Insert` (kills a `>=`-to-`>` mutation).
- `write_inserts_when_no_neighbor_exists` — `existing.nearest` returns `Ok(None)` → always `Insert`.
- `pick_arm_rejects_empty_arms` — `arms = []` → `Err(NoArms)`.
- `pick_arm_is_deterministic_for_a_fixed_random_source` — a `RandomSource` test double replaying a
  fixed sequence of uniforms yields the identical chosen arm across repeated calls with the sequence
  reset each time.
- `pick_arm_favors_the_higher_success_rate_arm_over_many_trials` — over N=2000 repeated draws (each
  with a fresh, differently-seeded `RandomSource`), the arm with e.g. 90 successes/10 failures is
  chosen strictly more often than one with 10 successes/90 failures (a statistical property test,
  not an exact-count assertion — asserts `count_a > count_b`, generously bounded).
- `promote_lesson_refuses_before_checking_confirmations_when_pattern_is_empty` — empty pattern +
  under-threshold `confirmed_count` together → `Err(EmptyPattern)`, not `NotConfirmedEnough` (asserts
  the stated check-ordering from §6).
- `promote_lesson_accepts_zero_min_confirmations` — `min_confirmations = 0` on an item with
  `confirmed_count = 0` → `Ok(_)`.
- `check_added_line_wraps_a_hit_as_gate_refusal_named_by_category` — a `PatternMatcher` test double
  returning `Ok(true)` → `Ok(Some(refusal))` where `refusal.code() == lesson.category`.

**Integration tests** (calling only the public API):
- `retrieve_fuses_lexical_and_vector_hits_via_rrf` — test-double `LexicalSearch`/`VectorSearch`
  ports return disjoint and overlapping id sets; asserts the fused `Relevance` for an id present in
  both equals `1/(60+bm25_rank) + 1/(60+vector_rank)` (the exact `memory_store.py:386` formula) and
  that an id present in only one port omits the other term rather than erroring.
- `retrieve_fails_fast_on_first_port_error` — a `LexicalSearch` double returning `Err` → `retrieve`
  returns that `Err` without the `VectorSearch` double ever being called (asserted via a call-count
  check on the double).
- `retrieve_is_deterministic_and_ties_break_by_id` — two items with identical fused `Score` sort by
  `MemoryId` ascending, verified across repeated calls.
- `write_dedup_reduces_insert_rate_on_a_near_duplicate_stream` — feeding `write` a stream of
  candidates where every 3rd one is a near-duplicate (cosine ≈ 0.95) of an earlier one, with
  `tau = 0.9`, asserts the `Insert` count is strictly less than the candidate count (the dedup
  property the brief's bloat-reduction goal is actually about) and every non-`Insert` outcome is a
  `Merge` referencing the correct earlier id.
- `promote_then_check_end_to_end` — `promote_lesson` on a sufficiently-confirmed item, then
  `check_added_line` against a matching line and a non-matching line via a real (non-double) simple
  substring-based `PatternMatcher` test implementation, asserting `Some`/`None` respectively.

**Mutation-testing targets (`cargo mutants -p fleet-memory` must not survive):**
- Flipping `write`'s `sim >= tau` to `sim > tau` — killed by `write_merges_at_exact_threshold_boundary`.
- Flipping `score`'s recency clamp (`max(age, 0)`) to allow negative age — killed by
  `recency_clamps_future_last_confirmed_to_one`.
- Deleting the fail-fast early-return in `retrieve` (so `vector.knn` runs even after `lexical.search`
  errored) — killed by `retrieve_fails_fast_on_first_port_error`'s call-count assertion.
- Swapping `promote_lesson`'s check order (confirmations before pattern/category emptiness) —
  killed by `promote_lesson_refuses_before_checking_confirmations_when_pattern_is_empty`.
- Flipping `pick_arm`'s "highest sample wins" to "lowest sample wins" — killed by
  `pick_arm_favors_the_higher_success_rate_arm_over_many_trials` (the favored arm would invert).

**Property tests (`proptest`, recommended given the float-heavy surface):**
- *Cosine similarity is always in range*: for arbitrary same-length `f32` vectors (excluding all-
  zero, an undefined-cosine edge this fn should refuse rather than divide by zero — a case to add to
  `Embedding::cosine`'s contract if not already typed), `cosine(a, b).get()` is always in
  `[-1.0, 1.0]`, 200 cases minimum.
- *RRF relevance is monotonic in rank*: for arbitrary rank pairs, a strictly better (lower) rank in
  either port never produces a lower fused relevance than a strictly worse rank, all else equal, 100
  cases minimum.

## 10. Verification recipe

```bash
cd crates/fleet-memory
cargo test -p fleet-memory --all-targets
cargo clippy -p fleet-memory --all-targets -- -D warnings
cargo mutants -p fleet-memory
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration tests pass, 0 skipped — publish as `<passed>/<total>` (e.g.
`24/24`). Clippy: 0 warnings. Mutants: every target named in §9 caught; publish `<caught>/<total
mutants>` — given this crate's small, mostly-pure surface, the floor is every named mutation-testing
target caught, no partial credit; any survivor gets a new test, not a lowered floor.

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`EmptyMemoryId`, `EmptyEmbedding`,
      `DimensionMismatch`, `ImportanceOutOfRange`, `RetrieveError`, `NoArms`, `PromotionRefusal`,
      `PatternError`) — none swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code.
- [ ] Clock/RNG/IO are injected — `Timestamp` and `RandomSource` are always parameters, never
      ambient; `LexicalSearch`/`VectorSearch`/`NearestNeighborLookup`/`PatternMatcher` are the four
      injected IO ports, never a direct `std::fs`/`std::net`/`std::process` call in `src/`.
- [ ] Thread-safety documented: every public fn takes `&`/`&mut` refs and returns owned values with
      no interior mutability of its own — `Send`/`Sync` follow from the trait objects' own bounds,
      which the caller's port implementations must uphold (stated per-fn in §6's "concurrent" rows).
- [ ] No float used for money or token counts — `Importance`/`Relevance`/`Score`/
      `CosineSimilarity` are measurement `f64`s (never money/tokens); `confirmed_count`/
      `successes`/`failures` are unsigned integers throughout.
- [ ] No self-grading: verification runs `cargo mutants`, not just this crate's own unit tests;
      denominator published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10, template) — restate
      the real numbers in the PR once the crate is built (mark done then).
- [ ] Tests that touch the filesystem: none needed — every port in this crate is a trait object test
      double; if a future test needs a temp file it must use `tempdir()`, never the repo tree.
- [ ] Every non-goal in §2 is absent from the code — no SQL, no regex engine, no ML model loading,
      no subprocess spawn anywhere in `crates/fleet-memory/src/`; enforce with
      `grep -rn "sqlite\|regex::\|fastembed\|Command::new" crates/fleet-memory/src/` returning nothing.
- [ ] No source file exceeds 80 lines — §8 splits every type/fn group into its own file; verified by
      the §10 `wc -l ... awk '$1>80'` gate before Opus review.

## 12. Definition of Done

`fleet-memory` is DONE when: §10's four commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught) run from `crates/fleet-memory/`; every unchecked box in
§11 is checked with its real numbers; `registry/features/memory/REGISTRY.md` (or the roster's
`features/REGISTRY.md` per C1/L2 — memory is a feature-layer concern, not core infrastructure) lists
the crate; and Opus has independently re-derived the score/dedup/bandit/promotion contracts from
this blueprint alone (without re-reading `memory.sh`/`memory_store.py`), reproduced the
`write`'s `sim >= tau` boundary mutation by hand, and driven one real end-to-end call
(`write` → `Merge` on a synthetic near-duplicate pair, then `retrieve` confirming the merged item's
`confirmed_count` increase is reflected in a higher `Score`) confirming the pipeline behaves as
claimed.

---

## Divergence from MIGRATION-PLAN (for Opus)

1. **No compile dependency on `fleet-store`/`fleet-context`.** MIGRATION-PLAN's DAG
   (`types → {lifecycle, store} → {siblings} → src/`) reads as if `fleet-memory` sits below `store`
   and could depend on it directly. Neither `fleet-store` nor `fleet-context` has a written
   blueprint yet (both `blueprints/fleet-store/` and `blueprints/fleet-context/` are empty). This
   blueprint instead defines four SPI traits (`LexicalSearch`, `VectorSearch`,
   `NearestNeighborLookup`, `PatternMatcher`) as the seam, mirroring `fleet-router`'s
   Opus-approved `RuntimeState` pattern. When `fleet-store`/`fleet-context` are blueprinted, Opus
   should decide whether these traits move to `fleet-types` as shared SPI (very likely, since
   `fleet-context` will probably want the identical `LexicalSearch`/`VectorSearch` shape for its own
   callers) or stay local to `fleet-memory` with `fleet-store`/`fleet-context` implementing them
   from outside.
2. **The "44%→14% bloat fix" figure named in the brief is not cited from real fleet source.** A
   repo-wide grep for `44%`, `14%`, and "bloat" found nothing in `fleet/` matching this claim. This
   blueprint's dedup design (`write`'s cosine-threshold `Merge`) is justified directly from the
   brief's stated problem (unbounded append growth on repeated near-duplicate corrections), not
   from a verified historical measurement — flagging this so Opus can supply the real source if one
   exists, or confirm the figure is a target/estimate rather than a measured fact.
3. **`mem0`/`Letta`/`MegaMemory` have no existing fleet integration.** Named per the brief in §7 as
   deliberately-not-depended-upon sidecars; no fleet source references any of the three today, so
   §5's reuse map has nothing to cite for them. If Opus wants a concrete adapter scoped, that is new
   work beyond this blueprint, entering through this crate's existing `LexicalSearch`/
   `VectorSearch` ports from a sibling crate, never as a new dependency of `fleet-memory` itself.
4. **`Role`/routing types are deliberately not imported.** The brief's "model-selection bandit"
   reads `ArmStats.id: &'static str` values shaped like `fleet-router`'s `CandidateSpec::id`
   strings, but this blueprint does not import `fleet-router` (a sibling-to-sibling edge the DAG
   does not grant) — `pick_arm` is generic over any `&'static str` arm identifier, and whoever wires
   `fleet-govern` to both crates is responsible for keeping the id strings in sync by convention,
   not by a shared type.
