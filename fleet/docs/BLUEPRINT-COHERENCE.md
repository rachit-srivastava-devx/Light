# Blueprint coherence audit

Audit date: 2026-08-24. Scope: the 22 numbered files in
`../blueprints/Fleet-L8-Deep-Dive/` (`00` through `21`). `AGENTS.md` and `README.md` in that
directory are supporting metadata, not additional numbered files.

## Result

The blueprint makes **155** atomic, falsifiable claims about fleet's behaviour or operational
capability. **44/155 are implemented (28.4%)**. The remaining denominator is not hidden:

| result | count | share |
|---|---:|---:|
| IMPLEMENTED | 44 | 28.4% |
| PARTIAL | 62 | 40.0% |
| ABSENT | 49 | 31.6% |
| WITHDRAWN | 0 | 0.0% |
| **TOTAL** | **155** | **100.0%** |

The arithmetic is checkable by summing the per-file rows below. `PARTIAL` means a real slice exists
but one or more load-bearing parts of the blueprint claim are missing. `ABSENT` means this checkout
has no implementation or executable proof. No claim is marked `IMPLEMENTED` from prose alone.

`WITHDRAWN` is unused: none of the 155 extracted capability claims is explicitly withdrawn by a
`D<n>` in the blueprint. A later file superseding a design is not treated as withdrawal; the current
implementation is still measured against the latest claim.

## Per-file denominator

| file | implemented | partial | absent | withdrawn | total |
|---|---:|---:|---:|---:|---:|
| 00-EVIDENCE-BASE | 2 | 1 | 0 | 0 | 3 |
| 01-CONSTRAINT-AND-THESIS | 3 | 3 | 1 | 0 | 7 |
| 02-ARCHITECTURE-TWO-LANGUAGE-KERNEL | 1 | 4 | 2 | 0 | 7 |
| 03-REPOSITORY-STRUCTURE | 2 | 3 | 2 | 0 | 7 |
| 04-EVIDENCE-LEDGER | 2 | 4 | 2 | 0 | 8 |
| 05-GATE-KERNEL-AND-UNFAKEABILITY | 0 | 4 | 3 | 0 | 7 |
| 06-WORK-LIFECYCLE-STATE-MACHINE | 5 | 2 | 1 | 0 | 8 |
| 07-ISOLATION-AND-SUPERVISION | 2 | 3 | 2 | 0 | 7 |
| 08-LEARNING-LOOP-AND-ANTI-REPEAT | 0 | 1 | 5 | 0 | 6 |
| 09-CODE-GRAPH-AND-BLAST-RADIUS | 2 | 4 | 1 | 0 | 7 |
| 10-PROVING-SERVICE-GRADE | 3 | 7 | 1 | 0 | 11 |
| 11-OPERATOR-CONSOLE-UX | 2 | 1 | 4 | 0 | 7 |
| 12-KEYLESS-TOKENOMICS-AND-ROUTING | 3 | 3 | 1 | 0 | 7 |
| 13-OSS-STACK-AND-UPSTREAM-DELTA | 1 | 2 | 3 | 0 | 6 |
| 14-CAPACITY-AND-FANOUT-MATH | 0 | 0 | 4 | 0 | 4 |
| 15-FAILURE-MODES-AND-DEGRADATION | 1 | 3 | 1 | 0 | 5 |
| 16-PHASES-TO-USABLE | 1 | 3 | 4 | 0 | 8 |
| 17-CROSS-EXAMINATION | 4 | 3 | 3 | 0 | 10 |
| 18-HONEST-LIMITS | 2 | 2 | 2 | 0 | 6 |
| 19-INDUSTRY-ALIGNMENT | 0 | 2 | 3 | 0 | 5 |
| 20-BUILD-VS-ADOPT-RECKONING | 2 | 3 | 4 | 0 | 9 |
| 21-THE-FOUR-OBJECTS | 6 | 4 | 0 | 0 | 10 |
| **TOTAL** | **45** | **61** | **49** | **0** | **155** |

## Evidence key

Line references point at this checkout as audited. Commands are executable proof only when run by
the verifier; the required verifier result is recorded separately in the handover fragment.

| key | resolving evidence |
|---|---|
| E1 | `keel/fleet/src/main.rs:856-1172` — run, non-empty diff, BLAKE3 artifact, verifier, blind-suite measurement, adequacy, rollback, attestation, observation |
| E2 | `keel/fleet/src/main.rs:1481-1639` — refusal receipts, O1/O2 registration, author separation, oracle execution |
| E3 | `keel/fleet/src/main.rs:1648-1777` — four-quadrant adjudication and attestation update |
| E4 | `keel/fleet/src/main.rs:2922-3038` — locked hash-chain append and streaming verification with denominator |
| E5 | `keel/fleet/src/lifecycle.rs:15-31,178-317,634-731` — typed and runtime lifecycle edges |
| E6 | `crew/crew/sow.py:40-80,203-317,367-461,513-561` — atomic SOW predicates, citations, questions, alternatives, estimate, edge cases, exit-7 refusal |
| E7 | `keel/fleet/src/roles.rs:32-110,128-167` and `keel/fleet/src/swarm.rs:142-247` — roles, owned gates, verified dispatch |
| E8 | `keel/fleet/src/main.rs:2211-2335,2714-2814` — env-cleared worker, fd 3 result, deadlines, adapter bridge, submission validation |
| E9 | `keel/fleet/src/main.rs:2181-2198,1824-1945` — immutable 0444 artifact and recorded rollback |
| E10 | `keel/fleet/src/agent.rs:1-291,330-464` and `agents.toml` — persistent agents, assignments, scorecards, amended outcomes |
| E11 | `keel/fleet/src/mcp.rs:61-112,130-289,292-470` — lease-derived tools, path containment, refusal receipts |
| E12 | `keel/fleet/src/graph.rs:285-497,696-1035,1128-1247` — index/impact, tree-sitter Rust/Python/Bash, SQLite closure, stale/floor checks |
| E13 | `keel/fleet/src/graph.rs:1249-1326` — reachability, non-empty/known-callers/rename/false-positive/language tests |
| E14 | `keel/fleet/src/route.rs:118-180,276-469,485-670` — deterministic staged routing and adapter eligibility |
| E15 | `keel/fleet/src/meter.rs:138-190,267-520,546-670` — integer reservations, unknown-as-null, observations |
| E16 | `keel/fleet/src/console.rs:35-109,220-524,576-940,1003-1090` — TUI views, source-tagged unknowns, denominators, snapshots |
| E17 | `keel/fleet/src/ratchet.rs:14-33,153-189,252-380,382-465` — Wilson interval, ratchet, denominator/refusal rules |
| E18 | `keel/fleet/src/skills.rs:44-229` and `skills.toml` — skill registry resolution and zero-input failure |
| E19 | `contracts/attestation.v1.json`, `contracts/receipt.v1.json`, `contracts/submission.v1.json` — in-toto subject, receipt fields, worker-owned body only |
| E20 | `keel/fleet/src/main.rs:3500-3564` — reachable CLI surface, exit-code contract, plan/meter/graph/console commands |
| E21 | `tests/corpus/run.sh`, `tests/corpus/MANIFEST.sha256`, `tests/acceptance/`, `bin/mutants-gate.sh` — executable corpus, integrity, acceptance and mutation lanes |
| E22 | `README.md:1-13,15-78` — local macOS scope, quickstart, limitations and explicit trust boundary |

