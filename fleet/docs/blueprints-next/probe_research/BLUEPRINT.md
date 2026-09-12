# BLUEPRINT — `probe_research`

## 1. Identity and LLD path
- Node id / label / tag: `probe_research` / Research Probe / `model,ungated`
- LLD authority: `docs/LLD/LLD.md §§6,22` and `docs/LLD/lld-full-detail.architecture.json:components[id=probe_research]`
- Why: retrieve bounded external documentation only when a material unknown needs it.
- Incoming edges: `scan -> probe_research`: canonical `RequirementInput`; the boundary adapter maps
  it to `ResearchInput` with scope, deadline, and selected unknowns.
- Outgoing edges: `probe_research -> questions`: `Vec<Question>` with cited evidence.
- Build status: `greenfield` — only an external research port seam exists.

## 2. Responsibility and non-goals
**Owns:** query shaping, source attribution, bounded research observations.
**Does not own:** web authorization, factual acceptance, provider capability claims, question merge, or effects.

## 3. Boundary and authority
Network/research adapter is untrusted and deadline-bound. The probe may return context/questions with URL/source/date and an uncertainty label. It may not install packages, authenticate, publish, modify memory, or declare a library/provider adopted. Parent owns network policy and redaction.

## 4. Crate/package layout
`Cargo.toml` declares the node (≤25 lines); `src/lib.rs` exports it (≤30); `src/probe.rs` validates sources (≤70); `src/port.rs` owns research transport (≤60); `tests/probe.rs` owns fixtures (≤80). All live under `crates/probe-research/`.

## 5. Public API contract
```rust
pub struct ResearchInput { pub query: String, pub unknown: String, pub deadline_ms: u64, pub revision: u64 }
pub struct Source { pub url: String, pub title: String, pub retrieved_at: String, pub excerpt: String }
pub struct Question { pub text: String, pub evidence: Vec<Source> }
pub trait ResearchPort: Send + Sync { fn search(&self, query: &str, deadline_ms: u64) -> Result<Vec<Source>, ResearchError>; }
pub fn probe(input: &ResearchInput, port: &dyn ResearchPort) -> Result<Vec<Question>, ResearchError>;
pub enum ResearchError { InvalidInput, InvalidSource, Timeout, Unavailable(String) }
```
No source is authoritative until a downstream deterministic/documentation gate validates it. The
boundary adapter maps the canonical `RequirementInput`/`ProbeOutcome` edge; this crate owns only
the research port and its cited question output.

## 6. Data model, invariants, and failure taxonomy
| Type / record | Invariant | Illegal state prevented | Failure / exit code |
|---|---|---|---|
| `Source` | URL/title/excerpt nonempty, total bytes bounded | uncited claim | `InvalidSource` / 6 |
| `ResearchInput` | deadline >0 and bounded; revision nonzero | unbounded network | refusal 7 |
| result | retrieved source count and query coverage published | zero-input green | `checked=0` -> 8 |

URLs are strings, timestamps are fixed RFC3339 strings, counts/bytes integers. Network, clock, and auth are injected.

