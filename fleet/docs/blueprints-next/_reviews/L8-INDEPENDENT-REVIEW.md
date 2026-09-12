# L8 Independent Review

Date: 2026-09-11
Reviewer: lead-architect (Opus) — independent; did not author any blueprint
Scope: all 32 `docs/blueprints-next/<node>/BLUEPRINT.md`, read in full
Calibration: `route/BLUEPRINT.md`, `REVIEW-GATE.md`, `AGENTS.md`, `_TEMPLATE.md`, `docs/LLD/LLD.md §24`,
`docs/LLD/lld-full-detail.architecture.json`

**Revision note (material).** Four blueprints were rewritten *during* this review:
`probe_learn` (22:30:26), `probe_business` (22:30:39), `probe_research` (22:30:55),
`probe_tech` (22:31:00) — after my first read at ~22:28. I re-read all four at their current
revision and scored the current text. The other 28 are unchanged since 21:43:56. Any later
edit invalidates the corresponding rows.

## VERDICT: FAIL

Not because the set is weak — the reasoning about authority, trust zones, refusal semantics and
zero-input gates is genuinely L8, and **32/32 reuse-map source citations point at files that
actually exist** (verified individually). It fails on four P0 classes that a junior engineer
cannot route around:

1. Types that do not exist are prescribed as if they did (`fleet-types::acp`, `AmbiguityRequest`).
2. 26 of 33 LLD-declared edge payload types are never named by the blueprint that owns them.
3. The gate's own FR1–FR46 coverage requirement has zero coverage.
4. The four newest blueprints contradict their own §5 API in their own §10 tests.

`REVIEW-GATE.md:48` — "unresolved P0/P1 findings block deletion of legacy blueprints." Legacy
`docs/blueprints/` must be retained until the P0 list below is closed.

---

## P0 Blockers (blocks legacy replacement)

**P0-1 — `probe_business`: prescribed dependency and four types do not exist.**
`probe_business/BLUEPRINT.md:48` and its verbatim Cargo.toml block at `:58`:
```toml
fleet-types = { path = "../../crates/fleet-types" }   # AmbiguityRequest, Question, AcpRequest/AcpResponse
```
Verified against the tree: `crates/fleet-types/src/` contains 15 modules — `attest_predicate,
attest_stmt, attest_subject, exit_code, gate_refusal, ident, lib, lifecycle, module, node_id,
prev_hash, receipt, receipt_event, role, tokens`. There is **no `acp` module**.
`grep -rn --include='*.rs' AcpRequest|AcpPort|AcpResponse|AmbiguityRequest crates src` → **0 hits each**.
The row at `:48` further lists proof as "`cargo test -p fleet-types` passes; AcpRequest round-trip
test" — presenting a non-existent type as locally proven. `:67` then makes it mandatory:
"All model calls must go through `fleet-types::acp::AcpRequest`". A 4B agent copies the block,
runs `cargo check -p probe-business`, and hard-stops on E0432 with no instruction to recover.
*Fix:* either add `crates/fleet-types/src/acp.rs` as a named prerequisite step in
`IMPLEMENTATION-ORDER.md` wave 0, or drop the ACP row and name the real injected port.

**P0-2 — `question.body` does not exist on the real `Question`.**
The authoritative type is `crates/fleet-scan/src/probe.rs:25-37`:
```rust
pub struct Question { pub probe: ProbeKind, pub text: String, pub why: String,
                      pub gap: GapSeverity, pub evidence: Option<String> }
```
The field is **`text`**. Every named integration test in the four upgraded probe blueprints
asserts on `question.body` / `q_a.body`:
- `probe_research:121,127,132,148` (`question.body` × 3, plus the mutation rationale)
- `probe_business:94,96,124` (`question.body`, `Question { body: "placeholder" }`)
- `probe_tech:86,87,105` (`body is non-empty`, `assert_ne!(q_a.body, q_b.body)`)
- `probe_learn:85,123` (`The body field of the returned questions`)
All twelve named tests fail to compile. `questions/BLUEPRINT.md:32` correctly says "text/why
nonempty", so the set contradicts itself on the same struct. This is the single highest-value
defect: the tests that are supposed to be the stub-proof are the tests that cannot build.