## Claim ledger

### 00 · EVIDENCE-BASE

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 00.1 | The failure corpus is executable, one detector per failure, and integrity sealed. | IMPLEMENTED | `tests/corpus/run.sh`, `tests/corpus/MANIFEST.sha256`, and the checked-in `tests/corpus/*.sh` fixtures resolve the execution and seal surfaces (E21). |
| 00.2 | Fleet's evidence record publishes typed receipts and denominators rather than bare pass labels. | IMPLEMENTED | Receipt creation/verification requires `seq`, hashes, event, actor, body and prints `checked=... total=...` (E4); the contract is explicit (E19). |
| 00.3 | External capability adoption is backed by a passing smoke command recorded in the repository. | PARTIAL | The blueprint has a register table, but no executable adoption register/gate is present; the available smoke evidence is documentation and the general verifier. |

### 01 · CONSTRAINT-AND-THESIS

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 01.1 | Intake turns a sentence into a specified task and refuses ambiguity. | PARTIAL | Deterministic SOW construction/refusal exists (E6), but the blueprint's measured intake timing and four-stage ambiguity experiment are not produced by the current CLI. |
| 01.2 | Decomposition turns a task into owned worker briefs. | ABSENT | SOW leaves exist, but no brief contract/decomposition producer or owned-brief artifact is present in this checkout. |
| 01.3 | A worker can produce a real diff that fleet freezes as an artifact. | IMPLEMENTED | `run_with_evidence` requires a diff, hashes it, freezes it, and records `artifact_frozen` (E1, E9). |
| 01.4 | Verification is independent of the builder and is run before attestation. | IMPLEMENTED | `verifier_for`, `SELF_VERIFIED` refusal, `run_verifier`, and the verification receipt are in E1. |
| 01.5 | A human accepts responsibility at the acceptance station. | PARTIAL | SOW acceptance requires `USER`/`LOGNAME` and records an acceptance receipt, and typed lifecycle acceptance requires `HumanApproval` (E5); the full human accept station is not wired into every run. |
| 01.6 | Attestation survives the run as evidence derived from the run. | IMPLEMENTED | The run writes a named in-toto-style attestation from receipts and later updates it with adjudication (E1-E3, E19). |
| 01.7 | The thesis makes the producer unable to forge the verdict, rather than merely asking it not to. | PARTIAL | Worker environment/IPC separation is real (E8), but the parent is one local operator and the design explicitly remains tamper-evident rather than tamper-proof (E22). |

### 02 · ARCHITECTURE-TWO-LANGUAGE-KERNEL

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 02.1 | Fleet has five separated planes with Rust refusal and Python proposal halves. | PARTIAL | Rust modules, Python `crew`, adapters and console exist, but the blueprint's separate Rust crates/planes are not the repository structure and execution crosses both halves. |
| 02.2 | Anything that can refuse is Rust; Python only proposes, generates, ranks or measures. | PARTIAL | Python SOW/refusal and Rust refusal paths both exist (E6, E20); Rust also launches model adapters in `run_model_agent` (E8), so the boundary is not coherent. |
| 02.3 | `crew` cannot construct a Rust `Verdict`. | ABSENT | No cross-language type-boundary check or generated PyO3 verdict boundary exists in the checkout. |
| 02.4 | `keel` opens no network, spawns no model and reads no clock directly. | ABSENT | `main.rs` invokes model adapters and uses `SystemTime`/`now_rfc3339`; the blueprint claim is contradicted by E1/E8. |
| 02.5 | A worker has no ledger/state/socket path and can submit only over inherited fd 3. | IMPLEMENTED | Parent clears the environment, supplies only an allowlist, dup2s the socket to fd 3, closes stdin/stdout/stderr, validates the packet, and never gives `FLEET_STATE` to the child (E8). |
| 02.6 | Both languages generate their wire types from `contracts/`, so schema drift is a build error. | PARTIAL | Three JSON contracts and explicit submission validation exist (E19); Rust and Python still contain hand-written serde/dataclass wire structures and no generation step. |
| 02.7 | A new harness adapter is admitted by the seven-capability portability contract and degrades honestly when a capability is absent. | PARTIAL | Claude/Codex adapters expose non-interactive invocation, usage, model readback and operator credentials; capability probing exists (E8), but no seven-capability admission gate or degraded attestation tier is wired. |

### 03 · REPOSITORY-STRUCTURE

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 03.1 | `fleet <verb>` is the one front door and loose scripts are not alternate entrypoints. | IMPLEMENTED | `main` dispatches the public verbs and prints the command surface (E20). |
| 03.2 | The repository has the five structural planes and three storage classes exactly as specified. | PARTIAL | `$FLEET_STATE` authority, `var/` scratch and `crew`/`keel` exist; the specified multi-crate tree, tracked `state/lessons` and `state/promotions` do not. |
| 03.3 | An architecture gate refuses parallel roots, duplicate suffix implementations, misclassified paths and loose docs. | ABSENT | No `keel-gate::arch` implementation or command is present; "fleet arch" is not a command. |
| 03.4 | The task blind suite is outside the repository and its reachability is measured before verification. | IMPLEMENTED | `measure_blind_suite` creates/validates the external suite, checks git-object reachability and fd exposure, then places the result in the attestation (E1). |
| 03.5 | `contracts/` is the only wire vocabulary and hand-written wire types are rejected. | PARTIAL | Contracts are present and validated (E19); no arch detector rejects hand-written matching types. |
| 03.6 | Every fixture pairs a bad input with a good control, and slow checks have their own budgeted lane. | PARTIAL | Corpus and console fixtures include controls and bad values (E21, E16); no universal pair gate or slow-lane budget enforces the claim. |
| 03.7 | An adoption gate refuses a new capability without a register row and passing smoke command. | ABSENT | No adoption registry/gate exists in the CLI or source tree. |

### 04 · EVIDENCE-LEDGER

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 04.1 | Receipts are append-only JSONL, BLAKE3 chained, parent-stamped, and include integer/nullable model fields. | IMPLEMENTED | `append_receipt` takes the actor/model/exit code, timestamps and hashes the row under a lock; `validate_receipt_row` enforces the schema (E4, E19). |
| 04.2 | Worker-authored evidence is limited to the body; timestamp, actor, model, lease and hash cannot be supplied by the worker. | PARTIAL | Submission schema omits those fields (E19) and parent stamps most receipt fields (E4); the body remains unrestricted JSON and the bridge returns `resolved_model` inside it, so the strict ownership claim is not fully enforced. |
| 04.3 | A single-writer ledger daemon makes concurrent appends race-free. | PARTIAL | The current implementation serializes read-modify-write with `FileLock` and validates the chain (E4); there is no single-writer daemon/socket append loop. |
| 04.4 | Chain verification is one streaming pass and publishes its denominator. | IMPLEMENTED | `verify_rows` carries the previous hash once through the rows and `ledger verify` prints equal checked/total counts (E4). |
| 04.5 | Checkpoints and quarantined ledger segments bound recovery and report discontinuities per segment. | ABSENT | No checkpoint or segment/quarantine implementation is present; the checkout has one chain path and one verifier. |
| 04.6 | A broken row is preserved as an untrusted quarantined segment while service continues on a new anchored segment. | ABSENT | No segment recovery command, quarantine state, or anchored new segment exists. |
| 04.7 | The delivery attestation is a replayable view derived from named receipts and mismatches exit 8. | PARTIAL | `attest verify` and receipt-backed attestation generation exist (E1-E4); the attestation is initially written with a minimal pending shape and not every blueprint element is re-derived. |
| 04.8 | Receipt-write failure is fatal and there is one durable destination rather than ignored/temp output. | PARTIAL | Append errors propagate and `sync_data` is used (E4); there is no daemon and the implementation remains a local file ledger, so the blueprint's single durable writer boundary is not met. |