## 7. Reuse map and adoption decisions
| Existing source or stable library | Evidence/version/commit/license | Use | Why not custom code | Proof still required |
|---|---|---|---|---|
| `crates/fleet-scan/src/ports.rs:23-28` `ResearchPort` | local source | retain narrow search seam | current fault boundary | source/timeout contract |
| `rmcp = "0.3"` — Rust MCP client, [upstream](https://github.com/modelcontextprotocol/rust-sdk) | Apache-2.0/MIT; crates.io 0.3.x; pinned exact | MCP tool invocation (search adapter) | adopt protocol, no custom MCP wire format | `cargo test` in pinned disposable checkout; smoke: call a no-auth local tool server |
| `reqwest = { version = "0.12", features = ["json", "rustls-tls"] }` | MIT OR Apache-2.0; crates.io 0.12.x | HTTP fallback when MCP unavailable | avoids raw hyper wiring; rustls keeps OpenSSL out | build with `--no-default-features`; smoke: GET returning 200 |
| `serde = { version = "1", features = ["derive"] }` | MIT OR Apache-2.0; crates.io 1.x | JSON (de)serialization of Source/Question | no hand-rolled parser; derive covers all types | `cargo check`; zero unsafe |
| `thiserror = "2"` | MIT OR Apache-2.0; crates.io 2.x | `ResearchError` enum | `thiserror` emit is stable and avoids boilerplate | `cargo check`; inspect generated `source()` impl |
| `tokio = { version = "1", features = ["rt-multi-thread", "macros"] }` | MIT; crates.io 1.x | async runtime for port and timeout | standard async executor; `timeout()` covers deadline_ms | `cargo test` with `#[tokio::test]`; confirm no `std::thread::sleep` in hot path |

Current provider versions/capabilities are explicitly unverified for Fleet.

### Exact `Cargo.toml` snippet — `crates/probe-research/Cargo.toml`

```toml
[package]
name    = "probe-research"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-scan  = { path = "../../crates/fleet-scan" }    # optional boundary adapter only
rmcp      = "0.3"
reqwest   = { version = "0.12", features = ["json", "rustls-tls"] }
serde     = { version = "1",    features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio     = { version = "1",    features = ["rt-multi-thread", "macros"] }
```

### MCP tool call pattern (rmcp, ≤15 lines)

```rust
use rmcp::{Client, tool::ToolInput};

async fn call_research_tool(
    client: &Client,
    query: &str,
    deadline_ms: u64,
) -> Result<String, ResearchError> {
    let input = ToolInput::new("search", serde_json::json!({ "query": query }));
    tokio::time::timeout(
        std::time::Duration::from_millis(deadline_ms),
        client.call_tool(input),
    )
    .await
    .map_err(|_| ResearchError::Timeout)?
    .map(|r| r.content)
    .map_err(ResearchError::Mcp)
}
```

This is the only entry point that touches the MCP wire; all other logic is pure over `&str` / `Vec<Source>`.

## 8. Behavior matrix
### `probe`
| Input/failure dimension | Observable behavior |
|---|---|
| empty / missing | empty query/unknown -> refusal 7 |
| huge / negative | truncate by validated byte/query caps; reject invalid deadline |
| duplicate / concurrent | dedupe URL+digest; parent may run probes concurrently |
| partial failure / timeout | return partial cited evidence plus missing-evidence question; never clear |
| stale / unavailable | mark unavailable with error/source gap; no fallback fabricated source |

## 9. Tiny implementation steps
1. In `src/lib.rs`: Define `ResearchInput`, `Source`, `Question`, `ResearchError`, and `ResearchPort` trait; re-export all public items; `cargo check -p probe-research` exits 0.
2. In `src/probe.rs`: Add URL/excerpt/deadline validation and bounded byte caps; `cargo test -p probe-research ambiguity_request_produces_research_question` exits 0.
3. In `src/port.rs`: Wrap `ResearchPort` and normalize partial results into cited `Source` or fallback `Question`; `cargo test -p probe-research question_references_external_source` exits 0.
4. In `src/port.rs`: Add `handle_timeout` fallback producing a non-empty `Vec<Question>` on deadline miss; `cargo test -p probe-research network_timeout_returns_fallback_question` exits 0.
5. In `tests/probe.rs`: Wire all three named integration tests using a mock MCP tool that resolves in <100 ms; `cargo test -p probe-research --no-fail-fast` exits 0 with `checked=8,total=8`.

## 10. Test matrix
**Unit:** source validation, byte caps, URL dedup, timeout mapping.
**Integration/contract:** `scan -> probe_research -> questions` with cited source fixtures; `8/8`.
**Hidden:** prompt injection in page, redirect/credential leak, stale source, zero-result provider.
**Property:** 128 generated sources remain bounded and cited; `128/128`.
**Differential:** compare normalization to a fixed primary-source fixture set; provider variation is reported.
**Real-binary/effect:** real `fleet` research task only after adapter smoke proves auth/scope; currently blocked.

### Named integration tests (3 required; names are exact)

#### `probe_research::tests::ambiguity_request_produces_research_question`
- **Inputs:** `RequirementInput { text: "implement auth flow; unknown: OAuth scope", task_id: None }`, mock MCP tool returning a one-sentence search result (no network).
- **Expected output:** `Vec<Question>` with exactly 1 element; `question.text` is non-empty.
- **Mutation caught:** deleting the `format_research_prompt` → `call_research_tool` → `Question` pipeline returns `Vec::new()`, which fails the `assert_eq!(questions.len(), 1)` guard.
- **Performance bound:** mock tool must resolve in <100 ms (enforced by `tokio::time::timeout`; if it fires, the test fails, not panics).

#### `probe_research::tests::network_timeout_returns_fallback_question`
- **Inputs:** same `RequirementInput`; mock MCP tool configured to hang for 1 s; deadline set to 50 ms.
- **Expected output:** `Vec<Question>` with exactly 1 fallback question (body contains "unable to retrieve" or similar); no panic; no empty `Vec`.
- **Mutation caught:** deleting `handle_timeout` or its fallback arm causes the function to return `Err(...)` or `Vec::new()`, failing `assert_eq!(questions.len(), 1)`.

#### `probe_research::tests::question_references_external_source`
- **Inputs:** `RequirementInput { text: "configure TLS; unknown: cert chain format", task_id: None }`, mock MCP tool returning `"See the rustls documentation at https://docs.rs/rustls"`.
- **Expected output:** `question.text` contains at least one of: `"research"`, `"documentation"`, `"reference"`, `"source"`.
- **Mutation caught:** a stub that returns a hardcoded question body without incorporating the tool result will fail the substring check; `call_research_tool` mutated to return `""` causes the body to lose all source-referencing text.

## 11. Mutation targets and anti-stub proof

| Function mutated | Mutation applied | Test that catches it | Why it's the right catcher |
|---|---|---|---|
| `call_research_tool` in `src/port.rs` | Change return value to `Ok(String::new())` (empty string) | `probe_research::tests::question_references_external_source` | The empty string cannot produce a body containing "documentation", "source", "reference", or "research"; proves the question body is derived from tool output, not a hardcoded template |
| `format_research_prompt` in `src/probe.rs` | Omit `task_description` from the prompt string; produce only the unknown fragment | `probe_research::tests::ambiguity_request_produces_research_question` | The resulting question body does not relate to the task; proves the full request context reaches the tool call, not just a stripped unknown label |
| `handle_timeout` in `src/port.rs` | Delete the fallback branch so a timeout propagates as `Err(ResearchError::Timeout)` with no question produced | `probe_research::tests::network_timeout_returns_fallback_question` | The function returns an error or empty `Vec` instead of the 1-element fallback; `assert_eq!(questions.len(), 1)` fails; proves the node never returns empty on a deadline miss |
| URL/date attribution in `src/probe.rs` | Drop URL and `retrieved_at` from `Source` | citation hidden test | Evidence must be attributable; a source without a URL is uncited and invalid |
| checked/total field in `src/probe.rs` | Return `checked=0, total=0` vacuously on any result | denominator test | Zero-denominator proof is never a pass; the gate must count real sources |

Safety mutation floor: `caught/total >= 75%`; reviewer manually enables one bypass mutation and confirms kill.

## 12. Verification recipe and denominators
```bash
cargo test -p probe-research --no-fail-fast
cargo clippy -p probe-research --all-targets -- -D warnings
find crates/probe-research -name '*.rs' -exec awk '{c[FILENAME]++} END{for(f in c) if(c[f]>80){print f": "c[f]" lines"; bad=1} exit bad}' {} +
# Mutation floor: caught/total >= 75%
```
Expected `8/8` contract and `128/128` property; real provider only after current primary docs plus observed smoke output. No external research capability is claimed from this blueprint.

## 13. Definition of done

Each line below is a checkable assertion. A reviewer ticks each box independently; no box is assumed from another.

- [ ] `cargo test -p probe-research --no-fail-fast` exits 0 with `checked=8,total=8` in output.
- [ ] `cargo clippy -p probe-research --all-targets -- -D warnings` exits 0.
- [ ] `probe_research::tests::ambiguity_request_produces_research_question` is present by exact name and passes.
- [ ] `probe_research::tests::network_timeout_returns_fallback_question` is present by exact name and passes.
- [ ] `probe_research::tests::question_references_external_source` is present by exact name and passes.
- [ ] mock MCP tool resolves in <100 ms (deadline enforced via `tokio::time::timeout`; test fails, not panics, on breach).
- [ ] `call_research_tool` returns empty string → `question_references_external_source` fails (mutation verified manually or via `cargo-mutants`).
- [ ] `handle_timeout` fallback deleted → `network_timeout_returns_fallback_question` fails.
- [ ] `format_research_prompt` omits `task_description` → `ambiguity_request_produces_research_question` fails.
- [ ] `Cargo.toml` pins `rmcp = "0.3"`, `reqwest = "0.12"`, `serde = "1"`, `thiserror = "2"`, `tokio = "1"`; no wildcard or path-only dep for these.
- [ ] No `unwrap()` or `expect()` in `src/probe.rs` or `src/port.rs` outside test modules.
- [ ] Edge contract `RequirementInput → Vec<Question>` has ≥1 question even on timeout; the boundary adapter may wrap it as `ProbeOutcome::Questions` for `fleet-scan`, and the reviewer manually sends a 1-second hanging mock and confirms.
- [ ] Adopted SDK (`rmcp 0.3`) has local smoke evidence in `tests/` (a passing test that constructs `rmcp::Client`); not just a Cargo entry.

## 14. Failure stories and review questions
**Failure → Cause → Fix:** a research page was treated as an approved dependency → probe confused evidence with adoption → require primary source, exact version, local smoke, and downstream review.
1. What prevents credentials leaving through a URL? 2. What does timeout mean? 3. Where is provider capability actually observed?