**P0-3 — `AmbiguityRequest` is undefined and carries four incompatible field sets.**
It is the LLD's canonical `scan -> probe_*` payload (architecture.json, 4 connections). No
blueprint declares who defines it, and the four probes each assume a different shape:
| Blueprint | Line | Assumed fields |
|---|---|---|
| `probe_research` | 120 | `{ task_description, unknowns }` |
| `probe_business` | 94 | `{ task_description, context }` |
| `probe_tech` | 88 | `{ …, file_context }` |
| `probe_learn` | 83 | `{ topic, scope }` |
One of these four builds; the other three do not. `scan/BLUEPRINT.md:36` emits
`ScanDecision::Probe { kinds, revision }` — none of the four. *Fix:* one blueprint (`scan`, as
producer) must own `AmbiguityRequest` with one field set, and the four probes must consume it.

**P0-4 — the four upgraded probes contradict their own §5 public API in their own §10 tests.**
Each declares one API in §5 and tests a different, undeclared one in §10/§11:
| Node | §5 declares | §10/§11 test and mutate |
|---|---|---|
| `probe_business` | `probe(&BusinessInput, &dyn BusinessReader) -> ProbeOutput` (`:27`) | `generate_questions(&req, &fake_acp)` (`:107`), mutants `generate_question`, `format_prompt`, `map_response_to_questions` (`:122-124`) |
| `probe_tech` | `probe(&TechnicalInput, &dyn CodebaseReader) -> ProbeOutput` (`:26`) | mutants `generate_tech_question`, `extract_code_context`, `map_response` (`:103-105`) |
| `probe_learn` | `probe(&LearnInput, &dyn MemoryReader) -> Vec<Question>` (`:26`) | mutants `retrieve_relevant_lessons`, `format_learning_prompt`, `map_to_questions` (`:100-102`) |
| `probe_research` | `probe(&ResearchInput, &dyn ResearchPort) -> ResearchOutput` (`:26`) | `call_research_tool`, `format_research_prompt`, `handle_timeout` (`:146-159`) |
`probe_business` even disagrees with itself on singular/plural: fixture calls `generate_questions`
(`:107`), mutation target is `generate_question` (`:122`). None of these functions appear in §5,
§4's file map, or §9's steps. AGENTS.md:66 — "The blueprint is incomplete if a 4B agent would need
to ask which file, type, edge, command, or failure behavior is intended." This is that, four times.

**P0-5 — 26 of 33 LLD edge payload types are never named by the owning blueprint.**
Checked every `connections[].payload` in `lld-full-detail.architecture.json` against the producing
node's blueprint text. Present: `NormalizedEvent`, `IntentSpec`, `RouteDecision`, `CatalogSnapshot`,
`QuestionSet`, `ContextManifest`, `IntegrationReceipt` (7). Absent: `InputEnvelope`,
`ConnectorEnvelope`, `DurableEvent`, `StoreCommand`, `DispatchRequest`, `AmbiguityRequest`,
`WorkflowSelection`, `WorkflowRecipe`, `SkipPlanningSignal`, `RepairSignal`, `KnowledgeManifest`,
`PlanProposal`, `ReviewedPlanDigest`, `PlanWalkthrough`, `Lease`, `NextPlanSignal`,
`CandidateObservation`, `ReviewedCandidate`, `GateEvidence`, `LessonCandidate`, `ValidatedLesson`,
`PostVerifyEvidence`, `PostVerifyFailure`, `PublicationGrant`, `PublicationRequest`,
`StateChangeEvent` (26).
The divergence is not cosmetic — it is contradictory:
- `ready -> builder` payload is `Lease`. `ready/BLUEPRINT.md:34` returns `ReadyVerdict` and never
  defines `Lease`; `builder/BLUEPRINT.md:33` defines `pub struct Lease`. **The consumer defines the
  producer's output type.**
