# BLUEPRINT — `probe_business`

## 1. Identity and LLD path
- Node id / label / tag: `probe_business` / Business-Context Probe / `model,ungated`
- LLD authority: `docs/LLD/LLD.md §6` and `docs/LLD/lld-full-detail.architecture.json:components[id=probe_business]`
- Why: extract stakeholder, outcome, constraint, and acceptance context for a material unknown.
- Incoming edges: `scan -> probe_business`: canonical `RequirementInput`; the boundary adapter
  maps it to `BusinessInput` and selected unknowns.
- Outgoing edges: `probe_business -> questions`: `Vec<Question>`; no effect edge.
- Build status: `partial` — current `fleet-scan` has a named business probe slot, not this isolated node contract.

## 2. Responsibility and non-goals
**Owns:** bounded business-context proposal with source/evidence references and candidate questions.
**Does not own:** materiality, user questioning, acceptance, grant checks, writes, or workflow routing.

## 3. Boundary and authority
Untrusted model/read adapter. It may read explicitly supplied requirement/business documents through an injected port and return bounded context/questions. It cannot decide that ambiguity is immaterial, authorize effects, persist memory, or call arbitrary network endpoints. Parent validates size, provenance, and timeout.

## 4. Crate/package layout
`Cargo.toml` declares the node (≤25 lines); `src/lib.rs` exports it (≤30); `src/probe.rs` maps business evidence (≤70); `src/port.rs` owns the reader port (≤60); `tests/probe.rs` owns fixtures (≤80). All live under `crates/probe-business/`.

## 5. Public API contract
```rust
pub struct BusinessInput { pub request: String, pub unknown_fields: Vec<String>, pub revision: u64 }
pub struct BusinessEvidence { pub source: String, pub excerpt: String }
pub struct BusinessContext { pub outcomes: Vec<String>, pub stakeholders: Vec<String>, pub constraints: Vec<String>, pub evidence: Vec<BusinessEvidence> }
pub trait BusinessReader: Send + Sync { fn read(&self, query: &str, max_bytes: u64) -> Result<Vec<BusinessEvidence>, ProbeError>; }
pub fn probe(input: &BusinessInput, reader: &dyn BusinessReader) -> Result<Vec<Question>, ProbeError>;
pub struct Question { pub text: String, pub why: String, pub evidence: Option<String> }
pub enum ProbeError { InvalidInput, InvalidEvidence, BudgetExceeded, Unavailable(String) }
```
Precondition: request/revision/nonempty unknowns are bounded. Postcondition: output is bounded, source-tagged, and proposal-only; no panic. Existing `fleet-scan::ProbeOutcome` and `Question` are reuse candidates, not this crate's contract; an adapter must be explicit if they are adopted.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `BusinessEvidence` | source and excerpt nonempty; bytes ≤ configured cap | invented grounding | `InvalidEvidence` / 6 |
| `BusinessContext` | total items and bytes bounded; no effect fields | model authorizing work | `BudgetExceeded` / 6 |
| reader call | one injected call within deadline | ambient network | timeout/env fault / 3 |

