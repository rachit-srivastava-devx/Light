# BLUEPRINT — `knowledge`

## 1. Identity and LLD path
- Node id / label / tag: `knowledge` / Repo Memory + Standards / `deterministic`
- LLD authority: `docs/LLD/LLD.md §§10–12`; `docs/LLD/lld-full-detail.architecture.json:components[id=knowledge]`
- Why: provide scoped repo memory and pinned standards to context without granting authority.
- Incoming edges: `offline -> knowledge`: promoted candidate/validated lesson; store/standards sources provide snapshots.
- Outgoing edges: `knowledge -> context`: manifest sources with scope, digest, applicability, and revision.
- Build status: `partial` — `fleet-memory` retrieval/promotion and `fleet-context` retrieval seams exist; central standards/snapshot authority is incomplete.

## 2. Responsibility and non-goals
**Owns:** memory/standards records, scope/applicability filtering, provenance, retention/tombstones, and context-source projection.
**Does not own:** context packing, model decisions, live policy grants, autonomous prompt edits, or external publication.

## 3. Boundary and authority
Deterministic store-port node. Imported memory and standards are untrusted data until schema, signature/revision, scope, and expiry checks pass. Knowledge may propose context sources; only controller/policy owns authority. Promotion requires offline evidence and never happens from one model completion.

## 4. Crate/package layout
`Cargo.toml` declares the node (≤25 lines); `src/lib.rs` exports it (≤30); `src/item.rs` owns records (≤70); `src/filter.rs` owns scope/expiry (≤70); `src/promote.rs` owns candidate checks (≤80); `src/port.rs` owns persistence (≤60); `tests/knowledge.rs` owns fixtures (≤80). All live under `crates/knowledge/`.

## 5. Public API contract
```rust
pub struct KnowledgeItem { pub id: String, pub kind: Kind, pub text_ref: String, pub scope: Scope, pub source_digest: String, pub evidence_count: u32, pub expires_at: Option<String>, pub revision: u64 }
pub enum Kind { Lesson, Standard }
pub trait KnowledgeStore: Send + Sync { fn list(&self, query: &str, scope: &Scope, limit: u32) -> Result<Vec<KnowledgeItem>, KnowledgeError>; fn put_candidate(&self, item: KnowledgeItem) -> Result<(), KnowledgeError>; }
pub fn sources(store: &dyn KnowledgeStore, query: &str, scope: &Scope, limit: u32) -> Result<SourceManifest, KnowledgeError>;
```
`SourceManifest` publishes `checked,total`, freshness, omitted refs, and immutable revision/digests. No `bool` success-only API.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| item | scope, digest, source, revision present | untraceable memory | 6 |
| standard | pinned revision/signature and expiry policy | branch widening authority | 6/7 |
| manifest | `checked>0` and `checked==total` for pass | zero/partial retrieval green | 8 |
| promotion | candidate + regression/evidence threshold | single-hit policy activation | 7/8 |

Counts are integers; hashes are fixed algorithm-tagged strings; missing usage/score remains unknown. SQLite is single-writer; reads are snapshot-consistent.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-memory/src/item.rs:45-54` `MemoryItem` | graph/source exact lines | extract item fields/counts | existing durable shape | add scope/digest/revision fields |
| `crates/fleet-memory/src/retrieve.rs:51-77` | graph/source exact lines | reuse retrieval behind scoped adapter | measured fusion exists | coverage/score authority test |
| `crates/fleet-memory/src/promote.rs:53-75` | graph/source exact lines | reuse refusal ordering/confirmation concept | avoids unsafe promotion | require nonzero evidence/regression |
| Tantivy `0.26.1`, MIT, [upstream](https://github.com/quickwit-oss/tantivy); SQLite FTS5 [docs](https://www.sqlite.org/fts5.html) | research snapshot, local Tantivy evidence; versions can drift | lexical local retrieval and durable co-location | maintained primitives | `cargo test -p fleet-memory`; exact lock/license/smoke; FTS5 not locally verified |
| tree-sitter `0.24.7` local bindings, [upstream](https://github.com/tree-sitter/tree-sitter) | local manifest and research; upstream release may be newer | context source indexing only | existing parser | `cargo test -p fleet-context --test repomap_fixtures`; freshness/overlay tests |

Research says central standards may travel via Git or MCP, but MCP capability/version is not verified in this runtime; keep adapter optional and fail closed.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tantivy = "0.26.1"
# tree-sitter: see §7 table for version (0.24.7 local bindings)
```

## 8. Behavior matrix
### `sources` / promotion
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty query/scope -> refusal 7; no manifest pass |
| huge / negative | bounded limit/bytes; reject invalid limit/revision |
| duplicate / concurrent | idempotent digest+scope key; store CAS conflict, no overwrite |
| partial failure / timeout | manifest marks incomplete and blocks pass; candidate retained, not activated |
| stale / unavailable | expired/unpinned standard excluded with reason; unavailable store -> environment fault 3 |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `KnowledgeItem`, `Kind`, `Scope`, `KnowledgeError`, and `SourceManifest` types; re-export all public items; `cargo check -p knowledge` exits 0.
2. In `src/filter.rs`: Wrap `fleet-memory` retrieval with scope and expiry filtering; `cargo test -p knowledge out_of_scope_item_excluded` exits 0.
3. In `src/item.rs`: Add standards pin/signature/applicability validation with negative fixtures; `cargo test -p knowledge expired_standard_refused` exits 0.
4. In `src/promote.rs`: Add candidate promotion port requiring evidence/regression count threshold; `cargo test -p knowledge zero_evidence_candidate_not_promoted` exits 0.
5. In `tests/knowledge.rs`: Wire all three named integration tests end-to-end; `cargo test -p fleet-memory -p knowledge -p fleet-context --no-fail-fast` exits 0.