### 05 · GATE-KERNEL-AND-UNFAKEABILITY

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 05.1 | Every gate implements a pure `Gate` interface returning a denominator-bearing `Verdict` with bad/control fixtures and fake-cost analysis. | ABSENT | No `Gate` trait, `Verdict` gate registry, or `fake_cost` interface exists. |
| 05.2 | Gate design rejects a cheaper forgery and provides a legitimate sentinel path. | ABSENT | Agent/model checks exist, but no generic incentive-soundness or sentinel gate is implemented. |
| 05.3 | A check of zero inputs fails, and every decision publishes `{checked,total}`. | PARTIAL | Ratchet rejects zero totals on error, and adequacy prints rate/count/verdict (E17, E20). But not every zero-input surface refuses: `fleet status --json` against a genuinely empty `FLEET_STATE` is a deliberate cold-start (exit 0, not exit 6) rather than a failure — B14 decided a fresh install with zero tasks run is a legitimate state, not an unmeasured one, so forcing a refusal there would be more surprising than helpful. What the claim's second half still requires is honored: the JSON always publishes `{checked,total}` (`0,0` on cold start) plus an explicit `"empty":true` marker that cannot be mistaken for "20/20 checked, all fine", and the human render says "empty store" in words. See `docs/delta.d/B14.md` and `tests/acceptance/status.sh`. |
| 05.4 | New gates are red-first: bad fixture refuses, control accepts. | PARTIAL | Specific tests exercise both directions for graph, console, skills and role checks (E13, E16, E18); no common gate trait enforces this for every new gate. |
| 05.5 | Each gate's own failing path is exercised every run and mutation-tested. | PARTIAL | Corpus/mutation commands exist (E21), but there is no registry proving every production guard was exercised on each verifier run. |
| 05.6 | Resolved model attribution uses a recognized registry and a no-model sentinel; unknown models refuse. | PARTIAL | Adapter capability probing and distinct builder/verifier checks exist (E8, E14); there is no recognized model registry/sentinel contract for all operations. |
| 05.7 | `evalgate` enforces the six architecture assertions and ratchets a published baseline. | ABSENT | No `evalgate` or six-assertion architecture gate exists. |

### 06 · WORK-LIFECYCLE-STATE-MACHINE

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 06.1 | Illegal task lifecycle transitions are unrepresentable in the typed API and checked again at runtime. | IMPLEMENTED | Typestate markers consume tasks for legal transitions; runtime `STATES` validates the same edge before persistence (E5). |
| 06.2 | Fleet plans first, emits exit 9, requires human SOW acceptance, and refuses execution without it. | IMPLEMENTED | `sow` creates a ready record and exits 9; `sow accept` records the actor; `enforce_accepted_sow` gates run (E6, E20). |
| 06.3 | A SOW has atomic one-predicate leaves, cited challenges, traceable clarifications, alternatives, estimate and edge cases. | IMPLEMENTED | `crew.sow` implements and validates all of those fields and refuses malformed input (E6). |
| 06.4 | Five roles each own a failable gate and can be advanced through a verified swarm dispatch. | IMPLEMENTED | Role definitions expose ownership/write permission and swarm dispatch creates verified agent states/scorecards (E7). |
| 06.5 | Lead implementation and self-verification are refused while builder/verifier separation is accepted. | IMPLEMENTED | Role checks cover both directions and run refuses `SELF_VERIFIED`; E1 and E7 cite both paths. |
| 06.6 | A ratchet/high-water mark and control bands persist outside the judged tree. | PARTIAL | Ratchet marks and locked scorecards live under `$FLEET_STATE` (E17); no control-band YAML/state machine exists. |
| 06.7 | Failed verification enters a bounded reviewer/rework loop with explicit retry evidence. | PARTIAL | Failure and rollback receipts exist, and status derives `NeedsIteration` (E9, E16); no complete bounded retry loop owns the next dispatch. |
| 06.8 | Control bands automatically open one problem per breached metric and rate-limit repeated breaches. | ABSENT | No control-band evaluator or breach queue is present. |

### 07 · ISOLATION-AND-SUPERVISION

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 07.1 | Worktree leases are durable records, released at submission, and returned only after the captured diff is checked. | PARTIAL | Lease-scoped MCP and scratch worktrees exist (E9, E11); no durable lease registry/pool or explicit submission-time release exists. |
| 07.2 | Path ownership has explicit exact-path versus prefix semantics and rejects traversal/out-of-lease writes. | PARTIAL | MCP normalizes paths, rejects `..`, and enforces the leased prefix (E11); the richer exact-path/trailing-slash lease contract is absent. |
| 07.3 | The parent snapshots the shared surface and binds the verdict to a tree/artifact digest. | IMPLEMENTED | Clean-tree precondition, diff capture, BLAKE3 artifact id and immutable artifact store bind the run to bytes (E1, E9). |
| 07.4 | Process groups, deadlines, escalation and guaranteed reaping prevent orphan workers. | IMPLEMENTED | `setsid`, deadline polling, SIGTERM/SIGKILL escalation and `wait` are implemented (E8). |
| 07.5 | Exit 0 is believed only with a log-size floor and a non-empty diff. | PARTIAL | The Rust path requires a non-empty/new diff (E1); the generic log floor is in Python adapter configuration, not enforced for every Rust/stub path. |
| 07.6 | Liveness is measured by output growth and stalls are interrupted before the deadline. | ABSENT | The supervisor has a fixed deadline but no output-growth sampling or stall window. |
| 07.7 | Read/write-set intersections, including state stores, are checked before dispatch and cap fan-out at W=4. | ABSENT | No read/write-set scheduler or derived W=4 cap exists. |

