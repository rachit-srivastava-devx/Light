# BLUEPRINT — `probe_learn`

## 1. Identity and LLD path
- Node id / label / tag: `probe_learn` / Learning-Retrieval Probe / `model,ungated`
- LLD authority: `docs/LLD/LLD.md §§6,11` and `docs/LLD/lld-full-detail.architecture.json:components[id=probe_learn]`
- Why: surface scoped prior lessons relevant to a material unknown.
- Incoming edges: `scan -> probe_learn`: canonical `RequirementInput`; the boundary adapter maps it
  to `LearnInput` with the approved memory scope and selected unknowns.
- Outgoing edges: `probe_learn -> questions`: `Vec<Question>` through the shared questions adapter.
- Build status: `partial` — `fleet-memory` retrieval exists; node-specific ungated boundary is not complete.

## 2. Responsibility and non-goals
**Owns:** scope-filtered retrieval observations and contradiction questions.
**Does not own:** lesson promotion, policy activation, question cap, grants, or model authority.

## 3. Boundary and authority
Untrusted memory is data. `MemoryPort` is read-only and injected. Retrieved lessons can suggest a question/context item but cannot widen capability, become a standard, or suppress current instructions. Parent records retrieval coverage and timeout.

## 4. Crate/package layout
`Cargo.toml` declares the node (≤25 lines); `src/lib.rs` exports it (≤30); `src/probe.rs` maps scoped hits (≤70); `src/port.rs` owns memory lookup (≤60); `tests/probe.rs` owns fixtures (≤80). All live under `crates/probe-learn/`.

## 5. Public API contract
```rust
pub struct LearnInput { pub query: String, pub scope: String, pub unknowns: Vec<String>, pub revision: u64 }
pub struct LessonHit { pub id: String, pub text: String, pub scope: String, pub confirmed_count: u32, pub evidence_ref: String }
pub trait MemoryReader: Send + Sync { fn recall(&self, query: &str, scope: &str, limit: u32) -> Result<Vec<LessonHit>, ProbeError>; }
pub fn probe(input: &LearnInput, reader: &dyn MemoryReader) -> Result<Vec<Question>, ProbeError>;
pub struct Question { pub text: String, pub evidence_ref: Option<String> }
pub enum ProbeError { InvalidInput, ScopeViolation, NoCoverage, Unavailable(String) }
```
Limit is positive and bounded; every hit must match scope or be dropped with a reason. The `scan` boundary adapter owns conversion from the canonical LLD request to `LearnInput` and from `Question` to the shared questions edge.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `LessonHit` | scope exact/prefix policy is explicit; id/evidence nonempty | cross-repo leakage | `ScopeViolation` / 6 |
| retrieval | `checked>0` when claiming recall coverage; missing is unknown | fabricated recall | `NoCoverage` / 8 |
| confirmation | integer count only; no zero substitution | false trust | typed refusal 7 |

