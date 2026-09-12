# BLUEPRINT — `probe_tech`

## 1. Identity and LLD path
- Node id / label / tag: `probe_tech` / Technical-Context Probe / `model,ungated`
- LLD authority: `docs/LLD/LLD.md §6` and `docs/LLD/lld-full-detail.architecture.json:components[id=probe_tech]`
- Why: identify repository constraints, symbols, tests, and technical unknowns.
- Incoming edges: `scan -> probe_tech`: canonical `RequirementInput`; the boundary adapter maps it
  to `TechnicalInput` with the repo snapshot and selected unknowns.
- Outgoing edges: `probe_tech -> questions`: `Vec<Question>` through the shared questions adapter.
- Build status: `partial` — `CodebasePort` and `Probe` seams exist; complete node is absent.

## 2. Responsibility and non-goals
**Owns:** read-only technical observations and grounded candidate questions.
**Does not own:** code edits, graph authority, acceptance, routing, or question cap.

## 3. Boundary and authority
Read-only injected `CodebasePort`; repository content is untrusted. A model may summarize exact file/symbol evidence but cannot execute arbitrary commands or treat AST edges as runtime proof. Parent enforces scope, deadlines, and output bounds.

## 4. Crate/package layout
`Cargo.toml` declares the node (≤25 lines); `src/lib.rs` exports it (≤30); `src/probe.rs` validates facts (≤70); `src/port.rs` owns graph lookup (≤60); `tests/probe.rs` owns fixtures (≤80). All live under `crates/probe-tech/`.

## 5. Public API contract
```rust
pub struct TechnicalInput { pub request: String, pub unknowns: Vec<String>, pub repo_digest: String, pub revision: u64 }
pub struct SymbolFact { pub query: String, pub path: String, pub start_line: u32, pub end_line: u32, pub exists: bool }
pub trait CodebaseReader: Send + Sync { fn symbol(&self, query: &str) -> Result<SymbolFact, ProbeError>; }
pub struct Question { pub text: String, pub evidence: Option<String> }
pub fn probe(input: &TechnicalInput, reader: &dyn CodebaseReader) -> Result<Vec<Question>, ProbeError>;
pub enum ProbeError { InvalidInput, InvalidFact, StaleSnapshot, Unavailable(String) }
```
Preconditions include nonempty digest and bounded queries; postcondition every fact has a repo digest and line range or explicit `exists=false`. The `scan` boundary adapter owns conversion from the canonical LLD request to `TechnicalInput` and from `Question` to the shared questions edge.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `SymbolFact` | line range is ordered and path is relative to declared repo | fabricated source | `InvalidFact` / 6 |
| `TechnicalInput` | digest/revision bind all reads | stale evidence | `StaleSnapshot` / 6 |
| reader | no shell/network; bounded query count | command injection | environment fault/refusal 3/7 |

No floats for thresholds; line/count/bytes are integers. Concurrent reads are safe if the port is `Send + Sync`.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-scan/src/ports.rs:5-10` `CodebasePort` | local source | narrow symbol existence seam | already expresses technical lookup | digest-bound adapter test |
| codebase-memory graph `search_graph`/`get_code_snippet` | current MCP tool, capability available here | discovery during blueprinting and future adapter | graph-first repository rule | local adapter receipt; MCP version/capability currently not pinned |
| `tree-sitter = "0.24.7"` + `tree-sitter-rust = "0.23.3"` ([upstream](https://github.com/tree-sitter/tree-sitter)) | MIT; `docs/blueprints/_research/model-context-planning.md` | optional AST evidence through context service; grammar crate for Rust symbol extraction | maintained parser, no custom parser | `cargo test -p fleet-context --test repomap_fixtures`; grammar/version smoke |
| `serde = { version = "1", features = ["derive"] }` | MIT OR Apache-2.0; crates.io | serialize `TechnicalInput`/`Vec<Question>`/`SymbolFact` across node boundary | mature, universal, derive macro eliminates boilerplate | `cargo check -p probe-tech` |
| `thiserror = "2"` | MIT OR Apache-2.0; crates.io | derive `ProbeError` variants with structured Display messages | no hand-rolled Display or match-arm strings | `cargo check -p probe-tech` |
| `tokio = { version = "1", features = ["rt-multi-thread", "macros"] }` | MIT; crates.io | async executor for concurrent reads behind `CodebaseReader`; `#[tokio::test]` in test harness | runtime already in fleet tree; not duplicated | `cargo test -p probe-tech` |

**Exact `Cargo.toml` `[dependencies]` snippet — copy verbatim, do not substitute other versions:**
```toml
[dependencies]
fleet-scan  = { path = "../../crates/fleet-scan" }    # optional boundary adapter only
serde         = { version = "1", features = ["derive"] }
thiserror     = "2"
tokio         = { version = "1", features = ["rt-multi-thread", "macros"] }
tree-sitter   = "0.24.7"
tree-sitter-rust = "0.23.3"
```

No claim that graph MCP is available in a deployed Fleet binary; current evidence is development-time only.

## 8. Behavior matrix
### `probe`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty query/digest -> refusal 7 |
| huge / negative | cap query count/bytes; reject negative line/limit |
| duplicate / concurrent | dedupe identical queries; stable order under concurrent reads |
| partial failure / timeout | retain successful facts, emit fault/question for missing ones |
| stale / unavailable | digest mismatch or unavailable graph -> missing evidence, never clear |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `TechnicalInput`, `SymbolFact`, `ProbeError`, and `CodebaseReader` trait; re-export all public items; `cargo check -p probe-tech` exits 0.
2. In `src/probe.rs`: Implement query cap and relative-path/line-range validation; `cargo test -p probe-tech ambiguity_request_produces_question` exits 0.
3. In `src/probe.rs`: Map codebase facts into `Question` evidence bodies using `extract_code_context`; `cargo test -p probe-tech question_references_language_construct` exits 0.
4. In `src/port.rs`: Inject unavailable/stale `CodebaseReader` producing typed refusals; `cargo test -p probe-tech empty_file_context_returns_question` exits 0.
5. In `tests/probe.rs`: Wire all three named integration tests end-to-end; `cargo test -p probe-tech --no-fail-fast` exits 0 with `checked=8,total=8`.