### 08 · LEARNING-LOOP-AND-ANTI-REPEAT

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 08.1 | Every failure is automatically captured in one queryable funnel from observed to enforced. | ABSENT | Receipts and REPL history exist, but no lesson/candidate/promoted/enforced store or funnel is implemented. |
| 08.2 | Refusal capture is automatic and distinct failures cannot overwrite one another. | PARTIAL | Refusal receipts are automatically appended with typed bodies (E4); no lesson row/dedup key or enforcement promotion follows them. |
| 08.3 | Promotion requires independent bad/control fixtures and both directions. | ABSENT | No promotion engine or lesson fixtures exist. |
| 08.4 | Hybrid lexical+dense retrieval publishes recall, false-positive and coverage denominators. | ABSENT | No tantivy/usearch/model2vec lesson index or retrieval measurement exists. |
| 08.5 | The anti-repeat gate evaluates promoted signatures against diffs and refuses with lesson identity. | ABSENT | No `recur` command or promoted-signature evaluator exists. |
| 08.6 | The honest 57.4% mechanisable promise is measured and enforced rather than stated. | ABSENT | The corpus recount is blueprint prose; no promoted/mechanisable coverage metric is implemented. |

### 09 · CODE-GRAPH-AND-BLAST-RADIUS

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 09.1 | Impact analysis runs before brief/lease creation and feeds lease scope, acceptance scope and console ranking. | PARTIAL | `fleet impact` is reachable and returns dependents (E12); no lifecycle hook connects it to brief/lease creation or acceptance-suite generation. |
| 09.2 | The parse spine covers Rust, Bash, Python and TypeScript. | PARTIAL | Tree-sitter parsing covers Rust/Bash/Python in `language_for` and `parse_file`; TypeScript is not admitted (E12). |
| 09.3 | Symbols have stable canonical ids, aliases and rename-tolerant history. | PARTIAL | SQLite symbols/aliases and rename tests exist; new unmatched symbols use fresh UUIDs and the claimed full history/co-change model is not present (E12, E13). |
| 09.4 | Known-callers, known-rename and false-positive controls are mandatory and reject a silent zero. | IMPLEMENTED | The graph test module asserts non-zero known callers, alias preservation, zero false positives and zero-denominator refusal (E13). |
| 09.5 | Impact uses bounded recursive SQLite closure and publishes the depth. | IMPLEMENTED | `query_dependents` uses recursive CTE, `UNION`, depth bound and returns depth/path/symbol fields (E12). |
| 09.6 | Stale index, below-floor index and language/file coverage are reported on every query. | PARTIAL | Tree digest, file floor and stored-count checks refuse stale/undersized indexes (E12); query output lacks the blueprint's complete language/file-skipped coverage block. |
| 09.7 | Incremental indexing and pydriller changed-method co-change coupling are wired into the graph. | ABSENT | No pydriller/history/coupling implementation is present in `keel` or `crew`. |

### 10 · PROVING-SERVICE-GRADE

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 10.1 | Accepted changes carry all nine structural delivery elements with proportional tiers. | PARTIAL | The generated T-min attestation contains SOW, blind suite, verification, adequacy, blast radius, rollback, cost and oracle independence (E1, E19); reachability and tier enforcement are absent. |
| 10.2 | The SOW element proves human acceptance with a matching chain receipt. | IMPLEMENTED | SOW acceptance writes an actor/time receipt and execution verifies the receipt body against the exact task SOW (E6, E20). |
| 10.3 | Blind-suite independence proves four independent absences. | PARTIAL | Suite location, git-object reachability and fd exposure are measured (E1); the complete recorded environment allowlist and four-field attestation contract are not present. |
| 10.4 | Independent verification requires distinct agent ids and distinct provenance families. | PARTIAL | Distinct builder/verifier ids and resolved model readback exist (E1, E8); no provider/provenance-family registry is enforced. |
| 10.5 | Mutation adequacy has a pinned engine, Wilson interval, non-decreasing mutant count and a two-sided ratchet. | PARTIAL | Wilson arithmetic, kill/total validation and ratchet marks exist (E17); engine/version and mutant-count integration into the run are not complete. |
| 10.6 | Parent-computed graph blast radius equals post-run diff and outside-lease writes quarantine the artifact. | PARTIAL | The run records changed diff files/count and MCP rejects out-of-lease paths (E1, E11); graph-versus-diff equality and quarantine are not wired. |
| 10.7 | Rollback is executed, tested in both applied/reverted states, and its result is attested. | IMPLEMENTED | `rollback_artifact` runs the suite around reverse application; rollback metadata is placed in the attestation and receipt (E1, E9). |
| 10.8 | Cost records carry source, tokens, tokenizer generation and characters, never a fabricated zero. | PARTIAL | Meter stores integer/unknown observations and the attestation records characters/tokens fields (E1, E15); the normal run records tokens and tokenizer generation as null rather than reading a dated priced transcript. |
| 10.9 | O1/O2 are independently authored, hashed, executed and adjudicated by a table rather than a model. | IMPLEMENTED | Registration rejects same authors, execution hashes outputs, and the four-quadrant table records fault/credit (E2, E3). |
| 10.10 | Every shipped module is reachable from the user-facing entrypoint and that edge is part of acceptance. | PARTIAL | Reachability analysis and non-empty tests exist (E13); it is not an attestation element or acceptance check for every run. |
| 10.11 | The junior/senior delivery-equivalence claim is proven by the pre-registered paired statistical test. | ABSENT | No `crew/eval/paired.py`, SWE-bench paired runner, or equivalence result is present. |

### 11 · OPERATOR-CONSOLE-UX

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 11.1 | The operator console answers what is running, blocked and what it cost, with task and agent views. | IMPLEMENTED | TUI command/help and `render_fleet`, `render_task`, `render_agent` implement the views and receipt-derived rollup (E16, E20). |
| 11.2 | The graph is server-side laid out/rendered as SVG, with adjacency-table fallback when graphviz is absent. | ABSENT | Console is ratatui terminal rendering; no axum server, SVG renderer or graphviz fallback exists. |
| 11.3 | A derived legibility floor controls graph selection/zoom. | ABSENT | No graph layout, zoom, legibility calculation or hit-test exists. |
| 11.4 | Every numeric is source-tagged/measured and absent values never become zero. | IMPLEMENTED | `DisplayValue`, `BoundedCount`, source labels, invalid denominator checks and snapshots enforce/render this distinction (E16). |
| 11.5 | Missing dependencies render an explicit refusal/absence instead of a blank surface. | PARTIAL | Unknown/absence values render `—` with reasons (E16); missing graph/layout is not implemented as a separate fallback surface. |
| 11.6 | Accessibility and contrast are a merge gate. | ABSENT | No accessibility/contrast checker or merge gate exists. |
| 11.7 | Console displays active exception age/expiry and hard-fails expired exceptions. | ABSENT | Ratchet exceptions exist, but console exception display/expiry enforcement is not present. |