- `candidate -> offline` payload is `LessonCandidate`. `candidate:33` defines `CandidateLesson` —
  the words inverted. `offline:8` then consumes `CandidateLesson` and emits `OfflineScore`, but the
  LLD says the `offline -> knowledge` payload is `ValidatedLesson`, and `knowledge:26` returns
  `SourceManifest` where the LLD says `KnowledgeManifest`.
- `scan:1` says its incoming edge is "admitted `IntentSpec`"; the LLD says `RouteDecision`
  (`route:39` correctly produces `RouteDecision`). The two adjacent nodes disagree on the wire.
Related: of the 43 canonical test identifiers declared in `connections[].test` (e.g.
`route::tests::intent_spec_admitted_on_capability_match`), **only 4 appear anywhere in the 32
blueprints** — and all 4 are in the probe files rewritten tonight. REVIEW-GATE.md:11 requires
"Every LLD edge has at least one typed payload contract and one integration test plan"; the plans
exist as prose but nothing binds them to the LLD's own identifiers.

**P0-6 — FR1–FR46 trace does not exist.**
REVIEW-GATE.md:12 requires "Every FR1–FR46 row in LLD §24 maps to one or more node blueprints and a
required observation." `LLD.md:590` has all 46 rows, each with a "Required acceptance observation".
`grep -rn "FR[0-9]" docs/blueprints-next/` returns hits **only inside REVIEW-GATE.md itself** — no
blueprint cites a single FR, and no trace file exists. Spot-checking the FRs against the set finds
genuinely unowned requirements (greps below run across all 32 blueprints):
| FR | Requirement | Owner found |
|---|---|---|
| 11 | Repo `.fleet/` control, strict config validation, value provenance | **none** — `.fleet/` matches 0 blueprints |
| 12 / 29 | Inject skills/MCP/standards; declared capability effects | **none** — "skills" matches 0 blueprints |
| 43 | One logical multi-repo change (LLD §15 saga: PREPARING/VERIFIED/PUBLISHING/PARTIAL/COMPLETE/COMPENSATING) | **none** — `ChangeSet`, `saga`, `PREPARING` match 0 blueprints; `integrate` and `rollback` both cite §15 as authority but neither mentions the saga |
| 20 | Quota failover, reroute on exhaustion | partial — `route`/`model_catalog` mention quota; "failover" matches 0 |
| 28 | Small local footprint; RSS/CPU/disk profile and cleanup denominators published | **none** — "RSS" matches 0 |
| 21 | Tokenomics; failed-attempt costs included, unknown billed data stays unknown | partial — `control` reserves budget; "tokenomics" matches 0; no node publishes failed-attempt cost |
| 42 | Pause/resume (LLD §16, 15 named states) | partial — `control` owns pause/resume but never enumerates the 15 states, while `control:121` publishes `checked=15,total=15` for "transition cases" without saying which 15 |
FR43 is the most serious: an entire six-state LLD subsystem has no node blueprint at all.

---

## P1 Gaps (must fix before build phase)

**P1-1 — `probe_learn` pins `tantivy = "0.22"` and corroborates it with a false claim.**
`probe_learn:45` — "`tantivy = \"0.22\"` … pinned; same version in `fleet-memory/fleet-context`
Cargo.toml", repeated at `:50` and `:55` as "Do not substitute". Verified:
`crates/fleet-context/Cargo.toml:14` declares `tantivy = "0.26.1"`; `fleet-memory` declares tantivy
in no manifest at all; `Cargo.lock` resolves **0.26.2**. This is a four-minor-version regression
presented as workspace corroboration, and the "do not substitute" instruction locks a 4B agent into
it. Note the *earlier* revision of this same file said 0.26.1 and was correct — the rewrite
regressed it. `knowledge:46` and `context:56` still correctly say 0.26.1.