## 10. Test matrix
**Unit:** path/line/digest validation, query caps, stable dedup.
**Integration/contract:** `scan -> probe_tech -> questions`, with `CodebasePort` adapter and exact evidence refs; `checked=8,total=8`.

**Named integration tests (all three must be present in `tests/probe.rs` and pass):**

| Test name | Inputs | Expected output | Mutation caught |
|---|---|---|---|
| `probe_tech::tests::ambiguity_request_produces_question` | `TechnicalInput { request: "Implement authentication module; see src/auth.rs for existing types", unknowns: ["dependency"], repo_digest: "abc123", revision: 1 }` | `Vec<Question>` with exactly 1 element; `questions.len() == 1`; body is non-empty | `probe` mutated to return an empty `Vec`; test fails on `assert_eq!(questions.len(), 1)` |
| `probe_tech::tests::question_references_language_construct` | Same `TechnicalInput` fixture as above | question body contains at least one of: `"function"`, `"struct"`, `"trait"`, `"module"`, `"dependency"`, `"interface"`, `"type"` | `extract_code_context` mutated to return `""` (empty string); body loses all language terms; assertion `any(|term| body.contains(term))` fails |
| `probe_tech::tests::empty_file_context_returns_question` | `TechnicalInput { request: "", unknowns: ["module"], repo_digest: "abc123", revision: 1 }` | exactly 1 `Question` returned — no early-return stub for empty input; `questions.len() == 1` | early-return stub `if context.is_empty() { return Ok(vec![]) }` is inserted; test fails because 0 questions returned instead of 1 |

**Hidden:** symlink/path traversal, poisoned source comment, stale graph, malformed symbol response.
**Property:** 128 generated facts preserve digest and ordered ranges; `128/128`.
**Differential:** compare adapter normalization with direct graph snippet for 16 fixtures; divergence is explicit on dynamic edges.
**Real-binary/effect:** real `fleet` read-only task; blocked until composition wiring and an installed graph adapter are proven.

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `probe` in `src/probe.rs` | Return empty `Vec` regardless of input | `probe_tech::tests::ambiguity_request_produces_question` | `assert_eq!(questions.len(), 1)` fails; proves the node is not an empty-result stub |
| `extract_code_context` in `src/probe.rs` | Return `""` (empty string) unconditionally | `probe_tech::tests::question_references_language_construct` | No language-construct term survives in the body; `any(term)` assertion fails; proves context extraction drives question content |
| `generate_tech_question` in `src/probe.rs` | Hardcode a fixed string return regardless of input | `probe_tech::tests::ambiguity_request_produces_question` run with two distinct request fixtures | Both inputs produce identical question body; `assert_ne!(q_a.text, q_b.text)` fails; proves input-sensitivity |
| repo-digest binding in `src/port.rs` | Drop `repo_digest` from `SymbolFact`; omit snapshot check | snapshot contract hidden test | Stale facts must be rejected; a missing digest means any stale index passes |
| absolute-path guard in `src/probe.rs` | Accept absolute paths in `SymbolFact.path` | traversal hidden test | Scope containment is a safety boundary; an absolute path can escape the repo |
| zero-symbol guard in `src/port.rs` | Examine zero symbols and return a passing result | coverage denominator test | Zero examined inputs cannot masquerade as success; the gate must fail on zero |

Safety mutation floor: `caught/total >= 75%`; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators
```bash
cargo test -p probe-tech --no-fail-fast
cargo clippy -p probe-tech --all-targets -- -D warnings
find crates/probe-tech -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
# Mutation floor: caught/total >= 75%
```
Expected `8/8` contract and `128/128` property cases; no file >80 lines. MCP/provider capability and real binary are unverified until smoke-tested.

## 13. Definition of done

Each item is a shell assertion that must exit 0 in a clean checkout; qualitative confidence is not sufficient.

- `cargo test -p probe-tech --no-fail-fast 2>&1 | grep "test result" | grep -E "[0-9]+ passed"` exits 0 — denominator ≥8 tests pass, none skipped.
- `grep -E "fn (ambiguity_request_produces_question|question_references_language_construct|empty_file_context_returns_question)" crates/probe-tech/tests/probe.rs` exits 0 — all three named integration tests are present in source.
- `cargo clippy -p probe-tech --all-targets -- -D warnings` exits 0 — no compiler warnings.
- `cargo test -p probe-tech --no-fail-fast -- --list 2>&1 | grep -c " test$"` prints ≥8 — honest denominator, not zero.
- `grep -rn "digest" crates/probe-tech/src/` exits 0 and matches ≥1 line — every read path references repo digest in source.
- Mutation gate (manual reviewer step): replace `probe` with `Ok(vec![])` and run `cargo test -p probe-tech`; `ambiguity_request_produces_question` must fail — if it passes, the test suite does not prove behavior.
- `cargo check -p probe-tech` exits 0 with the exact `Cargo.toml` versions from §7 — no version substitution allowed.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a stale index said a symbol existed → snapshot digest was omitted → bind every result to repo digest and refuse mismatch.
1. Is graph evidence being confused with runtime dependency proof? 2. Can a path escape the repo? 3. What command proves the adapter is real?
