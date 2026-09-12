# Package Adoption Decisions — Fleet 32-Node SDLC Pipeline

Research snapshot: 2026-09-11/12. All decisions are point-in-time; evidence labels follow the
convention in `effects-operations.md` (D = documented, L = locally verified, U = unverified here).

## What we are building and why

Fleet is a **local CLI SDLC harness** — a 32-node directed-acyclic pipeline that runs the entire
software development lifecycle on a developer's machine without cloud dependencies or a long-running
server. Every node is a separate Rust crate wired together through injected port traits, so model
calls, IO, and storage are swappable without changing business logic. The pipeline mirrors the
LLD (`docs/LLD/LLD.md`) exactly in code structure so a junior engineer can trace any behavior from
the diagram to the running crate.

The four authority nodes (store, control, route, fleet-scan) enforce every durable invariant.
Everything else is stateless or delegates persistence through a port. This is the architectural
invariant the mutation floors (≥80% authority, ≥75% non-authority) protect.

## Per-node package decision table

| Node(s) | Package chosen | Version in Cargo.lock | Why (not custom) | Rejected alternatives |
|---|---|---|---|---|
| `store` | `rusqlite` | 0.32.1 | Mature SQLite WAL binding; bundled feature avoids system lib; MIT license. Adopted version from Cargo.lock, not the research candidate 0.40.2 (see below). | `redb` kept for KV-only in fleet-store; `sled` unmaintained |
| `store` | `sha2` | 0.10.x | SHA-256 for blob content-addressing; part of RustCrypto; no C dependency | Custom rolling hash has no audit trail |
| `store` | `tempfile` (dev) | 3.x | Reproducible temp-dir isolation in tests; no test writes to production paths | Manual `/tmp/fleet-test-*` leaves artifacts on panic |
| `fleet-scan`, all probes | `serde` + `serde_json` | 1.x | Standard de/serialization for port messages and ScanDecision; zero runtime cost for in-process calls | `bincode` — non-human-readable, harder to debug |
| `fleet-scan`, `control` | `thiserror` | 2.x | Typed error derivation without macro noise; cleaner than `anyhow` for library errors | `anyhow` — for binary surfaces, not library APIs |
| `dag` | `petgraph` attempt | — | Blueprint specified 0.8.3; **not in crates.io** — version does not exist. Replaced with adjacency-list HashMap; Kahn's algorithm is 50 lines, stable, and has no external dep. | `petgraph` 0.6.x (exists, but adds 200 kB for a graph we own completely) |
| `planner`, `intent`, `context` | `tokio` | 1.53.1 | Async runtime for concurrent probe fan-out; bounded `mpsc` for backpressure; MIT. Tokio is already in the workspace lock. | `async-std` — smaller community; `smol` — no ecosystem tooling |
| `knowledge` | Custom `InMemoryStore` | — | No external dep needed; knowledge items fit in-process for a local harness; eviction policy is simple LRU-equivalent | `redis` — requires external server; defeats local-only premise |
| `connectors` | Port trait only (no SDK) | — | `ProviderClient` + `CredentialPort` are injected; no Anthropic/OpenAI SDK in the crate. Tests use mock impls. Credential values never appear in `NativeEvent`. | Direct SDK — couples every test to network; violates security boundary |
| `probe-research` | Port trait only | — | `ResearchPort` is injected; `rmcp` marked optional and unused in CI. Fallback question emitted on timeout. | `rmcp` direct dep — not in Cargo.lock; would require network in tests |
| `review` | `semgrep` (subprocess) | system | Semgrep is invoked as a subprocess with a JSON output format; findings are parsed, not trusted. | Rust SAST library — none mature enough |
| `builder` | `fd-3` frame protocol | — | Fleet's own frame protocol over file descriptor 3; subprocess spawning + RLIMIT via `nix` 0.29.0 | `bubblewrap`/Landlock — Linux-only; defeats macOS dev loop |
| `post` | `gitleaks` (subprocess, optional) | system | Secret scanning as a best-effort subprocess; never a required gate. Evidence label: U (not installed on all hosts) | `cargo-audit` — dependency audit, not secret scan; different threat |
| `user-cli` | `clap` | 4.x | Standard CLI parser; derive macros; MIT. Already in workspace. | `argh`, `pico-args` — smaller but no derive support |
| All crates | `proptest` (dev, selected) | 1.11.0 | Property tests for DAG, state machines, revision arithmetic. Already declared by `fleet-context`. | `quickcheck` — less ergonomic shrinking |
| `verify` | `cargo-mutants` (dev tool, not dep) | 27.1.0 | Mutation score measurement. Run as: `cargo mutants -p <crate> --timeout 60 --jobs 4` | Manual mutation — not repeatable |

## Rusqlite version note (criterion 12 — teach models to use stable libraries)

The research file `effects-operations.md:30` documents rusqlite `0.40.2` as a 2026-08-08 release.
**Do not adopt 0.40.2 in this migration.** The Cargo.lock resolves `0.32.1` with `bundled` and
`load_extension`. Mixing a version upgrade into a blueprint migration creates two changes (API
surface + business logic) that cannot be independently reverted. The blueprint agent brief says:
"adopt the existing lock version; upgrade separately in a dedicated PR."

## What each research file covers

| File | Scope |
|---|---|
| `effects-operations.md` | integrate, approval, notify, broker, rollback; Git CLI vs git2; SQLite vs redb; Tokio channels; sandbox options |
| `foundation-control.md` | store, control, route, scan, ingest; SQLite WAL; reducer patterns; materiality gate |
| `model-context-planning.md` | planner, plan_review, context, dag, knowledge; in-process stores; proptest; tantivy |
| `quality-evidence.md` | review, verify, candidate, offline, post; semgrep, cargo-mutants, cargo-fuzz, cargo-llvm-cov |

## Adoption rule for future blueprint authors (criterion 15 — low-parameter agents)

Before writing a custom data structure or IO handler, answer four questions:
1. Is this functionality in `std`? (iterators, HashMap, BTreeMap, Path, File) → use std.
2. Is there a crate already in `Cargo.lock`? → use it; pin the exact version already resolved.
3. Is there a crate with ≥1M downloads, MIT/Apache-2 license, and a `L`-verified smoke? → adopt after one end-to-end smoke run.
4. Otherwise → build new, but cap at 80 lines and test it at ≥75% mutation coverage.

A crate without a locally-run smoke is a **claim**, not an adoption. Record the exact command and
its output.