**P1-2 — `probe_tech` mandates a tree-sitter version that conflicts with the workspace.**
`probe_tech:49` — "copy verbatim, do not substitute other versions" — then `:55-56` pins
`tree-sitter = "0.25"`, `tree-sitter-rust = "0.23"`. `crates/fleet-context/Cargo.toml:9-10` declares
`tree-sitter = "0.24.7"`, `tree-sitter-rust = "0.23.3"`; lock resolves 0.24.7. `knowledge:47` and
`context:54` both say 0.24.7. Two tree-sitter majors in one workspace is a known-bad build.

**P1-3 — `store` adopts a rusqlite version the research explicitly says not to adopt.**
`store:66` adopts `rusqlite 0.40.2`. `Cargo.lock` resolves **0.32.1**.
`_research/effects-operations.md:30` says: "Current checkout resolves `rusqlite 0.32.1` … **do not
mix a version upgrade into the blueprint migration**", and `broker:55` correctly records
"local resolves `rusqlite 0.32.1`; newer `0.40.2` is a research candidate, not adopted."
Two blueprints give opposite adoption status for the same crate, and `store` took the side its own
research file forbids. The two research files (`foundation-control.md:26` vs
`effects-operations.md:30`) contradict each other on this crate.

**P1-4 — `probe_research`'s only wire-touching code sample is unverified and under-declared.**
`probe_research:70-88` is the sample the blueprint calls "the only entry point that touches the MCP
wire" (`:90`). Two problems:
- It calls `serde_json::json!` at `:78`, but `serde_json` is **not in the Cargo.toml block at
  `:60-66`** (only `serde`). Guaranteed compile failure for a verbatim copy.
- It assumes `rmcp::Client` and `rmcp::tool::ToolInput::new(...)` + `client.call_tool(input)`.
  I could not verify this API (rmcp is absent from `Cargo.lock`), and it does not match rmcp's
  documented entry points. The blueprint's own proof column says "smoke: call a no-auth local tool
  server" — that smoke must run **before** this snippet is handed to a builder, not after.
  Note the earlier revision said `rmcp 3.3.0`; this one says `0.3`. The version moved without a
  recorded reason.

**P1-5 — `user_cli`'s command denominator is wrong, and it is a denominator.**
`user_cli:89` — "parser tests cover all 29 visible/hidden names"; `:125` — "parser cases
`checked=29,total=29`". Actual `src/cli/root.rs` `Commands` enum: **34 variants** — 29 visible plus
5 hidden (`__pipeline_probe`, `__planahead_probe`, `__agent`, `__spawn_probe`, `__capacity_probe`).
"29 visible/hidden" reads as the total. A builder writes 29 tests, publishes `29/29`, and the gate
is green while `__agent` — the real child path `builder` depends on — is untested. This is exactly
the failure mode the denominator doctrine exists to prevent, inside the doctrine's own example.

**P1-6 — 15 of 32 blueprints state no numeric mutation floor, and they are the authority nodes.**
REVIEW-GATE.md:43-44 requires a ≥75% default and ≥80% for safety/authority predicates, with the raw
denominator. Stated: `builder, context, dag, knowledge, next_plan, plan_review, planner, probe_learn,
questions, ready, review, scan, store, verify` at ≥80%; `probe_business, probe_tech, probe_research`
at ≥75%. **Not stated anywhere:** `approval, broker, candidate, connectors, control, ingest,
integrate, intent, model_catalog, notify, offline, post, rollback, route, user_cli`.
That list contains every node that mints or executes authority: `approval` (mints the capability),
`broker` (the only external-effect path), `rollback` (destructive), `integrate` (merge CAS),
`post` (the approval gate), `control` (sole effect authority), and `route` — **the designated
exemplar**. Each has a good qualitative mutation table; none commits to a floor or denominator.

