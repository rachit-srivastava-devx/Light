# BLUEPRINT — `context`

## 1. Identity and LLD path
- Node id / label / tag: `context / Context Compiler / deterministic`
- LLD authority: `docs/LLD/LLD.md §10` and `docs/LLD/lld-full-detail.architecture.json:components[id=context]`
- Why this node exists: compile exact authority and bounded evidence into a digest-bound manifest.
- Incoming edges: `knowledge -> context` (`manifest source`); task/plan/source snapshot.
- Outgoing edges: `context -> planner`, `builder`, `review` (`compiled manifest`).
- Build status: `partial` — `crates/fleet-context/src/retrieve.rs:26-62` is a retrieval seam, not the full compiler.

## 2. Responsibility and non-goals
**Owns:** scope filtering, graph/lexical retrieval, ranking, dependency expansion, deduplication, exact budget accounting, packing, omissions, and manifest digest.

**Does not own:** policy/grants, model choice, plan approval, file mutation, or test verdicts; those belong to `knowledge/control`, `route`, `plan-review`, `builder`, and `verify`.

## 3. Boundary and authority
Deterministic and port-driven. Mandatory objective, instructions, acceptance, grants, plan, immutable source pointers, and open questions are first; retrieved prose cannot override them. The compiler may return `ContextManifest` or a typed refusal. Unknown tokenizer/window is conservative estimate, never exact. Large results require bounded stored originals or explicit incomplete coverage.

## 4. Crate/package layout
```text
crates/context/
  Cargo.toml
  src/lib.rs                 # ≤40 lines
  src/compile.rs             # ≤80 lines
  src/manifest.rs            # ≤80 lines
  src/budget.rs              # ≤80 lines
  tests/manifest.rs          # ≤80 lines
```
`src/` wires named ports only; no hidden store or model dependency.

> **Crate status:** `context` is a NEW crate at `crates/context/`. Do not confuse with the existing `crates/fleet-context/` — that crate is a separate existing module. The §7 references to fleet-context source are extraction/reuse candidates, not the build target.

## 5. Public API contract
```rust
pub trait Retriever: Send + Sync { fn retrieve(&self, q: &RetrievalQuery) -> Result<Vec<Evidence>, ContextError>; }
pub fn compile(input: &CompileInput, r: &dyn Retriever, count: &dyn TokenCounter) -> Result<ContextManifest, ContextError>;
pub struct CompileInput { pub task_digest: String, pub plan_digest: String, pub base_digest: String, pub mandatory: Vec<Span>, pub budget: u64, pub reserve: u64 }
pub struct ContextManifest { pub digest: String, pub mandatory: Vec<Span>, pub evidence: Vec<EvidenceRef>, pub omitted: Vec<String>, pub checked: u64, pub total: u64 }
```
Precondition: `reserve < budget`, mandatory spans have source digests/ranges. Postcondition: `checked == total > 0` for included evidence and `mandatory` is retained. No panic.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `ContextManifest` | plan/base/tokenizer/digest recorded | stale or unverifiable context | `Stale` / 8 |
| `EvidenceRef` | source range + trust + digest | unattributed prose | `MissingProvenance` / 6 |
| budget | checked integer arithmetic; reserve protected | overflow/overfill | `BudgetExceeded` / 7 |
| coverage | `checked>0`, `checked==total` for pass | empty green gate | `ZeroCoverage` / 6 |

Filesystem/index/network/token ports are injected; compiler is `Send + Sync` if ports are. Embeddings are optional and currently absent/BM25-only; no provider capability claim.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-context/src/retrieve.rs:26-62` | local exact source; partial evidence | reuse BM25/vector fusion and compaction seam | preserve tested retrieval behavior | integration with manifest |
| `tree-sitter` workspace pins `0.24.7`; MIT; research observed 0.27.0 upstream | local manifest vs current research | repo symbol map | parser is maintained primitive | pin/digest/grammar smoke |
| `tantivy 0.26.1`, MIT; local tested | research ledger | BM25 | no handwritten index | indexed/queried denominator |
| `tiktoken-rs 0.12.0`, MIT; local tested | research ledger | conservative token counts | provider-independent fallback | encoding/source receipt |

No custom implementation is introduced where the listed crate already provides the facility.

### Cargo.toml snippet (pinned)
```toml
[dependencies]
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror  = "2"
tokio      = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 8. Behavior matrix
### `compile`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | refuse if mandatory set or query is empty; no `0/0` success |
| wrong type / Unicode | typed parse/provenance error; preserve valid Unicode |
| huge / negative | checked overflow and bounded chunk count; negative budget refuses |
| duplicate / concurrent | dedup by source digest/range; immutable same input yields same digest |
| partial failure / timeout | omit only optional evidence with omission reason; mandatory failure refuses |
| stale / unavailable | stale index is marked and refused when required; unavailable vector port falls back only if policy permits |