## 10. Test matrix
**Unit:** scope, expiry, digest, tombstone, deterministic ranking, promotion refusal.
**Integration/contract:** `offline -> knowledge -> context` with manifest/digest and `checked=16,total=16`.
**Hidden:** poisoned standard, branch policy widening, expired critical item, duplicate candidate, store crash.
**Property:** 256 generated items never cross scope; 128 manifests preserve `checked==total>0`; `384/384`.
**Differential:** compare retrieval projection to current `fleet-memory` for 32 fixtures; score differences are recorded, not silently accepted.
**Real-binary/effect:** real `fleet` context command must show source digest/revision and no activation side effect; blocked until proposed knowledge crate is composed.

### Named integration tests (these three must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `knowledge::tests::out_of_scope_item_excluded` | `KnowledgeStore` seeded with two items: one with `scope = "repo/a"`, one with `scope = "repo/b"`; `sources` called with `scope = "repo/a"` | `SourceManifest` contains exactly 1 item; the `scope = "repo/b"` item is absent; `checked == total == 1` | Removing the scope filter in `filter_by_scope` leaks both items; the length assertion fails |
| `knowledge::tests::expired_standard_refused` | `KnowledgeStore` seeded with a `Standard` whose `expires_at` is in the past | `sources` returns `SourceManifest` that does not include the expired standard; no panic | A stub that ignores `expires_at` includes the standard; the absence assertion fails |
| `knowledge::tests::zero_evidence_candidate_not_promoted` | `put_candidate` called with `KnowledgeItem { evidence_count: 0, kind: Kind::Lesson, ... }` | `Err(KnowledgeError::InsufficientEvidence)` is returned; no item is inserted into the store | A stub that always inserts fails — proves the evidence gate is real, not a pass-through |

## 11. Mutation targets and anti-stub proof
| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `filter_by_scope` in `src/filter.rs` | Remove the scope predicate; return all items regardless of scope | `knowledge::tests::out_of_scope_item_excluded` | Test asserts exactly 1 scoped item; the leaked cross-scope item makes `len() == 2` and fails |
| `check_expiry` in `src/filter.rs` | Always return `false` (never expired); pass all items | `knowledge::tests::expired_standard_refused` | Test expects the expired standard is absent; the mutation includes it and the absence assertion fails |
| `promote_candidate` in `src/promote.rs` | Accept `evidence_count == 0` as sufficient for promotion | `knowledge::tests::zero_evidence_candidate_not_promoted` | Test expects `Err(InsufficientEvidence)`; the mutation returns `Ok(())` and the error assertion fails |
| `sources` checked/total | Return `SourceManifest { checked: 0, total: 0 }` vacuously | denominator test | Partial or empty manifest cannot pass; zero-denominator gate must fail |
| digest/revision field | Omit `source_digest` from `SourceManifest` items | manifest contract test | Source traceability is a hard invariant; missing digest is a boundary violation |

Safety mutation floor: `caught/total >= 75%`; reviewer kills scope, expiry, and zero-evidence promotion mutants.

## 12. Verification recipe and denominators
```bash
cargo test -p knowledge --no-fail-fast
cargo clippy -p knowledge --all-targets -- -D warnings
find crates/knowledge -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-memory -p fleet-context -p knowledge --no-fail-fast
```
Expected property `384/384`, integration `16/16`, all manifests `checked==total>0`; Safety mutation floor: `caught/total >= 75%`; no file >80 lines. Central provider/MCP publication remains unverified.

## 13. Definition of done

- `cargo test -p knowledge --no-fail-fast` exits 0 with `test result: ok`.
- `knowledge::tests::out_of_scope_item_excluded` appears in test output and passes.
- `knowledge::tests::expired_standard_refused` appears in test output and passes.
- `knowledge::tests::zero_evidence_candidate_not_promoted` appears in test output and passes.
- `cargo clippy -p knowledge --all-targets -- -D warnings` exits 0.
- `find crates/knowledge -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- Property test output shows `384/384` (256 scope-isolation + 128 manifest denominator cases).
- Mutation floor: `caught/total >= 75%` for the 5 named mutation targets in §11.
- `grep -rn "activate\|policy_write" crates/knowledge/src/` exits 1 (zero hits — knowledge proposes only, never activates).
- No unverified MCP or provider capability is claimed in `Cargo.toml` or source comments.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a branch edit widened a trusted standard → applicability was resolved from untrusted repo config → load trusted pinned revision first and treat branch changes as proposals.
1. What exact evidence promotes a lesson? 2. Can a standard silently widen grants? 3. Does every manifest publish checked/total and freshness?