**P1-7 — `probe_business`'s clippy gate cannot fail.**
`probe_business:149`:
```bash
[ ] cargo clippy -p probe-business --all-targets -- -D warnings 2>&1 | grep -qv "^error"
```
`grep -qv PATTERN` exits 0 if **any** line does not match. Clippy output always contains at least
one non-`error` line (`Checking …`, `Compiling …`), so this assertion passes even when clippy fails
with errors. A check cheaper to fake than to satisfy will be faked (`fleet/PRINCIPLES.md`). Correct
form is to assert the exit status directly.

**P1-8 — `probe_tech`'s test-count gate is arithmetically wrong.**
`probe_tech:118`:
```bash
cargo test -p probe-tech --no-fail-fast 2>&1 | grep "test result" | grep -E "[89] passed"
```
`[89]` matches a single digit 8 or 9 anywhere, so it passes on "18 passed" and "89 passed" and
**fails on "10 passed"** — i.e. it fails once the suite grows past 9 tests, which the blueprint
itself requires (§10 asks for 8 contract + 128 property + 3 named). `:121` has the same class of
problem: "`… | grep -c " test$"` prints ≥8" is a printed number, not an exit-code assertion.

**P1-9 — sibling probes hold contradictory dependency policy.**
`probe_business:67` — "**Do NOT** add `anthropic-sdk` or any provider SDK as a direct dependency …
consistent with C9 (model calls via llm-gateway)", enforced at `:158` by
`! grep -E 'anthropic-sdk|openai|reqwest' Cargo.toml` — which forbids **reqwest**.
`probe_research:62` requires `reqwest = { version = "0.12", … }` as a direct dependency and never
mentions C9. Same node family, same wave, opposite rule. A builder given both cannot satisfy both.

**P1-10 — 14 of 32 blueprints carry an unresolvable LLD JSON pointer.**
Eighteen use the correct form (`…architecture.json:route`, or `components[id=scan]`). Fourteen use a
bare integer: `planner:27, context:28, plan_review:29, ready:30, next_plan:31, builder:32, review:33,
verify:34, candidate:35, integrate:36, offline:37, post:38, approval:39, notify:40, broker:41,
rollback:42`. The `components` array has 32 entries (indices 0–31), so 32–42 are out of range
entirely; 27–31 are in range but resolve to the **wrong node** — a 4B agent told "planner's LLD
authority is `architecture.json:27`" lands on `post`. The numbers are consistently
(1-based ordinal + 10), which suggests a garbled scheme rather than a lookup anyone performed.
`candidate:5` (`json:35,81-83`) and `approval:5` (`json:39,67,76,78-79`) cite ranges that exist in
no addressing scheme for this file.

**P1-11 — `ProbeOutput` and `ProbeError` have no owner.**
`probe_business:28` defines `pub enum ProbeOutput`. `probe_tech:26` returns `ProbeOutput` from a
**different crate** with no dependency on `probe-business` declared in its Cargo.toml (`:51-57`).
`ProbeError` is used in `probe_business:26`, `probe_tech:25`, `probe_learn:25` and defined in none.
(`ProbeError` does exist in-tree — 37 hits — but no blueprint says which crate re-exports it to the
probes.) Same class as P0-3: shared vocabulary with no declared home.

**P1-12 — `scan` cannot tell a builder which crate it is building.**
`scan:25` specifies `crates/scan/{Cargo.toml,src/lib.rs,…}`. `scan:78-82` then checks every step with
`cargo check -p fleet-scan` / `cargo test -p fleet-scan`, and `:113` runs
`cargo test -p fleet-scan materiality` while `:115` runs `find crates/scan -name '*.rs'`. New crate
or edit-in-place? The blueprint answers both. `scan` is also the only file using a one-line brace
layout for §4, so it diverges from the template as well.

---

## P2 Warnings (fix during build)

- **`control:118` / `intent:117` prove a real binary with `--help`.** `cargo run --bin fleet --
  __spawn_probe --help` and `__pipeline_probe --help` exit 0 without executing the node. An API 200
  is not a rendered page; a `--help` is not a spawn. Both nodes elsewhere demand real fd-3 frames —
  the §12 command should match that bar.
