# Fleet — Migration & Build Plan (historical record — migration complete)

> **This migration is DONE.** This file is kept as the historical resume-anchor record of how the
> 16-crate migration happened; it is no longer a live anchor to resume work from. The workspace is
> green — 380 tests passing, 0 failing, `crates/` + `src/` are the only code that exists (`fleet/keel`
> and the other predecessor directories named throughout this file have since been deleted). §7
> below (the teach-back log) is the part still worth reading: dated lessons from real mistakes
> caught during the build, kept transferable to future work in this tree.

Last updated: 2026-09-08 · Owner: rachit · Lead/review: Opus · Build: Sonnet + opencode

> **PROJECT STATUS (as of this rewrite): migration complete.** All 16 crates + `src/` (fleet-cli)
> build and pass; `cargo test --workspace` is 380 passed / 0 failed. Two bugs found after the
> original "16/16 DONE" milestone below have since been fixed (see §7's newest entries). The
> `probe_no_ambient` keyless-proof item mentioned throughout this file as "needs human sign-off"
> is still exactly that — a human-judgment call, not automatable — see
> `crates/fleet-worker/src/probe.rs`'s own doc comment, which still says so. The aux CLI subcommands
> (`console/freeze/contract/pr/attest/adjudicate/skills`) and MCP mounting remain intentionally
> unimplemented (`DispatchError::NotYetImplemented`) — see `REQUIREMENTS.md`'s tally for the current,
> re-derived status of every original ask, not this file (this file is a build log, not a live
> status source after the migration's completion).
>
> The rest of this document (§§1–6, 8) is left as-written — it is the accurate record of the
> decisions, phases, and per-crate build state AT THE TIME the migration ran, including items that
> have since changed (e.g. `fleet/keel` no longer exists, though this file still refers to it as the
> extraction source throughout — that is correct history, not a live claim).

**Every agent follows `PLAYBOOK.md`** (AI-native SDLC: spec-before-code, verify-own-work
with a published denominator, no self-grading, mistakes→teach-back). The blueprint template is
`_TEMPLATE.md`; the worked exemplar is `fleet-router/BLUEPRINT.md`.

**Global hard constraint — ≤ 80 lines per source file.** No new code file (`.rs`, `.py`, any source)
may exceed 80 lines (comments + blanks included). Decompose into small single-responsibility modules;
`lib.rs` is a thin declaration/re-export hub. Docs (blueprints, this plan, PLAYBOOK) are exempt.
Enforced per crate in §6.

---

## 1. The decision (settled, evidence-backed)

**ITERATE, do not rebuild.** Extract fleet's working, tested modules into clean crate boundaries;
build fresh ONLY the crates fleet genuinely lacks. Rationale (from 3 parallel Sonnet audits,
2026-09-08): fleet builds today (`cargo check` exit 0, 54 tests green in plain `cargo test`), and the
hard/risky parts are already coded AND adversarially tested — fd-3 protocol (`libc::dup2`),
hash-chained tamper-verified ledger, worktree isolation, keyless CLI-driving, type-state lifecycle
(compile-fail tests), Wilson-score mutation-gated ratchet. The target design (formerly codenamed
"Foreman") is the SAME doctrine redrawn with cleaner `fleet-*` crate boundaries — a refactor target,
not a new system. All crates use the `fleet-` prefix; the project is Fleet.

Known pre-existing red (tracked in `fleet/STATUS.md`, must be cleared before/at extraction, NOT
inherited): fmt drift · 5 `manual_inspect` clippy lints in `main.rs::pr_emit_probe_command` ·
`f08_pr_emit.rs` git-apply/rev-parse race flake · 2 historical gitleaks from the `8f86440` import
(`fleet/tmp/pdfs/build_ai_native_sdlc_atlas.py:310`, `orb/ios/Podfile.lock:2926`) · 1 semgrep
(`registry-reference/.../memory/memory_store.py:184`).

## 2. Target layout

```
Light/fleet/          # <- the new workspace lives INSIDE fleet/ (moved here 2026-09-08 per owner)
  Cargo.toml          # virtual workspace: members = crates/fleet-* + src; excludes keel/registry-reference/tests/crew
  (this dir)/       # one folder per crate; each holds BLUEPRINT.md (hand-codeable / 4B-completable)
    _TEMPLATE.md      # the house blueprint template (fill per crate; lead reviews)
    _ADDING-A-CRATE.md # how to blueprint + implement a new crate, post-migration (trust the blueprint, no concurrent cargo mutants)
    MIGRATION-PLAN.md # THIS file
    fleet.html        # single-file HTML artifact (part 5)
  crates/             # independent packages, one per crate (Rust; Python fleet-crew lives here too)
  src/                # the running/composition layer (bin: fleet-cli) — imports crates, wires pipeline
```
All build/verify commands run from `Light/fleet/`. The existing `fleet/keel` workspace is untouched
and excluded.

## 3. Crate roster + fleet source map + build branch

Branch legend: **extract** (lift working code ~as-is) · **refactor** (lift + reshape out of the
`main.rs` god-file) · **partial** (some exists, finish the rest) · **build-new** (absent in fleet).

| # | crate | branch | reuse from fleet (evidence) |
|---|---|---|---|
| 1 | fleet-types | refactor | `Role` (+`bandwidth`/`allocation_fitness`) from `roles.rs`; ID newtypes (`TaskId`/`NodeId`/`LaneId`); `Tokens` (integer minor-units); `ExitCode` enum; `LifecycleState` **vocabulary only**; `Receipt`/`Attestation` wire-types mirroring `contracts/*.v1.json`. **NOT** the safety gate (→ fleet-router), **NOT** the `Task<S>` machine (→ fleet-lifecycle). |
| 2 | fleet-store | refactor+unify | `main.rs::append_receipt` (blake3 chain, ~L4355) · `graph.rs` (rusqlite) · `memory_store.py` (sqlite-vec). Add redb; unify. |
| 3 | fleet-events | build-new | ingress adapters (github/gmail/fs/cli) — ABSENT in fleet |
| 4 | fleet-router | extract (**file split, not move**) | `keel/fleet/src/route.rs` — only `decide()`+`role_allows`/`stage` (~230/687 lines) are pure and belong here. The other ~60% is IO that re-homes elsewhere: `adapter_contract`/`installed_non_interactive` (subprocess) → `fleet-worker`; `runtime()` cooldowns/meter-file reads → `fleet-govern`; `command`/`print_human`/`append_receipt` calls → `src/` + `fleet-store`. |
| 5 | fleet-scan | build-new | ambiguity probes — fleet's `scan.sh` is a SECURITY scanner (name clash), different purpose |
| 6 | fleet-plan | refactor | `intake.sh` · `lld.rs` · `lld_ready.rs` · `review.sh` — unify + add "teach" |
| 7 | fleet-context | partial+build | `graph.rs` (tree-sitter rust/py/bash) present; SCIP + tantivy + fastembed ABSENT |
| 8 | fleet-verify | extract | `fleet/verify.sh` — the re-run-every-check gate wall |
| 9 | fleet-merge | extract | `bin/merge-lane.sh` · `keel/fleet/src/worktree.rs` |
| 10 | fleet-memory | partial+build | `memory.sh` · `memory_store.py` (hybrid BM25+vec) present; episodic/semantic/procedural split + Thompson bandit ABSENT |
| 11 | fleet-govern | refactor+unify | `budget.sh` (atomic admit) · `meter.rs` (token accounting) · `route.rs` failover; add tiktoken-rs real counts |
| 12 | fleet-stream | partial+build | `telemetry_otel.py` (OTel) · `console.rs` (TUI) present; unified `Sink` trait + Orb/webhook/file sinks ABSENT |
| 13 | fleet-worker | refactor | `worktree.rs` · `skills.rs` · `mcp.rs` · `agent.rs` (incl. the file-locked-RMW `Scorecard`) · crew adapters · fd-3 in `main.rs` (~L3088 `dup2`) |
| 14 | fleet-crew (py) | package | `fleet/crew/` — keyless CLI adapters; package as a crate-dir Python pkg |
| 15 | fleet-lifecycle | refactor | the type-state `Task<S>` machine from `lifecycle.rs` (~1478 lines): 16 marker structs, sealed `State` trait, transition methods returning a `TransitionReceipt` **pure — no persistence** — + the compile-fail test suite. Depends only on `fleet-types`. |
| — | src/ (fleet-cli) | refactor | `main.rs` dispatcher + `swarm.rs` → composition root, tokio+rayon pipeline |

Dependency DAG (one-directional): `types → {lifecycle, store} → {events, router, scan, plan, context,
verify, merge, memory, govern, stream, worker} → src/`. `fleet-lifecycle` and `fleet-store` are the
foundational layer above `types` (both depend only on `types`; `fleet-lifecycle` is pure, no IO).
Sibling crates connect only through named minimal edges (see each blueprint's "edges" section).
`fleet-crew` is a runtime dependency of `fleet-worker` (subprocess), not a compile edge.

**Canonical sibling-dependency rule (dependency inversion — follow this everywhere).** A crate that
needs *advisory/data* from a sibling does NOT take a Cargo path-dep on it. It defines a minimal **port
trait it owns** (e.g. `CodebasePort`, `MemoryPort`, `ConcurrentRunner`) and the `src/` composition root
supplies the concrete adapter that bridges to the real sibling. This is the approved `fleet-router`
(`RuntimeState`) / `fleet-scan` (ports) pattern — it keeps the DAG acyclic and each crate unit-testable
with a fake port. Only a hard *behavioral* call (like worker invoking merge's `create`) is a real edge.

Named sibling→sibling COMPILE edges (the only hard ones allowed; add here when adjudicated):
- `fleet-worker → fleet-merge` — worker calls `create`/`merge_lane`/`remove` around a lane (worktree
  lifecycle lives in fleet-merge; `lane_cap()` scheduling lives in fleet-worker).
- Everything else advisory (scan→context/memory, plan→scan, memory→context, …) goes through **ports**,
  wired in `src/` — not compile edges.

## 4. Phases

- **P0 — foundation** (Opus, now): scaffold ✓ · this plan ✓ · blueprint `_TEMPLATE.md` (Sonnet) · house checklist.
- **P1 — blueprints** (Sonnet writes, Opus reviews): one `BLUEPRINT.md` per crate, detailed enough
  to hand-code or have a 4B model complete. Independent → written in parallel.
- **P2 — debt clear** (Sonnet/opencode): clear the §1 known-red in fleet so extraction starts green.
- **P3 — migration/build** (Sonnet + opencode, parallel per crate): implement each crate per its
  blueprint WITH unit + integration + mutation tests. Extract crates first (fast wins), build-new after.
- **P4 — Opus review** (per crate): review the diff at the contract level, **distrusting the tests** —
  re-derive behavior independently, reproduce a mutation, drive it end-to-end. Only Opus flips a crate to DONE.
- **P5 — integrate**: wire crates in `src/`; the pipeline runs end-to-end on a real task.
- **P6 — clean**: delete irrelevant files/folders, dead fleet scaffolding, `mutants.out.old`, etc.
- **teach**: every caught iteration/mistake → a lesson written back so fleet doesn't repeat it (§7).

## 5. Per-crate status (source of truth — update on every change)

State: `todo · blueprint-wip · blueprint-done · build-wip · build-done · opus-review · DONE · blocked`

Build order for P3 (dependency-respecting): **wave 0** types → **wave 1** lifecycle, store →
**wave 2** router, events, scan, plan, context, verify, merge, memory, govern, stream, worker, crew
(parallel; ports mean they don't hard-block each other) → **wave 3** src/ (cli).

| crate | state | blueprint | tests (u/i/m) | opus-verdict | notes |
|---|---|---|---|---|---|
| fleet-types | **DONE** | ✓ | 28/28 · mut✓ | Opus✓ | built: 14 src + 8 test files, all ≤80; clippy clean; 4 reconciliations in; wrapping-mutant killed by hand (Opus reproduced) |
| fleet-lifecycle | **DONE** | ✓ | 21/21 · 6 goldens · mut✓ | Opus✓ | type-state safe (trybuild compile-fail); resume-mapping mutant killed by hand (Opus reproduced) |
| fleet-store | **DONE**\* | ✓ | 18/18 · mut✓ | Opus✓ | ledger tamper-check mutant killed by hand. \*GAP: MemoryStore vector path skip-gates w/o a `vec0` ext installed — unverified in this env (blueprint-sanctioned); re-run with FLEET_STORE_TEST_VEC0 set before prod. Opus fixed 3 test files >80 via a shared tests/support/mod.rs |
| fleet-events | **DONE** | ✓ | 15/15 · ws✓ | Opus✓(ws) | pollers not listeners; guard() payload-blind (Opus-confirmed); +blake3/native-tls deps (needed) |
| fleet-router | **DONE** | ✓ | 11/11 · mut✓ | Opus✓ | pure decision core; IO-free (grep-verified); role→tier mutant killed by hand |
| fleet-scan | **DONE** | ✓ | 22/22 · ws✓ | verifier✓ | ambiguity prober; verifier confirmed 22 pass; ports for context/memory |
| fleet-plan | **DONE**\* | ✓ | 42/42 · ws✓ | build✓(ws) | lld/lld_ready lifted; \*OPEN: `LifecycleState`↔`review.sh` state mapping is a best-fit guess — confirm at composition; ERE patterns approximated not byte-diffed |
| fleet-context | **DONE**\* | ✓ | ws✓ | build✓(ws) | v1 = tantivy BM25 + tree-sitter (Opus-confirmed no ort/fastembed, VectorIndex port stubbed); RRF hand-rolled. \*embeddings deferred to a 2nd pass |
| fleet-verify | **DONE** | ✓ | 27/27 · mut✓ | Opus✓ | ports keep it IO-free; zero-total-guard mutant killed by hand (Opus reproduced) |
| fleet-merge | **DONE** | ✓ | 16/16 · mut✓ | Opus✓ | worktree lifecycle + merge-back + D31 invariants; head-moved-guard mutant killed by hand |
| fleet-memory | **DONE**\* | ✓ | 17/17 · ws✓ | build✓(ws) | hand-rolled Thompson (no rand); ports. \*deviations: `write` Merge values are nominal (caller applies real max/increment); `Box::leak` for static gate category |
| fleet-govern | **DONE** | ✓ | 15/15 · mut✓ | Opus✓ | real fs4 lock (verifier-confirmed). Verifier caught an under-tested `settle` id check (stale-id corruption path); Opus added + mutation-verified the pinning test |
| fleet-stream | **DONE** | ✓ | 14/14 · ws✓ | verifier(mut)✓ | egress Sink + 6 sinks; verifier ran+restored mutations; +tokio-stream dep (SSE glue) |
| fleet-worker | **DONE**\* | ✓ | 19/19 · env_clear-mut✓ | Opus✓(ws) | env_clear-deletion mutant CAUGHT (test hardened to sentinel-var absence, not HOME-value proxy); dropped cedar/rmcp/tokio (unused). \*M1 `probe_no_ambient` vs a real CLI still needs HUMAN sign-off before the keyless claim is proven |
| fleet-crew | **DONE** | ✓ | 55/55 · ruff✓ · mut✓ | Opus✓ | python pkg (40 files ≤80); gemini dropped; 2 latent bugs fixed; leaf-predicate mutant killed by hand |
| src/ (cli) | **DONE**\* | ✓ | 14/14 · ws✓ | Opus✓ | composition root wires all 14 crates; 8-stage pipeline w/ crash-resume step-log; ConcurrencyCap floored+tested. \*DEFERRED: Restate→on-disk step-log shim (same resume property); aux subcommands (Pr/Attest/Freeze/Contract/Console/Skills/Mcp) parse but return typed NotYetImplemented — wire in a follow-up |

## 6. How to verify a crate (never trust the checkbox)

Per crate, from its `crates/<name>/`:
1. `cargo test -p <name>` — unit + integration green, **none skipped**; publish the denominator.
2. `cargo clippy -p <name> --all-targets -- -D warnings` — clean.
3. mutation: `cargo mutants -p <name>` (or the scoped gate) — record kill rate vs a committed floor.
4. file-size gate: `find crates/<name> \( -name '*.rs' -o -name '*.py' \) -exec wc -l {} + | awk '$1>80'`
   must print nothing — any source file over 80 lines fails the crate.
5. **Opus manual review** — re-derive the contract, reproduce ONE mutation by hand, drive one real
   behavior end-to-end. A proxy is not the property (exit 0 after a pipe ≠ pass).
Only then flip state → DONE here.

## 7. Teach-back log (mistakes → lessons, so fleet stops repeating)

| date | crate | mistake caught | lesson / gate added |
|---|---|---|---|
| 2026-09-08 | fleet-router | plan called route.rs "pure" — only ~230/687 lines are; rest is IO | blueprint records the split; agents must cite real line ranges (§5 reuse map), not assume a whole file is liftable |
| 2026-09-08 | fleet-types | `Role` + safety-gate live in `roles.rs`, uncited in the plan | added roles.rs to row 1 evidence; agents grep for the type's real home before assuming a file |
| 2026-09-08 | fleet-lifecycle | `lifecycle.rs`'s type-state `Task<S>` machine had no owner in the 14-crate roster (orphan-rule: its inherent impls can't be split across crates) | added a 15th crate `fleet-lifecycle` (pure, depends only on fleet-types); `fleet-types` keeps only `LifecycleState` vocabulary |
| 2026-09-08 | fleet-merge/worker | `worktree.rs` double-cited (rows 9+13) with no split | worktree lifecycle→merge; `lane_cap()`→worker; named edge worker→merge (§DAG) |
| 2026-09-08 | fleet-scan | siblings (context/memory) still `todo`, can't path-dep them | **canonical rule**: advisory sibling deps use caller-owned **port traits** wired in `src/`, not compile edges (§DAG) |
| 2026-09-08 | fleet-crew | design assumed claude/codex/**gemini**; no gemini CLI exists anywhere in fleet | **gemini dropped** to a non-goal; `CliAdapter` trait makes adding it trivial once a real CLI contract exists |
| 2026-09-08 | fleet-govern | plan called admission "atomic" — `meter.rs::reserve` does an UNLOCKED `rename` (the fan-out race) | real fix is build-new: `MeterStore::with_lane_locked` (fs4 advisory lock) + real `ReservationId`s. "cached" escalation rung = prefer-warm-lane, no CacheStore port |
| 2026-09-08 | fleet-store | `graph.rs` cited wholesale; only schema/read + txn-write belong here | graph.rs split: persistence→fleet-store, tree-sitter/git (696-1172)→fleet-context; 3 stores unified by convention, NOT a forced common trait |
| 2026-09-08 | src/ (worker spawn tests) | 351 tests passed while `fleet __agent` did not exist as a real subcommand — every spawn-path test substituted a fake child via `FLEET_WORKER_TEST_CHILD_EXE`, so nothing exercised the actual entry point | **lesson: a test seam that substitutes the binary can hide a missing entry point.** Fixed by driving `env!("CARGO_BIN_EXE_fleet")` — the real product binary — in at least one test per spawn path, not only the swapped-in fake |
| 2026-09-08 | src/ (fd-3 receipt channel) | `fcntl(3, F_GETFD)` was used as the check for "fd 3 is a receipt channel"; it passed spuriously because tokio's own kqueue reactor can independently land on fd 3 | **lesson: `fcntl(3, F_GETFD)` is a proxy for the property, not the property.** `getsockopt(3, SOL_SOCKET, SO_TYPE)` (checking it is actually the expected socket type) is the real check |
| 2026-09-08 | src/dispatch (command routing test) | a routing test asserted `contains("Commands::Agent")`; this string is also a substring of `Commands::Agents(a)`, so the test passed even when it should have caught a routing defect | **lesson: a substring check is not a symbol check.** The false-negative hid the very defect the check existed to find; assert on a full token/variant match, not `contains` |
| 2026-09-08 | repo-wide (gates/hooks/checks) | five separate instances of the same failure mode found across the tree: a corpus check `exit 77`-skipped when its source moved; the `recur` gate read a path absent from the materialized gates root; a SessionStart hook silently `exit 0`'d and injected nothing when its contract file moved; a gate reported success while every check inside it had failed; a pipeline stub returned success unconditionally | **lesson: fail-safe where fail-loud belongs is this repo's most repeated defect.** A gate that measures nothing has FAILED — it must error loudly, never degrade to a silent pass |
| 2026-09-08 | fleet/bin (cleanup) | cleanup deleted `fleet/bin/freelane.sh` (259 lines, the keyless lane) along with the rest of the dev scripts directory, because no crate's `Cargo.toml`/build listed it as owned; it was in fact load-bearing and had to be restored (now `crates/fleet-worker/assets/freelane.sh`, proven live in `crates/fleet-worker/tests/freelane_live.rs`) | **lesson: cleanup can delete a load-bearing implementation that "looks like" a dev script.** Before deleting a directory, check which of its files are referenced FROM CODE (grep for the path/filename across `src/` and `crates/`), not just which files look like throwaway scripts by name or location |

**`fleet-types` build must ALSO include (reconciliations surfaced by siblings — fold in before P3 builds fleet-types):**
- `LifecycleState::allowed_next()` — so fleet-lifecycle's caller validates transitions against one table (no 2nd copy).
- `ReceiptEvent::Rollback` — the 8th ledger variant (fleet's whitelist `main.rs:4373-4386` stores `"rollback"`); the drafted enum had 7.
- Confirm `Tokens` exposes checked add/sub (fleet-govern + meter rely on it).
- `valid_artifact_id` (from fleet-cli) — shared id validation belongs here, not in src/.

**More adjudications (2026-09-08, cli/plan/worker/memory batch):**
- `ConcurrencyCap::compute` (min(cores-2, ram_lanes, review_cap=3)) → **fleet-worker** (lane capacity is worker's concern); `src/` calls it. Pairs with `lane_cap()` already re-homed there.
- **fork/exec parent-child split:** `fleet-worker` owns the PARENT side (`spawn`/`join`) only. The child-side `__agent` re-exec + `dup2(fd,3)` entry stays in the `src/` binary (only the binary can re-exec itself). This is a real boundary, not a divergence to fix.
- `fleet-worker`'s HOME/XDG hermetic override is **new code** (today's fd-3 path passes real `$HOME`); it is the keyless-thesis crux. Its highest-risk unknown — ambient-credential leak (keychain / daemon socket) that `env_clear()`+tempdir can't catch — is **not automatable**; the M1 `probe_no_ambient` result needs human/Opus sign-off before the keyless claim stands.
- Dropped from deps as unused-in-source: `cedar-policy` (fleet-worker), `clap_complete` (fleet-cli). "A declared dependency is not an adopted one" — don't add back without a real call site.
- `agent.rs`'s `Assignment<S>` type-state machine (a 2nd machine parallel to `Task<S>`) is excluded from fleet-worker; owner TBD at build — fold into `fleet-lifecycle` or drop if redundant.
- SOW/intent hashing (`shasum`) → **fleet-store** (alongside its blake3 ledger hashing); plan/worker call it, not reimplement.
- Advisory subprocess calls plan excludes (`roles.sh`, `ratchet.sh`, node retry-read) reach their owners via **ports** wired in src/ (ratchet→fleet-verify gate orchestration; roles→fleet-types `Role`).
- **Dep risk (fleet-context):** `fastembed` hard-pins `ort =2.0.0-rc.13`; `ort` has never cut a stable release in 43 versions. → embeddings deferred behind the `VectorIndex` port; v1 is BM25(tantivy)+tree-sitter only. "Adopting a tool ≠ the tool working" — don't block the crate on a pre-1.0 native dep.
- **Dead crate (fleet-context):** the `rank-fusion` crate is fully yanked (`max_version 0.0.0`). RRF is hand-rolled (~15 lines), like the Thompson bandit. Verify a crate is alive before naming it.
- **P2 debt-clear is DEFERRED to pre-cleanup (P6):** the new `crates/`+`src/` workspace re-authors logic into fresh ≤80-line files; it does NOT inherit fleet's fmt/clippy/gitleaks red (that lives in `fleet/keel` code we're not git-moving). Clear fleet's red only when we delete/retire fleet in P6.
- **Build lesson (fleet-types):** a blueprint behavior-table claim was wrong — duplicate JSON keys on a `#[derive(Deserialize)]` struct are REJECTED by serde, not last-value-wins (last-wins only for a bare `serde_json::Value`/map). The build agent caught it and tested real behavior. Lesson: blueprint behavior tables are hypotheses; the build verifies them, and a wrong prediction becomes a corrected test, not a forced-through expectation.
- **Build-agent stall lesson:** the first fleet-types build agent spent 34 min re-grepping fleet source to re-verify the (already-reviewed) blueprint and wrote nothing. Fix: build agents must TRUST the blueprint, scaffold + `cargo check` first, write incrementally, and stop reading after ~10 min. Codified in `_ADDING-A-CRATE.md`.
- **cargo-mutants race lesson:** 5 wave-1 build agents each launched `cargo mutants` and hung — concurrent mutants runs collide on the shared `mutants.out`/`target` in one workspace. Fix: build agents do NOT run cargo mutants; **Opus does mutation review by hand** (one mutation per crate, confirm a test kills it). Codified in `_ADDING-A-CRATE.md`. This is also faster.
- **Move (2026-09-08):** owner asked ``+`src/`+`crates/`+workspace `Cargo.toml` to live inside `fleet/`. Done atomically after wave-1 agents stopped (moving mid-build would corrupt writers). Excludes updated to keel/registry-reference/tests/crew. Workspace green at `fleet/`.
- **Test-file ≤80 fix pattern:** integration test files that bust 80 lines get their helpers extracted to `tests/support/mod.rs` (`#![allow(dead_code)]`, the std idiom) rather than split by test — keeps each test file focused and under the cap.
- **Adversarial-verifier findings (2026-09-08), both real, both the "proxy is not the property" class:**
  (1) fleet-govern `settle` id check was under-tested — the cross-lane test left the reservation list empty so the id comparison never ran; neutering it passed all 14 tests. Fix: `tests/settle_stale_id.rs` keeps a 2nd reservation open; Opus mutation-verified it kills `.position(|_| true)`.
  (2) fleet-worker hermetic test first checked HOME's *value* (a proxy — `.env("HOME",…)` sets it regardless of `env_clear()`); the real property is that a non-allowlisted sentinel var never reaches the child. Builder caught + fixed it, re-confirmed the env_clear-deletion mutant dies.
  Lesson reinforced: a green suite is not proof; the adversarial mutation is. Verifiers ≠ builders (separation of duties) earns its keep.
- **Concurrency lesson:** 4 parallel Sonnet verifiers + idle build agents all running `cargo` deadlocked on the shared `target/`. One workspace build serializes better than N contending per-crate runs. Cap concurrent cargo-running agents.

## 8. Resume protocol (quota died mid-run)

1. Read this file top-to-bottom. 2. Find crates not `DONE`. 3. For each, read its
`<crate>/BLUEPRINT.md`. 4. Continue at its `state`. 5. `opencode` can build a crate solo
from its blueprint; run §6 before marking done. 6. Opus (when back) does P4 review — never skip it.