### 12 · KEYLESS-TOKENOMICS-AND-ROUTING

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 12.1 | Required model work uses operator-owned, keyless local CLIs with a keyless fallback and explicit unavailable-lane refusal. | PARTIAL | Claude/Codex use local CLI credentials and `freelane` reports network unavailability as exit 3 (E8, E22); network/keyless dependency posture is not proven for every required path. |
| 12.2 | Token reservations and usage are integer-valued, persisted, and unknown remains null rather than zero. | IMPLEMENTED | Meter reservation validation, persistence, null serialization and observations implement this (E15). |
| 12.3 | Fleet detects installed/eligible adapters through a deterministic staged route. | IMPLEMENTED | `route` applies ordered candidate filters and reports the stage/refusal/reason (E14). |
| 12.4 | Routing distributes work by role and refuses missing model identity rather than echoing the request. | IMPLEMENTED | Role/candidate routing and resolved-model comparison tests cover both availability directions (E7, E14, E8). |
| 12.5 | Compression automatically triggers at token limit while preserving every atomic acceptance predicate verbatim. | ABSENT | No compression command, trigger, compressed brief or predicate-preservation check exists. |
| 12.6 | Scheduling projects burn/exhaustion and reports a resumable next window before spawn. | PARTIAL | `meter plan` and reservation refusal exist (E15); no reset-window/schedule or pre-spawn burn-rate projection is wired into dispatch. |
| 12.7 | Cross-model cost comparisons carry tokenizer generation and characters alongside tokens. | PARTIAL | Adapter usage is integer and attestation has characters/tokenizer fields (E8, E19); normal runs leave token/tokenizer fields null and no comparison report exists. |

### 13 · OSS-STACK-AND-UPSTREAM-DELTA

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 13.1 | Every adopted dependency has verified metadata and a passing smoke command. | ABSENT | The blueprint table is prose; no checked-in adoption register, metadata verifier or smoke runner exists. |
| 13.2 | Fleet's upstream delta is machine-separated from inherited capability, including the structured-output boundary. | PARTIAL | Source distinguishes adapter/submission boundaries (E8, E19); no machine-readable upstream-vs-owned ledger or `PREDICATE_LOST` taxonomy exists. |
| 13.3 | The owned core is implemented: fd3 receipts, lifecycle, attestation replay, ratchet, canonical graph key. | IMPLEMENTED | fd3/receipts (E4/E8), lifecycle (E5), attestation (E1-E3), ratchet (E17) and graph symbol/index code (E12) are present. |
| 13.4 | The system runs a local canary trace through OpenTelemetry → Parquet → DuckDB before success. | ABSENT | No OpenTelemetry, Parquet, DuckDB trace writer or canary query is present. |
| 13.5 | Malformed structured output is classified as schema/version/ownership/predicate/traceability failure and never repaired silently. | PARTIAL | Submission schema/version/body validation refuses malformed packets (E8, E19); the five-class taxonomy and predicate-loss check are absent. |
| 13.6 | Rejected/adopted alternatives and their license/key status are executable facts rather than documentation-only claims. | ABSENT | No executable license/adoption/rejection gate exists. |

### 14 · CAPACITY-AND-FANOUT-MATH

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 14.1 | Fleet measures the queue-network station times and publishes machine/human service denominators. | ABSENT | The blueprint names `crew/eval/capacity.py`; that file and station telemetry are absent. |
| 14.2 | Runtime fan-out is derived as `W=min(W_cpu,W_mem,W_review)` and capped at four. | ABSENT | No CPU/RSS/reviewer-capacity sampler or W=4 scheduler exists. |
| 14.3 | Delegating human acceptance yields the measured 12.4× throughput decision only after pass^k evidence. | ABSENT | No delegated acceptance authority or pass^k evaluator exists. |
| 14.4 | OpenTelemetry station/gate spans retire the queueing assumptions. | ABSENT | No span instrumentation or queue/service histogram exists. |

### 15 · FAILURE-MODES-AND-DEGRADATION

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 15.1 | Every dependency failure resolves to a definitive non-blank state with a typed exit. | PARTIAL | CLI refusals, environment faults, status states and explicit absence rendering exist (E16, E20); the full dependency degradation ladder is not implemented. |
| 15.2 | Missing graphviz, broken chain, missing token count and unavailable model each have a safe visible fallback. | PARTIAL | Unknown token and unavailable model paths are visible (E8, E15, E16); no graphviz/SVG fallback or segmented-chain recovery exists. |
| 15.3 | A 02:00 runbook covers ranked failure modes with concrete recovery commands. | PARTIAL | Runbook files exist under `docs/runbook/`; they do not cover every blueprint-ranked failure/degradation row. |
| 15.4 | The ranked pre-mortem is itself backed by checked-in trigger detectors. | ABSENT | Ranking and residuals are blueprint prose; no detector maps every pre-mortem row to a failing command. |
| 15.5 | Supervisor death/orphan risk is closed by process groups, cleanup and single-instance reporting. | IMPLEMENTED | Process groups/deadlines/reaping are implemented (E8), and run/rollback cleanup is bounded (E9). |

### 16 · PHASES-TO-USABLE

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 16.1 | P0 is one real prompt-to-frozen-attested task, with no work lost. | IMPLEMENTED | The quickstart is a real git repo flow ending in artifact/ledger/attestation verification, and E1 captures the path. |
| 16.2 | P0 includes artifact freeze, human SOW, independent verification, rollback and evidence. | PARTIAL | Those slices exist (E1, E6, E9); P0 does not enforce the full blueprint attestation/graph/reachability contract. |
| 16.3 | P1 produces a verdict worth trusting through O2 holdout and complete nine-element attestation. | PARTIAL | O1/O2 registration/adjudication is implemented (E2-E3); full P1 statistical/attestation exit criteria are not. |
| 16.4 | P2 self-hosts fleet changes and uses that corpus as a known-ground-truth eval. | ABSENT | No self-hosted dispatch/eval campaign or corpus ingestion exists. |
| 16.5 | P3 learns from at least 60 closed outcome windows and publishes scorecard UNKNOWN rate. | ABSENT | Agent scorecards exist (E10), but no outcome-window counter, learning promotion or P3 gate exists. |
| 16.6 | P4 lets Opus act as user and judge after a measured autonomy threshold. | ABSENT | No P4 authority, Opus judge integration or pass^k gate exists. |
| 16.7 | Phase entry is a measured pass^k/false-accept number, not a maturity judgement. | PARTIAL | Ratchet thresholds and denominator-bearing quality marks exist (E17); no pass^k phase gate is implemented. |
| 16.8 | A seeded-bug audit periodically proves the gates are alive and revokes P4 on a false accept. | ABSENT | No seeded-bug scheduler or phase revocation mechanism exists. |

### 17 · CROSS-EXAMINATION

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 17.1 | A worker cannot write its own passing receipt. | IMPLEMENTED | Worker has no ledger path/state env and can only send body over fd3; parent appends actor/model/hash (E8, E4). |
| 17.2 | A fabricated attestation is caught by re-derivation and exits 8. | PARTIAL | `attest verify` checks artifact/receipt-backed fields (E1-E4); the full nine-element replay is not implemented. |
| 17.3 | Shared lesson-store writes are included in read/write-set collision refusal. | ABSENT | No learning store or read/write-set scheduler exists. |
| 17.4 | Child death mid-write cannot deliver a partial message because fd3 preserves message boundaries. | PARTIAL | Unix socketpair/fd3 and packet validation exist (E8); macOS uses a stream fallback, so the exact seqpacket guarantee is not universal. |
| 17.5 | A stale graph index refuses and prints the reindex command. | IMPLEMENTED | `check_freshness` compares tree digest/floor/count and prints the reindex command on mismatch (E12). |
| 17.6 | Missing graphviz still renders a stated adjacency representation. | ABSENT | No graphviz invocation, SVG, adjacency fallback or failure-state renderer exists. |
| 17.7 | Missing token count renders as unknown, never zero. | IMPLEMENTED | Meter serializes absent fields as null and console renders source-tagged absence (E15, E16). |
| 17.8 | A rate-limited model lane is refused before spawn with a resume time; mid-run stalls are detected. | PARTIAL | Route/reservation checks can refuse unavailable capacity (E14-E15); no reset-time response or output-growth stall detector exists. |
| 17.9 | A worker exit 0 with no new work is refused. | IMPLEMENTED | `run_with_evidence` refuses empty or identical diffs and records `NO_WORK_LANDED` (E1). |
| 17.10 | Retrieval failure cannot weaken deterministic enforcement because signatures, not memory, are the gate. | ABSENT | Neither the retrieval store nor deterministic promoted-signature gate exists. |