- **`README.md:49-52` omits the four probes from the build waves entirely** (wave 1 lists
  `ingest, lifecycle …, connectors, intent, scan, questions`), and invents a `lifecycle` node that
  is not one of the 32. `IMPLEMENTATION-ORDER.md:9-11` *does* include the probes in wave 2.
  REVIEW-GATE.md:53 requires the map and order to be "internally consistent".
- **`context:5` cites `architecture.json:28`** — same defect as P1-10; listed separately because
  `context` is otherwise one of the strongest files and is the only node whose outgoing payload name
  matches the LLD exactly.
- **`context:56` claims `tiktoken-rs 0.12.0` "local tested".** The lock contains both 0.6.0 and
  0.12.0. The claim is defensible but "local tested" is an observation claim (AGENTS.md:10-12) and
  should name the test that ran.
- **`integrate`, `post`, `rollback` leave Git unpinned** ("runtime-discovered", "version is
  unverified here"). Defensible for a system tool, but `git merge-tree --write-tree`
  (`integrate:74`) has a hard minimum version. Record it.
- **`next_plan:31` consumes `PlanDraft`** from `planner` with no cross-crate dependency declared in
  its §4 layout.
- **`questions:23` takes `Vec<Question>`** but its four upstreams emit three different shapes
  (`ProbeOutput`, `Vec<Question>`, `ResearchOutput`). No node owns the flattening.
- **`probe_business:152` allows `src/lib.rs` ≤80 lines** while `:19` specifies ≤30. The DoD check is
  looser than the spec it checks.
- **`user_cli:66`, `ingest:65`, `intent:62`, `ready:51`, `candidate:55` cite serde/clap with no
  version or license** ("pinned by workspace lock", "versions must be read at implementation time").
  AGENTS.md:29-31 requires version, license, source, smoke and limitation for every choice. The lock
  has them: `clap 4.6.6`, `serde 1.0.229`, `thiserror 2.0.20`.
- **No blueprint names the workspace `thiserror` correctly-verified pin as a positive.** For the
  record: `scan:60` and `probe_business:46` say `thiserror 2.0.20`, and member manifests do declare
  `thiserror = "2.0.20"`. That claim checks out — it is the model the version rows should follow.

---

## Per-Node Scorecard

Legend: ✓ meets bar · ~ partial · ✗ fails. Grade: A (build-ready) · B (build after P1s) ·
C (needs revision) · D (needs rewrite of the named sections).

| Node | Library spec | Stub-proof | 4B-buildable | Edge types | Denominators | DoD binary | Grade |
|------|-------------|-----------|-------------|-----------|-------------|-----------|-------|
| approval | ~ | ~ | ✓ | ✗ | ✓ | ~ | C |
| broker | ✓ | ~ | ✓ | ✗ | ✓ | ~ | B |
| builder | ✓ | ~ | ✓ | ✗ | ✓ | ~ | B |
| candidate | ~ | ~ | ✓ | ✗ | ✓ | ~ | C |
| connectors | ✓ | ~ | ✓ | ✗ | ✓ | ~ | B |
| context | ✓ | ~ | ✓ | ~ | ✓ | ~ | B |
| control | ✓ | ~ | ~ | ✗ | ✓ | ~ | C |
| dag | ~ | ~ | ✓ | ✗ | ✓ | ~ | C |
| ingest | ~ | ✓ | ✓ | ~ | ✓ | ~ | B |
| integrate | ~ | ~ | ✓ | ~ | ✓ | ~ | B |
| intent | ~ | ~ | ✓ | ~ | ✓ | ~ | C |
| knowledge | ✓ | ~ | ✓ | ✗ | ✓ | ~ | B |
| model_catalog | ✓ | ~ | ✓ | ✓ | ✓ | ~ | B |
| next_plan | ✓ | ~ | ~ | ✗ | ✓ | ~ | C |
| notify | ✓ | ~ | ✓ | ✗ | ✓ | ~ | B |
| offline | ✓ | ~ | ✓ | ✗ | ✓ | ~ | B |
| plan_review | ✓ | ~ | ~ | ✗ | ✓ | ~ | C |
| planner | ✓ | ✓ | ✓ | ✗ | ✓ | ~ | B |
| post | ~ | ~ | ✓ | ✗ | ✓ | ~ | C |
| probe_business | ~ | ✓ | ✗ | ✗ | ✓ | ✓ | D |
| probe_learn | ✗ | ✓ | ✗ | ✗ | ~ | ✓ | D |
| probe_research | ~ | ✓ | ✗ | ✗ | ✓ | ✓ | D |
| probe_tech | ~ | ✓ | ✗ | ✗ | ✓ | ~ | D |
| questions | ~ | ~ | ✓ | ✓ | ✓ | ~ | B |
| ready | ~ | ~ | ✓ | ✗ | ✓ | ✓ | C |
| review | ✓ | ~ | ✓ | ✗ | ✓ | ~ | B |
| rollback | ~ | ~ | ✓ | ✗ | ✓ | ~ | C |
| route | ✓ | ~ | ✓ | ✓ | ✓ | ~ | B |
| scan | ✓ | ~ | ✗ | ✗ | ✓ | ✓ | C |
| store | ~ | ~ | ✓ | ✗ | ✓ | ~ | C |
| user_cli | ✗ | ✓ | ✓ | ✗ | ✗ | ~ | C |
| verify | ✓ | ~ | ✓ | ✗ | ✓ | ~ | B |

Totals: Library spec 15✓/15~/2✗ · Stub-proof 6✓/26~ · 4B-buildable 26✓/3~/5✗ ·
**Edge types 3✓/4~/25✗** · Denominators 30✓/1~/1✗ · DoD binary 4✓/28~.

Note on "Stub-proof ~": 26 nodes carry a strong 4–6 row mutation table naming constant-return,
empty-success, skipped-input and dropped-receipt mutants — that part is good. They score ~ rather
than ✓ because their integration tests are described by *behavior* ("`scan -> dag -> planner/ready`;
`checked=16,total=16`") and never by *name*, so nothing pins which test a mutation must kill. The
four probes are the only files that name exact tests with exact assertions — which is why they are
the right pattern and simultaneously the worst-graded, since those named tests do not compile.

---

## Nodes requiring blueprint revision (≥2 criteria fail)

Strictly by the ≥2-✗ rule:

| Node | Failing criteria |
|---|---|
| `probe_business` | 4B-buildable, edge types |
| `probe_learn` | library spec, 4B-buildable, edge types |
| `probe_research` | 4B-buildable, edge types |
| `probe_tech` | 4B-buildable, edge types |
| `scan` | 4B-buildable, edge types |
| `user_cli` | library spec, edge types, denominators |

All six need a §5/§10 revision before any builder is dispatched. The remaining 26 need only the
cross-cutting fixes (P0-5 edge types, P0-6 FR trace, P1-6 mutation floors, P1-10 LLD pointers),
which are mechanical once the canonical type names are settled.

---

## Reviewer's own obligations (REVIEW-GATE.md:45-46)

**One invariant re-derived.** `scan`'s materiality predicate (`scan:47`):
`material iff effect_changes OR acceptance_changes OR missing_grant OR blocks_ready_node`.
Re-derived from LLD §6 and the node's own non-goals: `scan` may authorize only bounded read probes
and must not spend probe budget on cosmetic ambiguity. Each of the four terms is independently
sufficient — a missing grant alone blocks the effect regardless of acceptance, and a blocked ready
node alone blocks progress regardless of grants — so OR is correct and the `scan:102` "replace OR
with AND" mutant is the right kill target. The predicate is sound. Its *input* is not: `ScanInput`
(`scan:35`) carries `unknowns` and `revision` but the LLD's incoming payload is `RouteDecision`, so
the four booleans have no declared provenance from `route`'s output (P0-5).

**One hidden-risk path read.** `broker:17` — PREPARED committed before the call, DISPATCHING before
network I/O, crash after dispatch is UNKNOWN resolvable only by provider readback or a documented
idempotency guarantee. This is correct and is the strongest single paragraph in the set; `broker:96`
("mark timeout as acked") and `:101` (reviewer flips `Unknown` to immediate retry) kill the two real
failure modes. `broker` is missing only a mutation floor (P1-6).

**One mutation applied by hand.** Not a source mutation — there is no source yet — so I applied the
equivalent at the specification layer, which is where this set lives. I took `probe_business`'s
§13 item 3 exactly as written and evaluated it against clippy's actual output shape:
`cargo clippy … | grep -qv "^error"` returns 0 whenever any emitted line lacks the `error` prefix,
which is always true (`Checking probe-business v0.1.0`). The gate is unkillable by a failing build.
Recorded as P1-7. I separately confirmed the `probe_tech` `[89]` regex breaks at 10 tests (P1-8).

---

## What is genuinely strong (do not regress it in the fix pass)

- **Every reuse citation is real.** All 32 cited source paths exist, verified individually
  (`fleet-router/src/decide.rs`, `fleet-scan/src/merge.rs:23-46`, `fleet-worker/src/spawn/fd3.rs`,
  `keel/fleet/src/main.rs:2428-2661`, `fleet-crew/crew/adapters/capability_probe.py`, …). Several
  cite exact line ranges that land correctly. This is rare and worth protecting.
- **Honest negatives are recorded rather than papered over**: `ingest:67` flags `unwrap_or_default`
  as a rule violation in the code it extracts from; `questions:36` flags the legacy cap-4 against the
  LLD's cap-3; `probe_learn:37` flags `f32` relevance as unfit for an authority threshold;
  `dag:48` refuses to claim O(V+E) for a `Vec::remove(0)` implementation; `connectors:21` names the
  bearer-token/IMAP gap as non-compliant; `store:69` calls the redb→SQLite move a migration, not an
  extraction. That is the discipline the estate says it wants.
- **Zero-input gates are near-universal**: 30 of 32 publish `checked,total` with an explicit
  "`0/0` never passes" rule.
- **The four upgraded probes are the right template** — named tests, function-level mutation
  targets, verbatim Cargo.toml, shell-assertion DoD. Fix their type errors and propagate that shape
  to the other 28 rather than reverting them.

## Minimum close-out for a PASS

1. Publish `docs/blueprints-next/EDGE-TYPES.md`: one row per `connections[]` entry — payload type,
   owning crate, producer, consumer, canonical test id. Make the 33 payload names and the 43 test
   ids the only permitted spelling, then reconcile all 32 blueprints to it. (Closes P0-2, P0-3,
   P0-5, P1-11, and most of the C grades.)
2. Add `crates/fleet-types/src/acp.rs` as a named wave-0 prerequisite, or delete the ACP row from
   `probe_business`. (P0-1)
3. Reconcile §5 and §10 in the four probes to one function set. (P0-4)
4. Publish `docs/blueprints-next/FR-TRACE.md` mapping FR1–FR46 → node + required observation, and
   open a blueprint (or an explicit deferral ADR) for the FR43 multi-repo saga, FR11 `.fleet/`
   config, and FR12/29 skills-MCP injection. (P0-6)
5. Correct `tantivy` to 0.26.1, `tree-sitter` to 0.24.7, and resolve `rusqlite` 0.32.1-vs-0.40.2
   in one place. (P1-1, P1-2, P1-3)
6. State a mutation floor and denominator in the 15 blueprints that have none, ≥80% for
   `approval, broker, control, integrate, post, rollback, route`. (P1-6)
7. Fix `user_cli` to 34 commands, fix the two broken shell gates, and normalize the 14 numeric LLD
   pointers to `:<node-id>`. (P1-5, P1-7, P1-8, P1-10)