Counts/bytes are integers; unknown is a reasoned absent field, never empty success. Model identity and token usage are observations, not authority.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-scan/src/probe.rs:8-22` | local source | reuse `Probe`/`ProbeOutcome` shape | existing fault isolation contract | adapter contract test |
| `crates/fleet-scan/src/probe.rs:25-37` | local source | reuse `Question` provenance fields | avoids parallel question schema | negative-field tests |
| `serde` 1.x / `thiserror` 2.0.20, MIT/Apache-2.0, [Serde](https://github.com/serde-rs/serde), [thiserror](https://github.com/dtolnay/thiserror) | current workspace | wire/error types | maintained primitives | `cargo test -p probe-business`; locked version and schema smoke |
| provider SDK/model | no verified current provider capability | optional adapter only | avoid embedding an unverified provider | real provider probe required |

**Exact `[dependencies]` block — copy this into `crates/probe-business/Cargo.toml`:**
```toml
[package]
name = "probe-business"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-scan  = { path = "../../crates/fleet-scan" }    # optional boundary adapter only
serde       = { version = "1", features = ["derive"] }
thiserror   = "2"
tokio       = { version = "1", features = ["rt-multi-thread", "macros"] }

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "test-util"] }
```

**Do NOT** add `anthropic-sdk` or any provider SDK as a direct dependency. Model calls go through an injected `BusinessReader` port trait that the caller supplies. The port is defined in this crate. A `FakeBusinessReader` mock is used in tests.

## 8. Behavior matrix
### `probe`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty request/unknown -> typed refusal 7; missing evidence -> question with reason |
| huge / negative | cap bytes/items; negative limits reject at parse |
| duplicate / concurrent | dedupe evidence by source+digest; concurrent calls isolated by parent |
| partial failure / timeout | return bounded `Fault`/missing-evidence question; never fabricate context |
| stale / unavailable | revision mismatch or reader unavailable -> typed fault; no direct fallback write |

## 9. Tiny implementation steps
1. In `src/lib.rs`: define `BusinessInput`, `BusinessEvidence`, `BusinessContext`, `Question`, and `ProbeError` types → run `cargo check -p probe-business`.
2. In `src/port.rs`: implement `BusinessReader` port with byte/item cap enforcement → add `probe_business::tests::oversized_evidence_is_refused`; run `cargo test -p probe-business port`.
3. In `src/probe.rs`: implement named helpers `build_prompt` and `parse_response`; map only business fields to `Context`/`Questions` output with no effect fields → add `probe_business::tests::effect_field_absent_in_output` and `probe_business::tests::ambiguity_request_produces_question`; run `cargo test -p probe-business probe`.
4. In `src/probe.rs`: add timeout/fault normalization for `BusinessReader` errors returning a bounded `Fault` rather than empty success → add `probe_business::tests::failing_reader_returns_fault`; run `cargo test -p probe-business`.
5. In `tests/probe.rs`: wire integration fixtures `empty_context_still_returns_question` and `question_body_contains_domain_term` → run `cargo test -p probe-business --no-fail-fast`; output shows `checked=10,total=10`.

## 10. Test matrix
**Unit:** empty/oversize/dedup/provenance and no-effect-field assertions.
**Integration/contract:** `scan -> probe_business -> questions` with a fake reader only for parser contract; `checked=6,total=6`.

### Named integration tests (these 3 must compile and pass)

| Test name (exact) | Inputs | Expected output | Mutation / anti-stub caught |
|---|---|---|---|
| `probe_business::tests::ambiguity_request_produces_question` | `BusinessInput { request: "Build a payment gateway; e-commerce startup context", unknown_fields: ["stakeholder"], revision: 1 }` with `FakeBusinessReader` returning bounded evidence | `Vec<Question>` with `len() == 1` and `question.text` nonempty | `probe` mutated to return an empty question vector — len assert fails |
| `probe_business::tests::empty_context_still_returns_question` | `BusinessInput { request: "Deploy service", unknown_fields: ["constraint"], revision: 1 }`; reader returns no evidence | `Vec<Question>` with `len() == 1` | guards against an early return that treats empty context as successful certainty |
| `probe_business::tests::question_body_contains_domain_term` | `BusinessInput { request: "Redesign onboarding flow; B2B SaaS product context", unknown_fields: ["scope"], revision: 1 }` | `question.text` contains at least one of: `"stakeholder"`, `"requirement"`, `"constraint"`, `"scope"`, `"budget"` (case-insensitive) | `build_prompt` mutated to omit `request` — model output loses domain-relevant grounding |

Fixture pattern for all three tests:
```rust
struct FakeBusinessReader { evidence: Vec<BusinessEvidence> }
impl BusinessReader for FakeBusinessReader {
    fn read(&self, _query: &str, _max_bytes: u64) -> Result<Vec<BusinessEvidence>, ProbeError> {
        Ok(self.evidence.clone())
    }
}
```
Each test constructs a `FakeBusinessReader` with bounded evidence, calls `probe(&input, &reader)`, and asserts on the returned `Vec<Question>`. A fake reader is permitted only for the port contract; the real-binary/effect proof remains mandatory.

**Hidden:** prompt injection in evidence, malformed model JSON, extra authorization fields, timeout.
**Property:** fixed seed 128 generated evidence records never exceed byte/item caps; `checked=128,total=128`.
**Differential:** compare normalization against the boundary adapter's `ProbeOutcome` serializer; no legacy semantic-equivalence claim.
**Real-binary/effect:** `./target/debug/fleet` read-only probe fixture; blocked until node wiring/provider adapter exists.

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `probe` in `src/probe.rs` | Return `vec![]` regardless of input | `probe_business::tests::ambiguity_request_produces_question` | Test asserts `questions.len() == 1`; empty-return mutant fails immediately |
| `build_prompt` in `src/probe.rs` | Omit `request` from the prompt string sent to the reader | `probe_business::tests::question_body_contains_domain_term` | Test asserts the returned question contains a domain term derived from input; removing the request loses domain grounding |
| `parse_response` in `src/probe.rs` | Always return the same hardcoded `Question { text: "placeholder" }` regardless of input | `probe_business::tests::question_body_contains_domain_term` | Test checks that output varies by input and contains a domain term; constant return fails both the domain-term assertion and the variance check across two different inputs |
| `probe` context path in `src/probe.rs` | Return questions with no `evidence` field populated | `probe_business::tests::effect_field_absent_in_output` + provenance unit test | Context-derived questions must be source-tagged; absent evidence breaks the traceability invariant |
| `BusinessReader::read` in `src/port.rs` | Ignore byte cap; return all bytes regardless of `max_bytes` | `probe_business::tests::oversized_evidence_is_refused` | Test sends evidence exceeding the byte cap and asserts `InvalidEvidence`; ignoring the cap passes unbounded data into the context, breaking the bounded-memory assertion |
| `probe` in `src/probe.rs` | Accept empty request/unknown fields without typed refusal | `probe_business::tests::empty_context_still_returns_question` | Test sends a minimal empty-context request and asserts `len() == 1`; a no-input-success stub short-circuits and returns empty, failing the assertion |

Anti-stub: constant `Vec::<Question>::new()` fails material and nonempty fixtures; fake child cannot be the only real-binary proof.

Safety mutation floor: `caught/total >= 75%`.

## 12. Verification recipe and denominators
```bash
cargo test -p probe-business --no-fail-fast
cargo clippy -p probe-business --all-targets -- -D warnings
find crates/probe-business -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
# Mutation floor: caught/total >= 75%
```
Expected: unit `10/10`, property `128/128`, 0 skipped, no file >80 lines. Provider/version/auth are unverified blockers.

## 13. Definition of done
All of the following must be true — each is checkable by command output, no subjective criteria:

- `cargo test -p probe-business --no-fail-fast` exits 0 with `test result: ok` in output.
- `probe_business::tests::ambiguity_request_produces_question` appears in test output and passes.
- `cargo clippy -p probe-business --all-targets -- -D warnings` exits 0.
- `find crates/probe-business -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +` exits 0.
- `cargo test -p probe-business property --no-fail-fast` output contains `checked=128,total=128`.
- Mutation floor: `caught/total >= 75%` for the named mutation targets in §11; mutating `probe` to return `vec![]` causes `probe_business::tests::ambiguity_request_produces_question` to fail.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a business document instructed a write → retrieved prose was treated as authority → strip effect fields and route only questions/context.
1. Can evidence authorize a grant? 2. What is the byte bound? 3. Is provider support documented or actually smoke-tested?