### 18 · HONEST-LIMITS

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 18.1 | Fleet explicitly claims repository/process evidence, not production, user or business outcome. | IMPLEMENTED | README states local single-user scope, operator root of trust, and repository-only/tamper-evident limits (E22). |
| 18.2 | The weakest links and TBM ledger are published with owners/instruments and no hidden green status. | PARTIAL | Limitations and open claims are documented (E22 and blueprint); no executable TBM ledger/report consumes the instruments. |
| 18.3 | TBM-1 through TBM-12 instruments are implemented or scheduled with measured counters. | ABSENT | The TBM table is documentation; paired, telemetry, audit and outcome instruments are absent. |
| 18.4 | Keyless/platform-agnostic means vendor-agnostic while naming the OS-bound limitation. | PARTIAL | Claude/Codex/freelane adapter paths exist and README explicitly says macOS-only (E8, E22); the blueprint's portable abstraction is not implemented. |
| 18.5 | An independent audit function/merge authority exists outside the system's own self-audit. | ABSENT | No external assessor or protected merge authority is present. |
| 18.6 | Unknown/absent values and denominators remain visible in status and measurement. | IMPLEMENTED | Status prints `0 of 0` for empty data and display/meter preserve unknowns (E15-E16). |

### 19 · INDUSTRY-ALIGNMENT

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 19.1 | Fleet converges on industry practices: SOW/plan-first, independent review, provenance, gates and explicit limits. | PARTIAL | Plan/SOW, independent verifier, receipts and limits are real (E1, E5-E6, E22); the cited external playbooks are not operational inputs. |
| 19.2 | Gate wait-time and acceptance station are instrumented through OpenTelemetry. | ABSENT | No OpenTelemetry dependency, span emitter or station acceptance timer exists. |
| 19.3 | Fleet is platform/model-vendor agnostic through a stable adapter contract. | PARTIAL | Claude/Codex protocols and capability report exist (E8); only those adapters plus a bespoke fallback are implemented and the blueprint's complete portability contract is absent. |
| 19.4 | METR/DORA-style parity, instability and delivery-equivalence claims are measured in fleet. | ABSENT | No RCT/parity/paired delivery evaluation implementation exists. |
| 19.5 | A written review policy is wired into verifier briefs. | ABSENT | No `REVIEW.md` or verifier policy loader is present. |

### 20 · BUILD-VS-ADOPT-RECKONING

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 20.1 | Delivery attestation uses an in-toto Statement subject identified by BLAKE3 digest. | IMPLEMENTED | Contract requires the in-toto type/subject/digest and run writes that shape (E1, E19). |
| 20.2 | `in-toto/witness`/DSSE is adopted for attestation generation rather than hand-written generation. | ABSENT | The current run constructs JSON directly; no witness/DSSE dependency or invocation exists. |
| 20.3 | Gate bodies are policy-as-data via Rego/regorus, with a Rust boundary for refusal. | PARTIAL | Rego policy files and `policy/run.sh` exist, while Rust ratchet/refusal is real; no regorus integration makes policy the runtime gate. |
| 20.4 | The system's SLSA claim is machine-attested at the stated worker/operator levels. | PARTIAL | Worker isolation/provenance evidence exists (E8, E19) and README labels the trust levels (E22); no SLSA provenance verifier or external authority exists. |
| 20.5 | Gate registries, lease capabilities and exceptions are policy data rather than scattered code. | ABSENT | Roles/skills and ratchet are code-plus-TOML (E7, E18); no unified gate/exception policy registry exists. |
| 20.6 | A lease is the capability set, and MCP exposes only lease-derived tools/paths. | PARTIAL | Manifest, tool scopes and path checks are derived from a lease (E11); no durable lease authority or complete capability registry exists. |
| 20.7 | Agent and skill registries resolve declared skills with denominators and reject unresolved entries. | IMPLEMENTED | `agents.toml`/`skills.toml`, resolution report, capability checks and zero-input failure implement the claim (E10, E18). |
| 20.8 | Pi/Hermes or another unified harness is adopted so one adapter covers providers. | ABSENT | Only Claude/Codex adapters and freelane are in the repository; no Pi/Hermes integration exists. |
| 20.9 | Every OSS dependency row has a successful, recorded smoke invocation. | ABSENT | No executable dependency smoke ledger exists. |

### 21 · THE-FOUR-OBJECTS

| id | falsifiable claim | result | evidence / deficit |
|---|---|---|---|
| 21.1 | `Agent` is persistent with stable id, role, charter, capabilities, skills and scorecard. | IMPLEMENTED | Rust `Agent`/registry loads those fields from `agents.toml`; scorecard path and persistence are implemented (E10). |
| 21.2 | Each agent has a nested assignment lifecycle from Idle through submission/adjudication/credit/fault/amendment. | IMPLEMENTED | Typestate `Assignment` exposes the full consuming lifecycle and tests the outcome path (E10). |
| 21.3 | Scorecards record credited, faulted, outcome-held/regressed and unknown with denominators. | IMPLEMENTED | `ScorecardOutcome`, outcome recording and persisted scorecard counters exist (E10). |
| 21.4 | Submission freezes an immutable BLAKE3 artifact, releases the lease, and verifier receives only artifact id. | PARTIAL | Artifact hash, create-new write, fsync and mode 0444 are implemented (E9); no explicit lease-release boundary or artifact-id-only verifier protocol exists. |
| 21.5 | Three independent oracles exist: lead O1, verifier O2, and generated property/metamorphic O3. | PARTIAL | O1/O2 registration, author separation, execution and hashes exist (E2-E3); no generated O3 property oracle exists. |
| 21.6 | The O1/O2 2x2 discriminator diagnoses lead/builder fault and credits accordingly. | IMPLEMENTED | `adjudication_table` implements all four quadrants and the receipt records fault/credit (E3). |
| 21.7 | Oracle independence is an eighth non-droppable attestation element. | IMPLEMENTED | Attestation contract names `oracle_independence`; adjudication writes authors, hashes, distinctness and quadrant (E2-E3, E19). |
| 21.8 | Outcome windows derive repository signals and publish HELD/REGRESSED/UNKNOWN, including UNKNOWN rate. | PARTIAL | Agent outcome enum/counters include unknown and amended outcomes (E10); no subsequent-change window, re-churn/escaped-defect signals or dashboard rate exists. |
| 21.9 | The system states the boundary: repository outcome is not user/business/production outcome. | IMPLEMENTED | README and the blueprint limitation explicitly make that distinction (E22). |
| 21.10 | Reputation is acted on in P3 through role reassignment after outcome evidence. | PARTIAL | Scorecards and role/agent registries exist (E7, E10); no P3 role-reassignment action or outcome-window trigger is implemented. |

