# Model/context/planning migration research

Research Group B, checked 2026-09-11. Scope is the LLD node family: `intent`,
`scan`, four context probes, `questions`, `dag`, `knowledge`, `planner`,
`context`, `plan_review`, `ready`, `next_plan`, and `builder`.

## Recommendation

Keep Fleet's deterministic Rust controller as the authority. Adopt protocol and
library primitives behind small ports; do not replace the local DAG, receipts,
leases, readiness predicate, or acceptance gates with an agent framework.

| LLD node | Migration boundary | Evidence-backed decision |
|---|---|---|
| `intent` | Schema-constrained proposal plus deterministic second route candidate | Adopt `serde`/`serde_json` + `schemars`; model confidence is advisory. Reject unknown effect/workflow kinds. |
| `scan` + four probes | `business`, `technical`, `memory`, `research` run concurrently and fault-isolated | Keep the existing four-port shape. Do not use a security scanner or an LLM swarm as authority. Probe timeout is missing evidence. |
| `questions` | Merge, deduplicate, rank, cap at three per interaction | Keep pure deterministic merge and answer-to-plan provenance. MCP elicitation may deliver a question, but cannot decide whether it is blocking. |
| `dag` | Versioned adjacency lists, indegrees, ready heap, cycle check, leases | Keep in SQLite/controller. Detect cycles in O(V+E); speculative work cannot satisfy a dependency. |
| `knowledge` + `context` | Repo map, lexical retrieval, conventions, pinned standards, bounded compaction | Adopt tree-sitter + Tantivy + exact token counting. Keep embeddings optional behind a port. |
| `planner` + `plan_review` | Immutable module contract, independent reviewer, digest-bound verdict | Keep native Fleet contract; ACP/MCP are transport/capability adapters only. |
| `ready` | Acceptance, questions, reviewer, pinned dependencies, grants, write-set, budget | Keep deterministic gate; model output never proves readiness. |
| `next_plan` | N+1 plan overlaps N build through bounded queue | Existing implementation is the right small primitive; retain backpressure and crash-resume log. |
| `builder` | Private worktree, fd-3 result channel, explicit tools/MCP, verified tree | ACP is the first integration experiment; strict edit mediation and OS sandbox remain separate Fleet controls. |

## Primary-source package/repository ledger

Versions are the latest observations available on 2026-09-11, not floating
requirements. Commit/date are included so a future migration can reproduce the
research snapshot. “Documented” means upstream metadata/docs. “Local” means
observed in this checkout; no unlisted package was installed into Fleet.