Current `fleet-memory` uses `f32` relevance in `crates/fleet-memory/src/retrieve.rs:51-77`; this blueprint requires fixed/integer or opaque provider score for threshold decisions. That source is reuse evidence, not compliant final authority.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-scan/src/ports.rs:13-20` `MemoryPort` | local source | narrow recall seam | existing node-facing boundary | scope/limit contract |
| `crates/fleet-memory/src/retrieve.rs:51-77` | graph/source exact lines | reuse lexical/vector fusion behind adapter | existing measured retrieval tests | score/coverage and unknown semantics |
| `crates/fleet-memory/src/promote.rs:53-75` | graph/source exact lines | reference promotion refusals only | promotion belongs to knowledge, not probe | no promotion side effect |
| `tantivy = "0.26.1"`, MIT, [upstream](https://github.com/quickwit-oss/tantivy) | pinned; same version in `fleet-memory/fleet-context` Cargo.toml | BM25 full-text search for lexical fallback retrieval | maintained index; BM25 scoring proven in fleet-memory tests | `cargo test -p fleet-memory`; verify license and smoke at implementation |

Exact `Cargo.toml` snippet for `crates/probe-learn/Cargo.toml`:
```toml
[dependencies]
fleet-scan  = { path = "../../crates/fleet-scan" }    # optional boundary adapter only
tantivy        = "0.26.1"
serde          = { version = "1", features = ["derive"] }
thiserror      = "2"
tokio          = { version = "1", features = ["rt-multi-thread", "macros"] }
```
These versions are pinned. Do not substitute `"*"` or a range — a version drift breaks the fleet-memory interop tests that are the adoption proof.

## 8. Behavior matrix
### `probe`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty query/scope -> refusal 7; no memory query |
| huge / negative | clamp only at validated maximum; reject invalid limit |
| duplicate / concurrent | dedupe by lesson id; deterministic id tie-break |
| partial failure / timeout | return missing-evidence question/fault; never “no lessons” |
| stale / unavailable | stale snapshot marked unknown; no promotion or policy write |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `LearnInput`, `LessonHit`, `ProbeError`, and `MemoryReader` trait; re-export all public items; `cargo check -p probe-learn` exits 0.
2. In `src/port.rs`: Implement `retrieve_relevant_lessons` around `MemoryPort` with a positive bounded limit and scope enforcement; `cargo test -p probe-learn ambiguity_request_retrieves_scoped_lessons` exits 0.
3. In `src/probe.rs`: Implement `format_learning_prompt` and `map_to_questions` to produce evidence-bearing `Question` output; `cargo test -p probe-learn no_matching_lessons_returns_question` exits 0.
4. In `src/probe.rs`: Add unavailable/empty coverage refusal (missing evidence is `NoCoverage`, not empty `Vec`); `cargo test -p probe-learn returned_question_differs_by_scope` exits 0.
5. In `tests/probe.rs`: Wire all three named integration tests end-to-end; `cargo test -p fleet-memory -p probe-learn --no-fail-fast` exits 0 with `checked=8,total=8`.

## 10. Test matrix
**Unit:** scope, dedup, confirmation/provenance, empty vs unavailable distinction.

**Integration/contract:** `scan -> probe_learn -> questions` and `fleet-memory` adapter; `checked=8,total=8`.

Named integration tests (all three must exist verbatim in `tests/probe.rs`):

| Test name | Inputs | Expected output | Mutation caught |
|---|---|---|---|
| `probe_learn::tests::ambiguity_request_retrieves_scoped_lessons` | `LearnInput { query: "ownership and borrowing in Rust", scope: "rust/borrowing", unknowns: ["ownership"], revision: 1 }`; store seeded with one lesson whose text contains `"ownership"` and whose scope is `"rust/borrowing"` | `Vec<Question>` has length ≥1; the first question `text` references `"ownership"` | `probe` mutated to ignore returned lessons — question text loses the lesson topic and the assertion fails |
| `probe_learn::tests::no_matching_lessons_returns_question` | `LearnInput { query: "some requirement text", scope: "general", unknowns: ["lessons"], revision: 1 }`; store is empty | `Vec<Question>` has length exactly 1; question text is nonempty | stub that returns `vec![]` — the test fails on length assertion |
| `probe_learn::tests::returned_question_differs_by_scope` | Two calls with the same query but `scope: "alpha"` and `scope: "beta"` | The `text` fields of the returned questions are not equal | `format_learning_prompt` mutated to omit scope — both calls produce identical text and `assert_ne!` fails |

**Hidden:** cross-tenant hit, poisoned lesson, zero-confirmation lesson, stale revision.
**Property:** fixed seed 128 hit sets never cross scope and preserve idempotence; `128/128`.
**Differential:** compare adapter output to current lexical retrieval for 32 fixtures; score ordering divergence must be reported, not hidden.
**Real-binary/effect:** real binary read-only retrieval is blocked until node wiring and a durable memory snapshot are proven.

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `probe` in `src/probe.rs` | Ignore reader hits and always format an empty-hit question | `probe_learn::tests::ambiguity_request_retrieves_scoped_lessons` | Question text must reference the seeded lesson topic; ignoring the hit fails the substring assertion |
| `format_learning_prompt` in `src/probe.rs` | Omit the `scope` field from the prompt string | `probe_learn::tests::returned_question_differs_by_scope` | Both scope-differing calls produce identical text; the `assert_ne!(q_a.text, q_b.text)` assertion fails |
| `map_to_questions` in `src/probe.rs` | Hardcode a fixed question body string regardless of input | `probe_learn::tests::returned_question_differs_by_scope` | Different scopes must produce different outputs; a constant return kills the `assert_ne` |
| scope filter in `src/port.rs` | Return all hits regardless of scope | cross-scope hidden test | Memory cannot expand authority; cross-scope items must be dropped |
| unavailable mapping in `src/probe.rs` | Map `NoCoverage`/unavailable error to empty `Vec` | fault-distinction test | Missing evidence is not the same as no prior lessons; the distinction drives different downstream handling |
| confirmed-count check in `src/probe.rs` | Substitute `0` for a missing confirmation count | unknown-count test | No fabricated confirmation; zero must be typed as unknown, not as a real zero-confirmation count |

Safety mutation floor: `caught/total >= 75%`; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators
```bash
cargo test -p probe-learn --no-fail-fast
cargo clippy -p probe-learn --all-targets -- -D warnings
find crates/probe-learn -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
cargo test -p fleet-memory -p probe-learn --no-fail-fast
# Mutation floor: caught/total >= 75%
```
Expected adapter `8/8`, property `128/128`, retrieval coverage `checked>0,total>0`; no file >80 lines. Current vector/provider status is not verified.

## 13. Definition of done

Each item is a checkable assertion — tick only when the evidence is present.

- [ ] `cargo test -p probe-learn --no-fail-fast` exits 0; output shows `probe_learn::tests::ambiguity_request_retrieves_scoped_lessons`, `probe_learn::tests::no_matching_lessons_returns_question`, and `probe_learn::tests::returned_question_differs_by_scope` as `ok`.
- [ ] `cargo test -p fleet-memory -p probe-learn --no-fail-fast` exits 0 and shows retrieval coverage `checked=8,total=8`.
- [ ] `cargo clippy -p probe-learn --all-targets -- -D warnings` exits 0.
- [ ] No source file under `crates/probe-learn/` exceeds the line cap in §4 (lib.rs ≤30, probe.rs ≤70, port.rs ≤60, tests/probe.rs ≤80).
- [ ] `crates/probe-learn/Cargo.toml` pins `tantivy = "0.26.1"`, `serde`, `thiserror = "2"`, `tokio` exactly as shown in §7; no ranges or `"*"`.
- [ ] Mutating `retrieve_relevant_lessons` to return `vec![]` causes `ambiguity_request_retrieves_scoped_lessons` to fail (reviewer verifies manually).
- [ ] Mutating `format_learning_prompt` to omit the scope causes `returned_question_differs_by_scope` to fail (reviewer verifies manually).
- [ ] Mutating `map_to_questions` to return a constant string causes `returned_question_differs_by_scope` to fail (reviewer verifies manually).
- [ ] `no_matching_lessons_returns_question` asserts `Vec<Question>.len() == 1`; the node never returns an empty Vec.
- [ ] All `LessonHit` values in integration tests match scope exactly; no cross-scope hits appear in any passing test result.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a local workaround became universal policy → probe output bypassed promotion/evaluation → make retrieval proposal-only and require knowledge promotion gates.
1. Can a memory hit grant capability? 2. How is unavailable distinguished from no hit? 3. Is `f32` used as an authority threshold?