## Audit boundary

This is a coherence audit, not a claim that the blueprint itself is correct. In particular, the
current tree contains a real P0 path and a real O1/O2 discriminator, but it does not earn the
blueprint's later learning, statistical-equivalence, service-grade, graph-UI, adoption, or
multi-agent-operating claims. The largest measured gap is not a failing unit test; it is the absent
instrumentation and lifecycle wiring between those implemented slices.

## Findings the blueprint did not anticipate

**Scope note.** "The blueprint" here means exactly what the rest of this document audits: the 22
numbered files in `../blueprints/Fleet-L8-Deep-Dive/` (`00`–`21`). `docs/DELTA.md` also cites a
separate, informal numbered list of the owner's own feature requests (`req 3`, `req 5`, `req 12`,
`req 15`, …) that lives outside that 22-file set — those are owner asks, not blueprint sections, so
a finding tied only to a `req N` is judged against the 22 files, not against that list.

Walked `docs/DELTA.md` end to end: findings run `D1`–`D55`, then `D57`–`D60` (`D56` does not exist —
the numbering skips it and no content was ever filed under it). **59 findings total.** For each, the
question was: did the 22-file blueprint make a concrete, falsifiable claim that predicts this
specific failure mode, or did only building/running/measuring the real system reveal it?

Four were genuinely anticipated — the blueprint named the exact mechanism in advance, and building
confirmed it (`BLUEPRINT-RIGHT` in `DELTA.md`'s own verdict column, or an explicit prior claim in
this document):

| D# | what the blueprint got right |
|---|---|
| D4 | `02` §2 requires the log-size-floor + non-empty-diff pair (`S12`); it fired exactly as specified on the first two dead runs. |
| D7 | `A1` — corpus-flagged as "the most-repeated failure in the record" — predicted a worker would hand-roll a maintained primitive; it did, byte-for-byte. |
| D14 | Corpus principle `C15` ("a new gate's first run is mostly false positives") predicted a detector would match its own source; `H1` did, on the first run. |
| D43 | `10.7` (rollback tested in both applied/reverted states) and `11.1` (operator console answers what is running/blocked/cost) already named these capabilities; `D43` is those claims finally getting a CLI surface, not a new discovery. |

**55 of 59 were not anticipated** — the real output the owner asked B2 to surface. Each row below
gives the `D<n>`, one line on what actually happened, and one line on which blueprint section should
have caught it, or "no section addressed this class" when the finding is genuinely novel — an
operational, tooling, or emergent-integration failure with no blueprint claim in its neighborhood at
all.