| Capability | Primary source; exact version/commit/date | License | Adoption | Limitations | Smoke command | Evidence status |
|---|---|---|---|---|---|---|
| ACP Rust | [agentclientprotocol/rust-sdk](https://github.com/agentclientprotocol/rust-sdk), `2.1.0`, `572d6636c04349b18da2d0389b5fb114819390ca`, 2026-09-04 | Apache-2.0 | Adopt first for agent subprocess/session adapter | Protocol conformance does not prove provider login, model identity, sandboxing, or acceptance quality; negotiate optional capabilities | `cargo test --manifest-path <pinned ACP checkout>/Cargo.toml` | Documented upstream metadata; not locally installed/verified |
| ACP TypeScript | [agentclientprotocol/typescript-sdk](https://github.com/agentclientprotocol/typescript-sdk), `1.4.0`, `e6463f444093ed7c5f1cc937c3f32afb5853e906`, 2026-08-20 | Apache-2.0 | Reference only; Fleet core is Rust | Adds a second runtime and does not improve controller authority | `npm test` in pinned checkout | Documented only |
| MCP Rust SDK | [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk), `rmcp 3.3.0`, `a18525858f37a16c8b9df49dcc421dbb64b18585`, 2026-09-10 | Licensing transition: new code Apache-2.0, some prior code MIT; docs CC-BY-4.0 | Adopt behind the trusted MCP broker, not directly in workers | MCP schemas/transport do not provide idempotency, authorization intent, lease accounting, or sandboxing; `rust-version 1.88` | `cargo add rmcp@3.3.0 --features transport-child-process,client` in a disposable probe | Documented upstream; not in current Cargo graph |
| MCP TypeScript/Python SDKs | [MCP SDK matrix](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/docs/2026-07-28/sdk.mdx); Python current stable v1.x, v2 alpha; TS current repo main | MCP transition above; Python package MIT | Do not add to Rust controller; use only for external server compatibility fixtures | Version/protocol churn; Python v2 migration is breaking; SDK tier is not a security proof | `python -m pip install 'mcp<2'` then official quickstart | Documented only |
| Structured output | [schemars](https://crates.io/crates/schemars) `1.2.2` (latest observed), [serde_json](https://crates.io/crates/serde_json) `1.0.151`; MCP Rust uses JSON Schema 2020-12 | MIT / MIT | Adopt for Rust wire types and schema generation | Schema validity is not semantic correctness; preserve unknown/absent distinction and integer counts | `cargo test -p fleet-types -p fleet-plan`; add negative schema fixtures before wiring model adapter | `cargo search` documented registry metadata; local Fleet already uses serde/serde_json, no schemars yet |
| JSON Schema validation | [jsonschema-rs/jsonschema](https://github.com/Stranger6667/jsonschema), crate `0.56.0`, commit `ea17251f5c04382d31fc92846fc24609a519a4e7`, 2026-09-10 | MIT | Adopt only if schema validation is needed outside serde; otherwise keep dependency surface smaller | Default features include HTTP/file reference resolution; disable remote resolution for untrusted schemas; still cannot validate authorization semantics | `cargo test -p <probe-crate>` with valid, missing-field, extra-field, wrong-type, and `$ref`-blocked fixtures | Upstream/package documented; not locally added |
| Tree-sitter | [tree-sitter/tree-sitter](https://github.com/tree-sitter/tree-sitter), `0.27.0`, `3e719425fc48f5b4cdb25c580e44023882f5e2a7`, 2026-08-30 | MIT | Adopt; existing Fleet context already uses Rust/Python/Bash grammars | Grammar versions are independent; parse recovery is not semantic/type resolution; SCIP remains absent | `cargo test -p fleet-context --test repomap_fixtures --test repomap_macro_calls` | Local verified: existing fixture tests passed; current manifest pins Rust binding `0.24.7`, Rust grammar `0.23.3`, Python `0.23.6`, Bash `0.23.3` |
| Tantivy | [quickwit-oss/tantivy](https://github.com/quickwit-oss/tantivy), `0.26.1`, `0093923d94157d9f1f63a292bb504bb8db401f2a`, 2026-05-10 (latest registry `0.26.2`) | MIT | Adopt for local BM25 retrieval; keep index coverage/freshness in evidence | Native index files, schema evolution and mmap/disk pressure need operational controls; lexical retrieval misses semantic synonyms | `cargo test -p fleet-context --test retrieve_pipeline --test fuse_rrf` | Local verified: retrieval/RRF tests passed; current manifest pins `0.26.1` |
| SQLite FTS5 | [SQLite FTS5 documentation](https://www.sqlite.org/fts5.html), SQLite source tag `version-3.45.1`, commit `189e44dfecdc7868bb860dfb5d98eab371318c37` | SQLite public-domain dedication (source also contains separately licensed components) | Keep SQLite as durable controller/store truth; use FTS5 where transactional co-location beats Tantivy | Tokenizer behavior, ranking and external-content consistency must be specified; FTS5 is not a vector index | `sqlite3 :memory: 'create virtual table x using fts5(t); insert into x values("hello fleet"); select rowid from x where x match "fleet";'` | SQLite docs/source documented; Fleet locally uses `rusqlite` graph storage, no FTS5 smoke run recorded |
| Token counting | [tiktoken-rs](https://crates.io/crates/tiktoken-rs) `0.12.0`, upstream commit `a79050e5a465592dc7e80f23ed69992bb1ac7f50`, 2026-07-01; OpenAI reference [tiktoken](https://github.com/openai/tiktoken) `0.14.0`, `4e71bbe0c078468e00fefbf94b39849389f346e5`, 2026-08-17 | MIT | Adopt `tiktoken-rs` for local admission/compaction counts, with model-encoding provenance | Tokenizer may not match every provider; counts are estimates for providers that do not expose usage; never convert unknown usage to zero | `cargo test -p fleet-context --lib tokens::tests` | Local verified: known cl100k example and empty-input tests passed; current manifest pins `0.12.0` |
| Async local process | [tokio](https://github.com/tokio-rs/tokio), `1.53.1`, `75fef53d0a8590c2d1dbb63672aa7b7d1ef51155`, 2026-07-20 | MIT | Adopt existing `tokio::process`/pipes for supervised adapters | Process API is not a sandbox; descendant cleanup, environment allowlists, fd-3 framing, deadlines and unknown side effects remain Fleet responsibilities | `cargo test -p fleet-stream`; a dedicated child-process kill/timeout test is required before production | Local dependency/use documented in current workspace; process security not fully verified |
| Linux sandbox | [containers/bubblewrap](https://github.com/containers/bubblewrap), `v0.12.0`, `014a04330642e5c870418beb621532cb896e0002`, 2026-08-26 | LGPL-2.0-or-later (upstream project) | Optional Linux strict/opaque sandbox adapter; detect capability and fail closed to reduced tier | Linux-only; namespaces/pivot_root/uid mappings need host privilege tests; no macOS equivalent; sandbox does not define Fleet policy | `bwrap --ro-bind / / --dev /dev --proc /proc --tmpfs /tmp --die-with-parent -- /bin/true` | Upstream documented; not locally run/verified |
| OS-specific sandbox API | [landlock](https://crates.io/crates/landlock) / Linux Landlock | MIT/Apache-2.0 dual license (verify exact selected release before adoption) | Evaluate as a Linux filesystem/network restriction port, never as universal sandbox | Kernel/version dependent; cannot cover all process/network/credential channels; macOS/Windows need separate adapters | `cargo test` plus a real child read/write denial test on a Landlock-capable kernel | Candidate only; no exact release or local proof captured at checkpoint |

## Local Fleet evidence and gaps

Documented in the LLD (repository snapshot `82454636847ddd2bc8b2e8aa6dc463a822f14bf6`,
2026-09-11):

- `intent` emits schema-constrained `IntentSpec`, but the controller computes a
  deterministic second workflow candidate and downgrades disagreement.
- Material ambiguity is defined by effect, acceptance refs, missing grants, or
  ready/scheduled dependency impact. Four probes have a local deadline and run
  concurrently; timeout is missing evidence. Questions are capped at three.
- `ready(module)` requires nonempty acceptance, resolved blocking questions,
  reviewer acceptance of the digest, pinned dependencies, grants, exclusive
  write scope, and available budget/resources.
- DAG cycle detection is O(V+E); scheduling is deterministic with critical-path,
  age, blocked successors, priority, cost, and stable task-ID tie-break.
- N+1 planning is allowed while immutable N builds; a shared-contract change
  invalidates affected descendants. The builder has private worktree/fd-3 and
  the worker receives no ledger/state/socket paths.

Locally verified in this checkout:

- `crates/fleet-scan/src/assess.rs::assess()` constructs exactly four probes
  (`business`, `technical`, `memory`, `research`), calls `run4`, isolates faults,
  and merges questions.
- `src/pipeline/planahead/orchestrator.rs::run_plan_ahead()` starts planner and
  builder tasks concurrently with a bounded channel and a durable `UnitLog`.
- `fleet-context` fixture, retrieval, RRF, PageRank, compaction, convention,
  and token tests observed passing during `cargo test -p fleet-context -p
  fleet-scan -p fleet-plan --no-fail-fast` before this checkpoint stopped the
  still-running plan suite. This is partial local evidence, not a full workspace
  verification claim.
- Current manifests locally pin `tree-sitter 0.24.7`, `tantivy 0.26.1`, and
  `tiktoken-rs 0.12.0`; `fastembed`/`ort` and real vector search are absent.
  Current requirements explicitly mark MCP mounting as `NotYetImplemented` and
  embeddings as absent/BM25-only.

## Migration proof gates

1. Pin each adopted dependency by version, source digest, license, MSRV, and
   feature set; reject unreviewed remote schema/reference resolution.
2. Add real-binary adapter smokes: ACP initialize/capability negotiation,
   MCP stdio child, structured-output invalid cases, cancellation, timeout,
   descendant cleanup, and fd-3 framing. A mock child is not proof.
3. Publish denominators: probe count must be four, question count must be 0–3
   with a reason, DAG checked/total must be nonzero, context tokens must carry
   encoding and source, and retrieval must publish indexed/queried coverage.
4. Keep provider, local-estimate, and unallocated account usage separate.
   Keep sandbox capability tier separate from protocol success.
5. Do not claim migration complete until the real Fleet binary reaches intent →
   scan → questions/plan → ready → builder, and independent verification proves
   the resulting tree and receipt chain.