## 9. Tiny implementation steps
1. In `src/lib.rs` and `src/manifest.rs`: define `CompileInput`, `ContextManifest`, `EvidenceRef`, `Span`, and `ContextError` types → `cargo check -p context` exits 0.
2. In `src/compile.rs`: port existing `retrieve_context` behind the `Retriever` port → add `context::tests::knowledge_to_context_manifest` and run `cargo test -p context knowledge_to_context_manifest` exits 0.
3. In `src/budget.rs`: add mandatory-first packer with checked integer budget arithmetic — reject overfill and overflow → add `context::tests::budget_overflow_refused` and run `cargo test -p context budget_overflow_refused` exits 0.
4. In `src/compile.rs`: add manifest digest, omission list, and coverage check — reject zero-input `0/0` → add `context::tests::zero_coverage_refused` and run `cargo test -p context zero_coverage_refused` exits 0.
5. In `tests/manifest.rs`: run real fleet context path → `target/debug/fleet context --fixture tests/fixtures/blueprint-context/minimal.json --json` output contains `checked=N,total=N` with N>0; manifest digest and source ranges appear in output.

## 10. Test matrix
**Unit tests:** budget arithmetic, dedup, mandatory ordering, stale snapshot.

**Integration/contract tests:** `knowledge_to_context_manifest` and `context_to_builder` with serialized digest.

**Hidden tests:** missing source, parser recovery, vector unavailable, Unicode path, exact budget boundary.

**Property tests:** packing never exceeds `budget-reserve`; fixed seed, 300 cases.

**Differential tests:** existing `retrieve_context` fixture results versus new manifest evidence; normalize ranking ties only.

**Real-binary/effect test:** `target/debug/fleet context ...`; assert manifest artifact and nonzero coverage. No network proof.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `context::tests::knowledge_to_context_manifest` | `CompileInput` with nonempty `task_digest`, `plan_digest`, `base_digest`, mandatory spans with source digests, and mock `Retriever` returning two `Evidence` items | `ContextManifest` with nonzero `checked`/`total`, all mandatory spans present, and a non-empty `digest` | catches empty-manifest mutation — any stub that returns an empty manifest fails the checked/digest assertion |
| `context::tests::context_to_builder` | `ContextManifest` from the previous test serialized to JSON and re-parsed | round-trip preserves all fields including `digest`, `mandatory`, and `evidence` references | catches digest-drop mutation — stripping the digest field from the serialized manifest breaks the equality assertion |
| `context::tests::stale_knowledge_ref_refused` | `CompileInput` with a `base_digest` that does not match the mock retriever's indexed snapshot | `Err(ContextError::Stale)` returned; no manifest emitted | catches stale-check removal — any stub that skips the staleness guard returns `Ok` and the test goes red |

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `compile` in `src/compile.rs` | Return an empty `ContextManifest` with `checked=0, total=0` for any input | `context::tests::knowledge_to_context_manifest` | test requires `checked > 0` and a non-empty digest; the empty-manifest mutant satisfies neither assertion |
| `assemble_manifest` in `src/manifest.rs` | Omit the `digest` field — return a manifest with an empty digest string | `context::tests::context_to_builder` | test round-trips through serialization and asserts digest equality; an empty digest fails the assertion |
| `compile` in `src/compile.rs` | Skip the staleness check — compile even when `base_digest` does not match the index | `context::tests::stale_knowledge_ref_refused` | test sends a mismatched base digest; removing the staleness guard makes `compile` return `Ok` instead of `Err(Stale)` |
| `compile` in `src/compile.rs` | Drop `reserve` subtraction from budget accounting | boundary budget test | output reserve is enforced; the packer overfills and the budget assertion fails |
| `compile` in `src/compile.rs` | Remove source digest from `EvidenceRef` | provenance contract test | evidence must be auditable; stripping the digest fails the provenance assertion |

Safety mutation floor: `caught/total >= 75%`. Reviewer manually removes mandatory-first ordering and confirms `knowledge_to_context_manifest` goes red.

## 12. Verification recipe and denominators
```bash
cargo test -p context --no-fail-fast
cargo clippy -p context --all-targets -- -D warnings
find crates/context -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-context --no-fail-fast
target/debug/fleet context --fixture tests/fixtures/blueprint-context/minimal.json --json
```
Publish `indexed=n,queried=m,checked=k,total=k` with all positive where pass; record tokenizer and tool versions. Current full workspace/production evidence is unverified.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p context --no-fail-fast` exits 0 with `test result: ok` in output.
- `context::tests::knowledge_to_context_manifest` appears in test output and passes.
- `context::tests::context_to_builder` appears in test output and passes.
- `cargo clippy -p context --all-targets -- -D warnings` exits 0 (zero warnings).
- `find crates/context -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `target/debug/fleet context --fixture tests/fixtures/blueprint-context/minimal.json --json` output contains `checked=N,total=N` with `N>0`.
- Mutation floor: `caught/total >= 80%` for the 3 named mutation targets in §11 (empty manifest, reserve-drop, digest-drop).
- `Cargo.toml` pins `serde` and `thiserror` exactly as shown in §7.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** compacted context silently omits an acceptance criterion → mandatory set was not separately tracked → fail closed and publish omission/coverage.

1. Can missing index input pass? 2. Is token count provider usage or local estimate? 3. Can retrieved prose override instructions?