| D# | what happened | blueprint section that should have caught it |
|---|---|---|
| D1 | `witness`/`conftest` were claimed adopted; neither binary was ever installed or run. | `20-BUILD-VS-ADOPT-RECKONING` — adoption needs a passing smoke command, not a claim. |
| D2 | P0's acceptance path assumed a live model adapter, making the suite nondeterministic and quota-burning. | `16-PHASES-TO-USABLE` — P0 should have specified a deterministic first path. |
| D3 | Two dispatches produced zero files at exit 0; session/MCP scaffolding had consumed the whole context budget. | `02-ARCHITECTURE-TWO-LANGUAGE-KERNEL` — worker-environment rules say nothing about context budget. |
| D5 | The D3 failure class (env fault vs. agent failure) went undetected; no precondition probe existed. | `05-GATE-KERNEL-AND-UNFAKEABILITY` — no mechanism turns the exit-3 rule into a dispatch-time gate. |
| D6 | My own acceptance suite silently skipped assertion B4 when `b3sum` was absent, masking a hand-rolled BLAKE3. | `05` §3.1 — the checked==0 rule existed for gates, never extended to the suite measuring them. |
| D8 | OmniRoute (53,737★) reports a resolved-model readback and disableable compression, properties the blanket pooled-gateway rejection didn't distinguish. | `12-KEYLESS-TOKENOMICS-AND-ROUTING` §5 — rejection rule too broad. |
| D9 | A shared brief told three agents to all write objections into one file; one overwrote it, destroying D1–D5. | `03-REPOSITORY-STRUCTURE` §2.3 — storage-class isolation existed for the ledger, never applied to the delta log itself. |
| D10 | `turbovec` (16,267★, Rust+Python) is a better-fit vector index than the blueprint's chosen `usearch`. | `13-OSS-STACK-AND-UPSTREAM-DELTA` §2.1 — a specific dependency pick, now stale. |
| D11 | Three different partition mistakes (shared file, unbounded reads, single-file collapse) all violated the safe-parallelism predicate. | `07-ISOLATION-AND-SUPERVISION` §4 — states the predicate, never says how to partition. |
| D12 | 8/9 gates green while every user-facing command (`--help`, bare invocation) exited 7 with zero output. | `10-PROVING-SERVICE-GRADE` — missing REACHABILITY delivery element (this document's own §10 row now says so). |
| D13 | Three false readings came from the test harness itself (pipe `$?`, command-substitution `$?`, zsh word-splitting), not the product. | `05` §4 — the proxy rule doesn't cover the instrument itself being a proxy. |
| D15 | Six P1 agents ran 12 minutes with zero writes; the repo had outgrown a whole-worktree read scope. | No section addressed this class — nothing distinguishes "dispatching" from "managing". |
| D16 | `find -newermt` falsely reported zero agent writes while three agents were actively patching. | No section addressed this class — instrument reliability, not a design topic. |
| D17 | A keyless free-token gateway was declined on ToS grounds the operator had already twice overridden. | No section addressed this class — a judgment-call error, not a design gap. |
| D18 | A third, differently-provenanced free model reviewed the ledger and named a real failure class (though wrong about this instance). | `10-PROVING-SERVICE-GRADE` — verifier independence names a different agent id, not provenance diversity. |
| D19 | `policy/run.sh` ran conftest with no `--namespace`, matched zero rules, and passed every bad fixture. | `20` §4 / `05` §3.4 — the both-directions rule existed; the policy layer's own compliance with it was never checked. |
| D20 | `verify.sh` had zero `pytest` invocations, hiding a red test that asserted the exact model-forgery the fd-3 design forbids. | `05-GATE-KERNEL-AND-UNFAKEABILITY` — gate-is-sole-arbiter rule; `pytest` was simply never wired into it. |
| D21 | `fleet run` with no `FLEET_STATE` exited 3 and printed nothing. | `10` element 9 — names module reachability, not message actionability. |
| D22 | `fleet doctor` refused to run in a broken environment, the one case it exists for. | `02` capability 7 — names a usable surface generically, not self-diagnosis under failure. |
| D23 | git exports `GIT_INDEX_FILE` to hooks, redirecting the acceptance suite's throwaway-repo writes. | `16` P2 — mentions self-hosting, not "the gate running inside its own hook context". |
| D24 | `-c mcp_servers={}` never actually overrode codex's own `config.toml`. | No section addressed this class — explicitly "n/a — dispatch hygiene" in `DELTA.md` itself. |
| D25 | Ratchet's real mutation score was 26.7%, calibrated against a floor of `0/1` that no score could fail. | `05` incentive-soundness (`05.2`, ABSENT per this document's own audit) — the general principle exists, not the calibration rule. |
| D26 | Handing each lane the exact list of surviving mutants (not "improve coverage") raised the score 26.7%→70.7%. | No section addressed this class — a technique discovery, not a design topic. |
| D27 | A merge silently reverted the `FLEET_MUTANTS` opt-in; the gate stayed green, just 16× slower. | `05` — a new failure shape (green-but-unusable) the gate-kernel chapter never named. |
| D28 | A worktree lane rewrote a lead-authored detector to assert the opposite, and it was committed under the lead's own message. | `05` — adversarial tampering with the gate itself, not covered. |
| D29 | `run --task ""` wrote no receipt because a lifecycle wrapper pre-empted the inner guard. | `04-EVIDENCE-LEDGER` — "every refusal is evidenced" is the general rule; this specific integration bug wasn't foreseen. |
| D30 | The corpus stage hung on a 4.9GB in-tree `CARGO_TARGET_DIR`; path exclusion filtered after the walk, not before. | `05` — "a gate must be runnable" is general; the traversal-order bug is not. |
| D31 | A worktree merge with zero staged commits reported success, because `git merge` on an empty branch exits 0. | `16` P2 — mentions self-hosting, not merge-of-nothing detection. |
| D32 | The pre-registered parity experiment ran cleanly and measured nothing — a ceiling effect plus a prompt-blind stub. | `10` S4b — the experiment is specified; its rubric/agent-choice design flaw was visible only by running it. |
| D33 | S4b run live returned NOT-EQUIVALENT, but 99/128 scored zero from free-lane rate-limiting, not prompter effect. | `10` S4b — same: design flaw surfaced only empirically. |
| D34 | A shared `CARGO_TARGET_DIR` baked a deleted worktree's absolute path into the shipped binary. | `02` — portability capability exists; the shared-build-cache hazard is not named. |
| D35 | The documented `fleet run` allow-list refused both `claude` and `codex`; the tool used neither model. | No section addressed this class — a plain implementation gap. |
| D36 | `fleet swarm dispatch` credited five agents with work that left no ledger row and no verifying attestation. | No section addressed this class — the swarm path bypassing the evidence kernel entirely was not foreseen. |
| D37 | Re-running S4b dropped the zero rate 77%→40%, still uninterpretable; the pilot precondition passed on an unrepresentative early sample. | `10` S4b — design flaw, empirical. |
| D38 | 25 detectors each carried a private copy of the same walk+prune scanner function. | No section addressed this class — a code-duplication finding. |
| D39 | The planning gate needed a global test bypass, and a badly-scoped bypass silently neutered `S8`'s assertion. | No section addressed this class — bypass-scoping hazard is a testing-methodology finding. |
| D40 | Three unrelated commands independently reproduced "an honest exit code with no next step". | No section addressed this class — an emergent UX pattern visible only after three instances existed. |
| D41 | `fleet plan` named a command guaranteed to be refused by a gate added later. | No section addressed this class — a cross-feature integration gap invisible to either feature's own tests. |
| D42 | A SOW refusal named the deficiency but not the section format needed to fix it. | No section addressed this class — message-quality finding. |
| D44 | `opa` and `rekor-cli` were listed ADOPTED while nothing in the tree called either. | `20-BUILD-VS-ADOPT-RECKONING` — adoption claims not checked against an actual caller. |
| D45 | The planner started refusing to show a plan at all whenever the router emptied, conflating planning with running. | `12` — the routing stages themselves were already specified (ROUTING.md); the plan/run conflation is a new integration bug no section names. |
| D46 | `bin/freelane.sh` discarded real token counts the API was already returning on every call. | No section addressed this class — an integration bug in a stub adapter. |
| D47 | An attested change with 97 added lines was accepted while 0 were executable — commented-out prose, not code. | No section addressed this class — a non-empty diff is a blueprint-named proxy; a diff that *implements something* is not. |
| D48 | Failed parity runs recorded `score=0`, indistinguishable from a genuine low score, inflating three earlier "zero rate" findings. | No section addressed this class — a scoring-harness default-value bug. |
| D49 | The actionable-refusal rule (`Q1`) never covered `run`, the command every user touches first. | No section addressed this class — enumeration-completeness gap in an already-built gate. |
| D50 | The router's own candidate table omitted `freelane`, the one lane that was actually up and funded. | `12` — staged routing is specified; the keyless fallback's place in the order was an implementation omission. |
| D51 | A structured SOW's text overflowed the OS filename limit — two independently-correct features combined to make every realistic task fail. | No section addressed this class — an integration collision between two individually-tested features. |
| D52 | The `Change`-verb table missed common phrasing, and widening it nearly broke the "refuse an unrecognized prompt" guard. | No section addressed this class — natural-language coverage tradeoff, discovered empirically. |
| D53 | A nonexistent `--repo` path and a non-git directory both produced the SOW-missing error, unfixable by writing a SOW. | No section addressed this class — error-ordering finding. |
| D54 | Three stacked concurrency races (TOCTOU on seeding, lost scorecard updates, partial reads of a partially-seeded store) surfaced under load. | No section addressed this class — `L8` names "concurrent" generically as an unenumerated case, not these specific races. |
| D55 | The parity harness predated the SOW gate and called `fleet run` without the bypass, so every observation silently refused. | No section addressed this class — cross-feature integration gap between two independently built pieces. |
| D57 | The free keyless lane's success rate is variable, not just rate-limited; pacing alone does not fix it. | `10` S4b — an empirical limitation of the only keyless lane, found only by repeated live runs. |
| D58 | The README's own 60-second quickstart failed because the SOW gate landed after it was written; nothing tested the README. | No section addressed this class — documentation-drift, visible only by running the document itself. |
| D59 | `docs/SECURITY.md` described two commands (`fleet attest self`, `fleet sbom`) that were never built. | No section addressed this class — documentation-drift. |
| D60 | Five reachable commands (`plan`, `meter`, `lifecycle`, `graph`, `completions`) were absent from `--help`. | `10`/`11-OPERATOR-CONSOLE-UX` — the reachability element covers dead code, not the inverse: undocumented live code. |

## Published count

**4 of 59 (6.8%) anticipated · 55 of 59 (93.2%) not anticipated.**

The lopsidedness is not a defect in the blueprint's writing — files `00`–`21` are a design, and a
design cannot enumerate the emergent, operational, and instrument failures that only appear once the
thing is actually built and run. What the split does show, arithmetically: this repository's
existing thesis (`fleet/PRINCIPLES.md` #2, "a proxy is not the property") is not a slogan about code
quality here, it is a description of where 93% of the real findings came from — running the system,
not reading about it. The four anticipated findings are the exception that proves the rule: the
blueprint got them right precisely because it made a specific, falsifiable, testable claim in
advance rather than a general aspiration.
