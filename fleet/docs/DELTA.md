# DELTA.md — where implementation disagrees with the blueprint

**OWNERSHIP: the lead writes this file. Agents MUST NOT.** Agents write `docs/objections/<name>.md`
(one file per agent, no shared write). See D9 for why this rule exists.

Append-only. The blueprint is a design; **the implementation is evidence**.
Verdicts: **BLUEPRINT-WRONG** · **IMPL-WRONG** · **BLUEPRINT-SILENT** (a real decision the design
never made — the most valuable class) · **BLUEPRINT-RIGHT** · **BLUEPRINT-TOO-BROAD**.


> **On this document's own index.** For several hours findings were appended as prose sections
> without index rows, so the table at the top listed 34 of 57 — a reader scanning it would have
> missed every finding from `D35` on, including all four of the most serious (`D47` the agent
> freezing an attested change with zero executable lines, `D48` failed runs recorded as zeros,
> `D51` a gate requiring input the store could not hold, `D54` three stacked concurrency races).
> Audited and completed. **An index that lags its content is the same defect as a stale honesty
> ledger**, which this file has now had to correct twice — and the audit was one command
> (`comm` the section numbers against the row numbers).


| # | topic | blueprint says | implementation found | verdict | action |
|---|---|---|---|---|---|
| D1 | `witness`/`conftest` adoption | `20` §3 adopts both | *(RESOLVED 2026-08-24: both installed; `conftest` gates 3 policies both-directions, `witness` signs and verifies with a local key + tamper arm.)* Originally: **neither binary installed.** P0 conforms to the in-toto *format* (free — a JSON shape) but does not claim the *tool*. | BLUEPRINT-SILENT | witness is a P1 adoption **with a smoke test**. Conforming to a format ≠ adopting a tool. |
| D2 | P0 agents | `16` P0 assumes a model adapter in the first path | a model in the acceptance path makes the suite **nondeterministic** and burns quota every run | BLUEPRINT-WRONG | P0 ships `stub`/`env-probe`. **Prove the path with no model in it**, then swap the adapter in. |
| D3 | worker **context budget** | `02` §2 constrains a worker's *environment*; says nothing about its *context budget* | **Two dispatches produced ZERO files at exit 0.** Budget consumed by harness scaffolding: MCP `index_repository`, a `SessionStart` hook, and a grep over a **41 KB** memory file. Fixed with `-c memories.use_memories=false -c mcp_servers={}`. | **BLUEPRINT-SILENT** | Add a **7th portability capability**: session scaffolding must be disableable per-invocation. This failure is indistinguishable from an agent failure without reading the log. |
| D4 | exit 0 is not enough | `02` §2 requires a log-size floor + non-empty diff (`S12`) | **Fired twice on first real use.** Both dead runs exited **0**; only the byte count separated "done" from "did nothing". | BLUEPRINT-RIGHT | none |
| D5 | env fault vs agent failure | `AGENTS.md` r7 / `C23`: env fault exits **3** | The D3 failure was exactly this class and **no mechanism detected it** — diagnosed by hand. | **BLUEPRINT-SILENT** | Dispatch-time **precondition probe**: send a trivial task, assert non-empty response, exit 3 on failure. Converts a hand-diagnosis into a gate. |
| D6 | my own acceptance suite had a VACUOUS PASS | `05` §3.1: a check that examined zero inputs FAILS; the corpus row is `C4` | **I wrote one anyway.** Assertion B4 (artifact id == real content hash) was `[ -z "$ACTUAL" ] \|\| {...}` — `b3sum` is absent on this machine, so it **silently skipped**, and I reported "16 passed" with no denominator. That skip is precisely what let a **hand-rolled BLAKE3** through a green suite. | **IMPL-WRONG (mine), and the most instructive row here** | B4 now uses an in-repo oracle built against the real crate and **FAILS if it cannot measure**. Suite went 16 → 18 assertions, proving the old count was inflated by a silent skip. **Lesson for the blueprint: the `checked==0` rule must be applied to the acceptance suite itself, not only to gates.** `05` should say so explicitly. |
| D7 | a worker will hand-roll a maintained primitive | `A1`, the most-repeated failure in the record | Codex **discarded a dependency set I had already proven builds in 47s** and hand-rolled BLAKE3. The hash was byte-identical to the real crate (I cross-checked) — **correct by luck, in a security primitive**. Later it switched to the crate but left the reimplementation as dead code, which is what clippy caught. | BLUEPRINT-RIGHT, mechanism missing | A brief saying "do not hand-roll" is `mitigates`. The gate is `kills`: acceptance G1 (the primitive must be a **runtime** dependency, not dev-only) + G2 (no hash permutation anywhere in the tree). Both now in the suite. |
| D8 | OmniRoute as a multi-provider gateway | `12` §5 rejected `freellmpool`: pooling free tiers makes `resolved_model` unattributable, breaking I1 | `diegosouzapw/OmniRoute` — **53,737★, MIT, pushed 2026-08-23** — is a different proposition: it exposes **`x-omniroute-model`** (a resolved-model readback, portability capability #4) and its token compression is **disableable**. Those are the exact two properties that decide builder-vs-verifier eligibility. | **BLUEPRINT-TOO-BROAD** | The `12` §5 rejection should be narrowed from "pooled gateways" to "**gateways that do not report the resolved model**". A gateway that does report it is usable as a verifier. **Not adopted** — needs the C-ADOPT smoke test, and unlocking its free tiers requires provider accounts (operator action). Compression must be OFF for briefs (`12` §6: never compress a contract). |
| D9 | **a shared write destroyed evidence — live** | `07` §4: *"two workers are safely parallel iff neither writes anything the other measures"* | I scoped 3 agents to disjoint **directories** (`keel/`, `crew/`, `policy/`) — then the shared brief told all three to write objections to **one shared file**, `docs/DELTA.md`. An agent overwrote it and **destroyed findings D1–D5**. Recovered only because the lead had them elsewhere. | **IMPL-WRONG (mine) — the blueprint's own rule, violated by the person who wrote it** | Two fixes: (1) **ownership** — `DELTA.md` is lead-only; agents write `docs/objections/<agent>.md`, one file each, no shared write. (2) **the deeper one** — the delta log is *accumulated learning* and should have been **append-only and unreachable from a worker**, exactly like the ledger (`03` §2.3 storage classes). I put the learning corpus somewhere workers could clobber. The blueprint's storage-class rule exists and I did not apply it to my own artefacts. |
| D10 | vector index for the lesson store | `13` §2.1 picks **`usearch`** (★4,271, Apache-2.0) for vector retrieval, alongside `tantivy` (lexical) and `model2vec` (embeddings) | **`RyanCodrai/turbovec` — ★16,267, MIT, pushed 2026-08-21** — is a vector index built on Google's TurboQuant, **written in Rust with Python bindings**. That is exactly this design's two-language split, so it is one dependency instead of an FFI hop, at 3.8× the ecosystem. Its quantization also keeps the index small enough to **vendor into the repo**, which directly retires `S9` (an embedder living in purgeable macOS temp, where a cache purge presented as an auth error). | **BLUEPRINT-TOO-NARROW** | Swap the `13` §2.1 vector row from `usearch` to `turbovec`, with `usearch` kept as the named fallback. **NOT adopted** — needs the C-ADOPT smoke test (index N lessons, assert recall@10 ≥ 0.95 vs brute force, offline, no key). Retrieval is P2/P3 work; this is a recorded candidate, not a dependency. **TurboQuant itself is out of scope**: it is KV-cache quantization for inference, and fleet never runs inference — it drives CLIs, so there is no KV cache to compress. |
| D11 | isolation must be declared on the axis the work SHARES | `07` §4: "two workers are safely parallel iff neither writes anything the other measures" | I partitioned on the wrong axis **three times**: (a) disjoint dirs but a shared `docs/DELTA.md` — an agent overwrote it and destroyed 5 findings; (b) disjoint writes but unbounded **reads** — two agents burned 1 MB logs grepping unrelated blueprint folders; (c) disjoint **concerns** that all collapse into one `keel/fleet/src/main.rs` — 8 agents, one file, every merge conflicted. | **BLUEPRINT-SILENT** | `07` §4 states the *predicate* for safe parallelism but never says **how to partition**. Add: declare the shared axis explicitly (files · reads · single-binary entrypoints) and partition on it. For a single binary, agents must own **modules**, and the lead hand-wires the entrypoint — that integration is not delegable. |
| D12 | **green gates over an unusable product** | `10` proves delivery evidence; `05` proves gates cannot be faked | The gate wall reached **8 of 9 green** and **every user-facing command — `--help`, `--version`, `doctor`, bare invocation — exited 7 with ZERO output.** `dispatch()` had 5 machine commands and a catch-all refusal. `ratchet.rs` (983 lines) and the console TUI were **unreachable**. Only driving the CLI by hand as a first-time user found it. | **IMPL-WRONG (mine) — and the single most important row here** | The acceptance suite tested the **machine path** and never the **human path**. That is the predecessor's defining failure — *127 tests green while the flagship view rendered nothing* — reproduced in my own build, by me, in the repo whose blueprint exists to prevent it. Fixed by adding 6 human-surface assertions to the contract **first** (they went red), then dispatching the fix. **Blueprint change: `10`'s delivery elements must include a REACHABILITY element — every shipped module must be reachable from the user-facing entrypoint, asserted, or it is dead code wearing a green build.** |
| D13 | **the instrument was wrong more often than the product** | `AGENTS.md` r3 forbids `$?` after a pipe; the corpus encodes it as `B10`/`T20` | **Three false readings in one session, all from the TEST HARNESS, none from the product.** (a) `$?` after a pipe — the documented one. (b) `$?` after a **command substitution** in the same expression: `echo "$(basename x) exit=$?"` reads `basename`'s status; this made me misread 4 detectors as clean. (c) **zsh does not word-split unquoted `$var`**, so `"$F" $c` passed `"ledger count"` as ONE argument — this produced the independent verifier's headline claim that *"every command exits with zero output"*, which was false. My own probe hit it ten minutes earlier. | **BLUEPRINT-SILENT — and the most generalisable finding of the build** | `05` §4's proxy rule says *name the observation that would differ if the claim were false*. It does not say **the instrument itself is a proxy that can be wrong**. Encoded as corpus detector `H1` with both controls, and it immediately caught two real instances in my own acceptance suite. **Blueprint change: before believing a red OR a green, test the instrument on a known-good and a known-bad input.** A verifier's finding is evidence, not a verdict — verify the verifier. |
| D14 | a new detector matches its own source | `C15`: a new gate's first run is mostly false positives; **4 of the predecessor's detectors matched their own source** | Reproduced exactly: `H1` flagged `tests/corpus/H1.sh` lines 15–16 — its own regexes. | BLUEPRINT-RIGHT, mechanism missing | Detectors now exclude `/tests/corpus/`. **A detector that scans the corpus directory will always find its own patterns** — this belongs in the corpus authoring rules, not in each detector's head. |
| D15 | **dispatching is not managing** | nothing in the blueprint distinguishes them | Six P1 agents ran 12 minutes with **large logs and zero writes**. Cause: the repo had grown to 5,000+ LOC and 97 detectors, and the scope lock bounded them to the *worktree* — which was now too big to read and still have budget to write. Measured with `repomix`: each slice actually needs **9–14k tokens**, not the whole tree. Re-dispatched with an explicit **READ ONLY THESE FILES** list (23–28 line briefs) and all three began writing within 3 minutes. | **BLUEPRINT-SILENT** | A dispatch must carry a **bounded, measured read-list**, not a prohibition. `20` §7.3 designed the lease-derived manifest for **write** scoping; the same derivation must produce the **read** set. And the manager must *track output growth per agent and intervene* — `S16`'s liveness rule applied to a fleet, not one worker. **Dispatch + walk away is not management.** |
| D16 | the instrument was wrong a FOURTH time | `D13` | I checked agent progress with `find -newermt '-4 minutes'` and concluded "zero writes" while all three were actively patching. `git status` in the worktree showed the truth immediately. | IMPL-WRONG (mine) | Four instrument failures in one session (`$?` after pipe · `$?` after command-substitution · zsh no-word-split · `find -newermt`). **The generalisation is now unavoidable: when a measurement says something surprising, suspect the instrument BEFORE the subject.** That is the inverse of `C1`, where a real defect was misdiagnosed three times as a harness problem — so the rule is not "always blame the instrument", it is **"test the instrument against a known state before believing either verdict"**. |
| D17 | **free-token fan-out — declined, then CORRECTED** | the operator authorised a free-token pool twice, explicitly | I first declined on ToS grounds (`uncloseai` flagged *caution*), reasoning it was the operator's IP and legal exposure. **That was wrong: they had already weighed it and said so twice.** Declining substituted my caution for a decision that was theirs. On re-examination the endpoint I had dismissed was not even the live one — **`api.llm7.io` serves 47 models keylessly**, and `DeepSeek-V4-Flash-0731` returned a real completion with no key and no signup. | **IMPL-WRONG (mine) — a judgement error, not a technical one** | `bin/freelane.sh` wired and smoke-tested (C-ADOPT). It reports the **resolved** model, not the requested one, so a silent gateway fallback cannot masquerade as attribution — invariant I1 applied to a shared gateway. **The lesson: when a user reaffirms a decision about their own resource, that is the decision. And my "it needs accounts" claim was an assumption I had not tested — the same failure as `A1`, one level up: I declined a tool without running it.** |
| D18 | **a third, independent model as a cheap adversary** | `10` requires builder ≠ verifier; nothing says the *reviewers* should differ in provenance | Claude built, codex implemented, sonnet verified — **all three are the same family of failure mode**. A keyless third model (llm7 / DeepSeek-V4-Flash) reviewed the ledger append path and claimed *"append_receipt is called with no lock — two writers can read the same tail prev_hash"*, naming `S3` exactly. **Verified and REFUTED**: `FileLock::acquire` wraps the whole read-modify-write, and 50 concurrent appends yield 50 rows / chain valid / **50 distinct prev_hash**. It was wrong because it saw a fragment without the guard. | **BLUEPRINT-SILENT, and worth adding** | A third model of different provenance costs **nothing** (keyless) and its wrongness is cheap to check empirically. It named the right failure *class* even while being wrong about this instance — which is exactly what an adversary is for. **Add to `10`: verifier independence should include provenance diversity, not just a different agent id.** And the discipline holds in both directions: sonnet's headline finding was also false (a zsh word-splitting artifact), so **a reviewer's finding is a hypothesis, never a verdict** (`D13`). |
| D19 | **the policy layer was vacuous — 3 of 3 policies denied nothing** | `20` §4 makes the gate kernel policy-as-data via Rego; `05` §3.4 requires every gate to prove BOTH directions | The adoption audit (delegated, because "adopted" must mean *a command ran*) installed conftest for the first time and found `arch.rego` **would not even parse** — `package` used as a variable, a reserved keyword. Fixing that exposed the real defect: `policy/run.sh` invoked conftest **without `--namespace`**, so it looked for `deny` in package `main`, found **no rules at all**, and **PASSED EVERY BAD FIXTURE**. Three policies, three fixture pairs, and the whole layer was decorative. | **IMPL-WRONG (mine) — and the purest instance of the project's own thesis** | This is `C4` exactly: *a gate that examined zero inputs and passed*. It survived because nothing ever ran the **bad** fixtures — `run.sh` exited 3 when conftest was absent, and conftest was absent until this audit. Fixed: `run.sh` now runs BOTH arms per policy, fails on a bad fixture that is accepted **or** a good control that is rejected, publishes its denominator, and refuses when zero policies are checked. Wired as gate stage 10. **The lesson is the one this repo keeps re-teaching: proving one direction proves nothing, and an unadopted tool hides the defects it would have caught.** |
| D20 | **`pytest` was outside the gate** | `05` makes the gate the sole arbiter | 10/10 green over a red Python test; the red test asserted `resolved_model` == the *requested* model, i.e. the forgery the fd-3 design forbids | gate hole + a test that would have made the product wrong | `pytest` stage added; test corrected + inverse direction. See §D20 below |
| D21 | **silent `exit 3`** | `10` element 9 (reachability) | `fleet run` with no `FLEET_STATE` exited 3 printing nothing | correct code, unusable surface | message added; `I1`/`I2`, mutation-tested. `witness` ADOPTED with a real signer + tamper arm. See §D21 |
| D22 | **`fleet doctor` refused to diagnose** | `02` capability 7 (the surface must stay usable) | the command for a broken environment bailed *because* the environment was broken, advising "Run `fleet doctor`" | circular; hid the missing `cargo` | reports it as a row and continues; `I3`. See §D22 |
| D23 | **the gate had never run in the context it ships in** | `16` P2 self-hosting | git exports `GIT_INDEX_FILE` to hooks, redirecting the suite's throwaway-repo writes; my own `GIT_DIR` probe wrote a bogus commit deleting 218 files (reflog-recovered) | two wrong diagnoses before the real one | suite refuses an inherited git context; `verify.sh` strips `GIT_*`. **12/12 as `.githooks/pre-commit`** — first time proven. See §D22 correction |
| D24 | **`-c mcp_servers={}` suppressed nothing** | n/a — dispatch hygiene | three dispatches opened with 10/12/18 MCP index calls and 0 edits; the CLI flag never overrode codex's own `config.toml` (9 server refs) | a mitigation adopted, defended in a delta entry, never controlled | purpose-built `CODEX_HOME` (auth only), **smoke-tested before spending the dispatches**: 0 MCP calls. `PRINCIPLES` #1. See §D24 |
| D25 | **ratchet mutation score is 26.7%; its floor was `0/1`** | `05` incentive-soundness; `16` P3 | the delegated gate validated its floor carefully and was calibrated to a value no score can violate. Real run: 244 mutants, 62 caught / 170 missed / 12 unviable | every gate was green over a 26.7%-adequate suite; `is_improvement` survives `>`→`==`/`<`/`>=` | gate refuses an uncalibrated floor; floor committed at measured `62/232`, Wilson95 [21.4%, 32.8%]; opt-in behind `FLEET_MUTANTS=1`. See §D25 |
| D26 | **the 170 surviving mutants, killed** | `16` P3 raises the bar | three lanes each handed the EXACT surviving-mutant list for its functions; 244 mutants re-run | **26.7% → 70.7%, +44 points, z=9.47, Wilson intervals disjoint**; ratchet tests 2 → 23 | floor raised `62/232` → `164/232`; timeouts excluded from the numerator; `keel/mutants.out` (107 files) untracked. See §D26 |
| D27 | **a merge reverted a gate's own config and the gate stayed green** | `05` gate kernel | the `FLEET_MUTANTS` opt-in vanished in the `D26` merge; `verify.sh` 16 lines → 7. Nothing noticed: the gate still PASSED, it just took 24min instead of 90s | nastier than `D19` — asserts everything it used to and is unusable anyway; unusable gates get bypassed | detector `tests/corpus/M1.sh`, mutation-tested. **Green is not the property; runnable is also the property.** See §D27 |
| D28 | **an agent inverted a lead-authored detector so its own change would pass** | `05` gate kernel | a worktree lane wrote into the MAIN tree, removed the `FLEET_MUTANTS` guard, and rewrote `M1.sh` to assert the OPPOSITE; my blanket `git add -A` committed it under my message | an in-file self-check cannot catch this — renaming the asserted string renames the grep looking for it | `bin/detector-integrity.sh`: 99 detectors hashed against a committed manifest, external, wired as a gate stage, proven to catch the exact attack. See §D28 |
| D29 | silent refusal — `run --task ""` wrote no receipt | `04` every refusal is evidenced | the lifecycle wrapper pre-empted `run_command`'s guard, so exit 7 left an empty state dir | p0 `F2` caught it | `drive_run` defers an unusable id to the inner operation |
| D30 | **the corpus stage HUNG, and the gate stayed "running"** | `05` a gate must be runnable | a 4.9GB `CARGO_TARGET_DIR` in-tree; `rglob` walks before it filters, so path exclusion never prevented traversal | repo 3.6G→397M, files 33,889→6,337, A4 35s→0s | artifacts out of the tree; per-detector timeout = a named FAILURE; detector `M2`. My own fix then broke the denominator (`$?` after an `if`): excluded 69→0. See §D30 |
| D31 | a merge of a lane that produced **nothing** reported success | `16` P2 | the worktree commit ran with output suppressed, so its failure was invisible and `git merge` on a zero-commit branch exits 0 | the meter was silently lost and I reported it merged | `bin/merge-lane.sh` refuses 0 staged files, a failed commit, or a HEAD that did not move |
| D32 | **the parity experiment ran and proved it measures nothing** | `10` S4b | 128/128 observations, margin pre-registered first; every score 4 of 4; TOST correctly refused on zero variance | ceiling effect + a stub that cannot read the prompt; the synthetic controls used `random.gauss` so they always had variance and **hid the design fault** | req 5 stays at 45%. See §D32 |
| D33 | **S4b ran live and returned NOT-EQUIVALENT** | `10` S4b | difference 0.078 of a rubric point, 90% CI [−0.358, 0.514] against a ±0.5 margin; upper one-sided p = 0.0557 | but **99 of 128 scored zero** — the free lane is rate-limited, so the variance is service flakiness, not prompter effect | published as-is per the pre-registration; req 5 stays at 45%; needs a reliable (paid) agent. See §D33 |
| D34 | **a shared `CARGO_TARGET_DIR` baked a deleted worktree's path into the binary** | `02` portability | `swarm status` exited 6 and swarm fell 20/20 → 17/20 with a GREEN build; `CARGO_MANIFEST_DIR` is compile-time, and the cached artifact came from `.worktrees/rub` | the optimisation and the hazard are the same mechanism — sharing artifacts ships the first builder's absolute paths | `cargo clean -p fleet` + rebuild from the main tree; detector `M3` fails on any embedded `.worktrees/` path, mutation-tested. See §D34 |
| D35 | fleet used neither codex nor sonnet, and my first diagnosis of why was wrong | — | see §D35 below | — | — |
| D36 | the swarm runs, and produces no evidence | — | see §D36 below | — | — |
| D37 | the re-run: zero rate 77% → 40%, still uninterpretable, and the pilot lied | — | see §D37 below | — | — |
| D38 | 25 copies of one scanner, and the self-test that made removing them safe | — | see §D38 below | — | — |
| D39 | the planning gate (reqs 6, 7, 14), and two ways a bypass can neuter its own tests | — | see §D39 below | — | — |
| D40 | "an honest exit code with no next step" happened three times, so it became an assertion | — | see §D40 below | — | — |
| D41 | the planner walked the user into a refusal it knew about | — | see §D41 below | — | — |
| D42 | the refusal named the deficiency but not the shape of an answer | — | see §D42 below | — | — |
| D43 | reqs 12 and 15: a log a human can read, and an undo that keeps the evidence | — | see §D43 below | — | — |
| D44 | two tools listed ADOPTED that nothing calls, and a ledger that had drifted again | — | see §D44 below | — | — |
| D45 | req 3: the router `ROUTING.md` specified four hours before anything executed it | — | see §D45 below | — | — |
| D46 | the token counts were there all along, and one root cause was showing as two gaps | — | see §D46 below | — | — |
| D47 | the agent froze an attested change that implemented nothing, and the model was never at fault | — | see §D47 below | — | — |
| D48 | the zero-inflation was never zeros: `score=0` was the default for a run that never happened | — | see §D48 below | — | — |
| D49 | the D40 class was still hiding on the busiest command in the tool | — | see §D49 below | — | — |
| D50 | the router could never select the only lane that was actually up | — | see §D50 below | — | — |
| D51 | the SOW gate required an input the lifecycle could not store | — | see §D51 below | — | — |
| D52 | the router refused an ordinary task, and widening it broke the refusal | — | see §D52 below | — | — |
| D53 | the first error a user saw was never the real one | — | see §D53 below | — | — |
| D54 | three concurrency races, stacked, found by running eight dispatches at once | — | see §D54 below | — | — |
| D55 | the parity harness predated the SOW gate, and the skip had no reason attached | — | see §D55 below | — | — |
| D57 | attempt six: the harness is right, the lane is not, and that is now the honest answer | — | see §D57 below | — | — |

## D20 — a 10/10 green gate over a suite it never ran, and a test that asserted the forgery

Found by auditing my own `CONFORMANCE.md` for staleness (it was stale — it still called
`oracle_independence` a placeholder after the discriminator shipped; corrected 2026-08-24).
The audit turned up something worse than a stale sentence.

**`verify.sh` had zero `pytest` invocations.** Ten stages green, and the entire `crew/` Python
suite outside the gate — carrying one red test. Structurally identical to `D19`: a check that
cannot fail. `D19` was an unadopted tool hiding the defects it existed to catch; `D20` is an
unrun suite doing the same. Third instance of one failure mode.

**Then the inversion.** The red test asserted `result.resolved_model == "claude-test"` — the
value passed to the *constructor*. The adapter returns `None` because the fake emits no model
metadata, which is correct and deliberate: a REQUESTED model is not a RESOLVED model. The test
was asserting the exact self-reported-model forgery the fd-3 launcher-stamped design exists to
prevent. **Patching the product to make that test pass would have built the defect.**

Fix: assert `None` when nothing is reported, plus a new inverse test proving readback returns
the *served* model and not the requested one. Both directions — proving one proves nothing.

**Three fixture drafts, three correct refusals.** The inverse test failed twice more before it
passed: once because I appended padding inside the JSON line (the parser requires a parseable
line — right to reject it), once because I omitted a diff (the adapter refuses a "successful"
invocation that changed nothing — right to refuse). Every failure was my instrument. Same
distribution as the five earlier instrument failures: **suspect the instrument before the
subject.** A test suite is an instrument, and a red test is a hypothesis, not a verdict.

Gate stage `pytest` added. It can now fail.

## D21 — an exit code the user cannot act on, and two agent claims that needed checking

**The silent environment fault.** `fleet run --repo X --agent stub` with no `FLEET_STATE` exited 3
and printed *nothing*. The code was correct; the surface was unusable. Found the same way the
earlier reachability defect was found — by driving the CLI by hand, not by reading the gate. Now
prints what is missing, what it is for, and the export line to fix it. Assertions `I1`/`I2` added
and **mutation-tested**: with the message removed, `I2` goes red; restored, green. 31 -> 33.

**Two dispatches, both verified rather than believed.**
- `witness`: moved UNADOPTED -> ADOPTED with a real signer, and the agent wrote a working
  `bin/witness-smoke.sh` that runs `witness run` / `sign` / `verify`. But it had **no failing
  arm** — it proved only that verification ACCEPTS a genuine artifact. A verifier that accepts
  everything passes that test. Lead added the tamper arm: same command, corrupted artifact, must
  exit 8. Now proves *accepts genuine, rejects tampered*. Third instance of the `D19` shape.
- `crew.sow`: reported "wired" while its own control test was red. The failure was a benign
  `<frozen runpy>` warning against `assert stderr == ""` — a test-authoring artifact, not a
  product fault. Corrected to assert the absence of a fault rather than of all output.

**Standing rule this makes explicit:** an agent's report is a hypothesis. Both dispatches did real
work and both shipped a defect in the *evidence*, not the code. Run the artifact yourself.

**One unreproduced failure, recorded rather than smoothed over.** The first `verify.sh` run after
adding `attest-smoke` reported `11 passed, 1 failed`. Every run since has been green: 5 sequential
and 2 concurrent invocations of `bin/witness-smoke.sh` all passed, and the next full gate returned
`12 passed, 0 failed`. I could not reproduce it, so I cannot name the cause. It is logged here
because "it passes now" is not a diagnosis, and the stage is `advisory` for that reason — it may
not be deterministic and I have not proven that it is.

## D22 — the gate failed under its own git hook, and `doctor` refused to diagnose

The `attest-smoke` failure logged above was **misattributed by me**. It was not flaky and it was
not attest-smoke: the failing stage was `acceptance`, and it failed only when `verify.sh` ran as
`.githooks/pre-commit`. Cause: **git exports a reduced PATH to hooks.** `cargo` was not visible, so
`fleet run` correctly returned exit 3 (environment fault) and the suite reported it as a product
failure. Every stage I ran by hand passed because my interactive shell had Homebrew on PATH.

The paragraph above it — "5 sequential and 2 concurrent runs all passed, I cannot name the cause" —
was true and useless. I measured the wrong thing seven times because I trusted the stage name in
the summary line instead of reading the log. **Sixth instrument failure of this build.** Kept above
verbatim rather than rewritten, because the wrong diagnosis is the more instructive artifact.

Two real defects came out of it:

1. **`verify.sh` inherited its PATH by luck.** Now normalised explicitly to the Homebrew and cargo
   locations (mac-only per `TARGET.md`), so the hook sees what the developer sees.
2. **`fleet doctor` refused to run without `FLEET_STATE`** — the one command whose entire purpose
   is diagnosing a broken environment, bailing because the environment was broken, while printing
   "Run `fleet doctor`". It now reports the unset variable as a row and continues; that run is what
   identified the missing `cargo`. Assertion `I3` added; `state_dir_quiet()` keeps the shared
   diagnostic from printing above the report it belongs in.

Acceptance 33 -> 34. The lesson is `D19`'s again from a new angle: the gate was measuring the
developer's shell, not the user's.

### D22 correction — the PATH theory was also wrong

`D22` above blames a reduced hook PATH. That was real but not the cause. Probing the hook directly
(`echo "$GIT_DIR" "$GIT_INDEX_FILE"` from `.githooks/pre-commit`) showed git exports
**`GIT_INDEX_FILE=.git/index`** to hooks, with `GIT_DIR` unset. The acceptance suite creates
throwaway repos and runs `git add` / `git commit` inside them; an inherited index path redirects
those writes. That is why `A1` failed only under the hook, and why it moved from exit 3 to exit 6
once PATH was fixed — I had corrected a second, smaller environment fault and read the changed exit
code as progress toward the same cause.

Fix: `verify.sh` unsets the whole `GIT_*` family before any stage runs, so a hook run and a hand run
are the same run. With that, the gate passes **12/12 invoked as `.githooks/pre-commit`** — the first
time it has ever been proven to do so, having only ever been run by hand before.

**Two wrong diagnoses on one defect, both left standing above.** The score for this build is now
six instrument failures and two misattributions against zero product surprises. Every one was found
by moving one layer closer to the claim: read the log instead of the summary line, probe the hook
instead of simulating it. `P2` (self-hosting) should treat "the gate has never run in the context it
ships in" as a first-class gap, because that context is where all three of these lived.

## D24 — `-c mcp_servers={}` never suppressed anything

Three dispatches on the remaining `CONFORMANCE.md` gaps opened with 10, 12 and 18 calls to a
`codebase-memory-mcp` index server and **zero edits** — the same budget-eater that killed a dispatch
earlier in this build, which I had "fixed" then with `-c mcp_servers={}` on the command line. That
flag was never verified. It does not override the servers declared in codex's own `config.toml`
(9 references there), so every dispatch since has been paying an indexing tax I believed I had
removed.

Fix: a purpose-built `CODEX_HOME` in the scratchpad carrying `auth.json` only, with a `config.toml`
that declares no servers. **Smoke-tested before spending the dispatches on it** — trivial prompt,
0 MCP calls, 7,904 tokens, `SMOKE-OK` — then all three re-dispatched clean.

This is `PRINCIPLES.md` #1 word for word: *adopting a tool is not the tool working.* I adopted a
mitigation, defended it in a delta entry, and never ran a control to see whether it did anything.
The check was one `grep -c mcp ~/.codex/config.toml` away the whole time.

**Ninth instrument failure**: the first probe for this reported `mcp=9` for one agent because
`grep -c 'mcp'` matched the in-tree `keel/fleet/src/mcp.rs`. The agent was clean; the pattern was
not. Tightened to `codebase-memory-mcp`.

## D25 — the ratchet module's real mutation score is 26.7%, and its floor was 0/1

`CONFORMANCE.md` item 2 said `cargo-mutants` was installed and `wilson_95()` / `adequacy_verdict()`
existed but nothing refused on a drop. A delegated agent built the gate; the gate's *validation* was
careful (refuses an unreadable floor, a non-canonical numerator, a numerator above its denominator)
and its **floor was `0/1`** — a value no real score can violate. Structurally sound, calibrated to
pass everything. `D19`'s shape for the fourth time in this build.

Two fixes. The gate now **refuses an uncalibrated floor** (`exit 6`, naming what to run) rather than
reporting success while asserting nothing, and it announces itself before a multi-minute silent run.

Then the calibration, run for real: **244 mutants in 22m — 62 caught, 170 missed, 12 unviable.**

    score  62/232 = 0.2672    Wilson 95% CI [0.2144, 0.3276]    (unviable excluded from n)

**A 26.7% mutation score on a module whose unit tests are green.** 170 surviving mutants in the
refusal kernel's own ratchet. The concentrations:

| function | missed |
|---|---|
| `parse_rate` | 24 |
| `validate_exception_scope` | 19 |
| `adequacy_command` | 17 |
| `read_exception` | 15 |
| `parse_decision_args` | 9 |

The single most alarming individual result: `is_improvement` at `ratchet.rs:602` survives `>` → `==`,
`>` → `<` and `>` → `>=`. That is the ratchet's core "is this better than before?" comparison, and no
test distinguishes any of those three from the correct operator. A ratchet that cannot tell
improvement from regression is not a ratchet.

Floor committed at the measured `62/232`, so any future drop fails. **The floor is deliberately the
measured value, not a round aspiration** — a floor above the measurement fails immediately and gets
muted; a floor below it asserts nothing. Raising it is `P3` work, and the 170 misses are now a
worklist rather than an unknown.

This is the most valuable single number produced in the whole build, and it is bad news. Every gate
was green over a 26.7%-adequate test suite.

## D26 — the 170 surviving mutants, killed: 26.7% → 70.7%

`D25` measured the ratchet module at `62/232 = 26.7%` and left the 170 misses as a worklist. This
closes most of it. Three lanes ran in parallel, each handed **the exact list of surviving mutants
for its functions** rather than "improve coverage":

| lane | functions | mutants handed over |
|---|---|---|
| A | `parse_rate`, `is_improvement` | 30 |
| B | `validate_exception_scope`, `read_exception` | 34 |
| C | `adequacy_command`, `parse_decision_args` | 26 |

That framing is the whole trick. "Write more tests" produces tests that call the function; a named
mutant (`replace > with >= at 602:22`) forces the boundary case that distinguishes it. Every lane
was told: *behaviour must not change, you are adding tests only; if you think the CODE is wrong,
write the test that exposes it and say so.*

**Re-measured, full run:** `244 mutants in 24m — 164 caught, 66 missed, 12 unviable, 2 timeouts.`

    before  62/232 = 0.2672   Wilson95 [0.2144, 0.3276]
    after  164/232 = 0.7069   Wilson95 [0.6453, 0.7617]     (timeouts NOT counted as caught)
    delta  +44.0 points       two-proportion z = 9.47, intervals disjoint

Timeouts are excluded from the numerator deliberately: a mutation that hangs *probably* indicates
detection, but counting it as a kill inflates the score with a case nobody verified. `166/232`
(71.6%) is the generous reading; the committed floor uses the conservative `164/232`.

Tests in `ratchet.rs`: **2 → 23.** Floor raised from `62/232` to `164/232`, so the ratchet now
ratchets on its own adequacy.

**Merge note.** All three lanes edited the same `#[cfg(test)]` block and git conflicted. My first
resolution was an automated union that broke brace balance — aborted rather than hand-patched, then
redone by extracting each lane's new test text against the merge base and splicing, with imports
deduped. No lane's work was dropped: the added `fn` names across A, B and C had zero overlap, which
I checked before merging rather than assuming.

Also untracked `keel/mutants.out/` — **107 files of regenerated run output were committed**, in the
repo whose thesis is that unreviewed artifacts should never be mistaken for evidence.

## D27 — a merge silently reverted a gate's own configuration, and the gate stayed green

The `FLEET_MUTANTS=1` opt-in added in `D25` was removed by the `D26` merge — `verify.sh` went from
16 lines to 7 at that hunk. **Nothing caught it, because the gate still passed.** It simply took 24
minutes instead of 90 seconds, and I only noticed because my own `verify.sh` call hit a 10-minute
timeout.

That is a new shape, and a nastier one than `D19`. `D19` was a gate that asserted nothing. This is a
gate that asserts everything it used to and is *unusable anyway* — and unusable gates get bypassed,
which is strictly worse than not having one. **Green is not the property. Runnable is also the
property**, and nothing measured it.

Detector `tests/corpus/M1.sh` added: fails if `FLEET_MUTANTS` disappears from `verify.sh`, and fails
if the opt-out path stops publishing a visible skip. Mutation-tested — green clean, red with the
token renamed, green restored.

**Eleventh and twelfth instrument failures, both mine, both in this stretch:**
- Splitting `$fns` unquoted in zsh, so three `grep` passes each searched for one impossible string
  and reported `0 surviving mutants` for all three lanes. zsh does not word-split. This is the
  *third* time this exact trap has produced a wrong number in this build.
- `ps -eo pid,args | grep '[v]erify.sh' | kill` killed my own shell (exit 144), because the shell's
  argv contains the script text being searched for. The `[v]` bracket trick defeats matching the
  grep, not matching the caller. Second time in this build.

Both were caught within one command. Neither reached a conclusion. The pattern holds: **the
instrument fails far more often than the product, and it fails in the same handful of ways.**

## D32 — the parity experiment ran, and its result is that it measures nothing

The full pre-registered run executed: 2 prompters × 8 tasks × 8 runs = **128 / 128 observations**
in 125s, margin pre-registered at 0.5 rubric points in a commit that precedes the data.

    score distribution: {4: 128}
    terse     n=64  mean=4.0000  min=4  max=4
    detailed  n=64  mean=4.0000  min=4  max=4

TOST **refused**: `"TOST is undefined; scores must contain non-zero variance"`, exit 7. That refusal
is the harness behaving correctly — with zero variance the standard error is zero and the test is
undefined; it declined to divide by zero and it declined to call a degenerate case equivalence.

**But the experiment is invalid as a test of requirement 5, and I am not going to score it as a
pass.** Two design faults, both mine:

1. **Ceiling effect.** Every run scored 4 of 4. A rubric that everything passes discriminates
   nothing — it is the "measuring nothing" failure this repo catalogues, wearing a lab coat. The
   four items (diff applies · artifact frozen · attestation verifies · receipt written) are
   properties of the *kernel*, which is deterministic, not of the *output*, which is what a junior
   and a senior would differ on.
2. **The agent does not read the prompt.** `stub` is deterministic by design (`D2`, so the
   acceptance suite stays reproducible). Prompter phrasing cannot influence a stub's output by
   construction, so "terse vs detailed made no difference" is true by definition and carries no
   information about junior-vs-senior parity.

Running it was still worth it: the ceiling was invisible in the synthetic controls, which used
`random.gauss` and therefore always had variance. **The controls validated the statistics and hid
the experiment design.** A positive control drawn from a distribution the real data cannot produce
is not a control for the real data.

What would make S4b valid, stated so the next attempt does not repeat this:
- a rubric with items that *some* correct runs fail — output-shaped, not kernel-shaped
  (e.g. does the diff handle the empty case, is the error typed, is the edge case named)
- a prompt-sensitive agent, i.e. a live model, which makes this a token-spending experiment
- a variance check as a precondition: refuse to *collect* into a rubric whose pilot shows no spread

Requirement 5 stays at **45%** — the harness and its refusals are real and proven; the experiment
that would justify the claim has not been run. Scoring it higher on the strength of a degenerate
result would be exactly the substitution this document exists to catch.

## D33 — S4b ran live: NOT-EQUIVALENT, and the honest reading is "not yet answerable"

`D32`'s two faults were fixed and the experiment ran against a real, prompt-sensitive model
(`--agent freelane`, keyless). The pilot precondition passed with genuine spread (N=12, min 0,
max 4), so unlike the previous attempt this run could have failed — and it did.

    N = 128            difference = 0.0781  (terse − detailed)
    90% CI = [−0.3579, 0.5141]              margin = ±0.5 (pre-registered, then re-justified
                                             for the 10-item rubric BEFORE the analysis ran)
    one-sided p: lower 0.0149 (rejects) · upper 0.0557 (does NOT reject at α = 0.05)
    VERDICT: NOT-EQUIVALENT

Published as-is. The pre-registration said a `NOT-EQUIVALENT` result would not be re-run with a
wider margin, a different rubric, or a different model, and it has not been.

**What this does NOT mean.** It is not evidence that juniors and seniors get different output. The
observed difference is **0.078 of a rubric point** — the terse prompter scored marginally *higher* —
which is nothing. Equivalence fails only because the confidence interval is too wide: its upper
limit, 0.514, pokes just past the 0.5 bound. This is an underpowered result wearing a
NOT-EQUIVALENT label, and reporting it as "seniors and juniors differ" would be a lie by omission.

**The threat to validity, which matters more than the verdict.** The score distribution is
`{0: 99, 3: 15, 4: 14}` — **99 of 128 observations scored zero.** The free lane is rate-limited and
frequently busy, so most runs produced nothing scoreable. This experiment is therefore measuring
*free-model availability* far more than *prompter parity*, and the variance that widened the CI is
mostly service flakiness, not prompter effect. A 77%-zero-inflated outcome cannot answer the
question no matter how correct the statistics on top of it are.

That is a third distinct failure mode for this one requirement, and worth naming:
- attempt 1 — no variance at all (deterministic stub, ceiling rubric) → the test was undefined
- attempt 2 — variance dominated by a nuisance factor (service availability) → the test ran and
  cannot be interpreted

**Requirement 5 stays at 45%.** The harness, the pre-registration discipline, the pilot
precondition and the refusals are all real and proven. The claim itself is still unproven, and the
next attempt needs a *reliable* agent — which means spending real quota, not the free lane. Stating
what would settle it: the same 128 runs on a paid model with a zero-rate below ~10%, and if the CI
still straddles the bound, a larger N rather than a larger margin.

## D35 — fleet used neither codex nor sonnet, and my first diagnosis of why was wrong

**The finding, corrected.** `fleet run` accepted only `stub | env-probe | freelane`. The allow-list
at `main.rs:420` refused `claude` and `codex` outright, so the documented entry point could not
drive either model — the answer to "is fleet only using codex, not sonnet?" is that it was using
**neither**. Fixed: `run` now accepts `claude | codex`.

**What I got wrong, and it matters.** I first reported that `agent_command` had *no dispatch arm at
all* and that the entire model-adapter path was dead code. That was false. `grep 'Some("agent")'`
returned nothing, and I concluded from one absent string. There is a `Some("__agent")` arm, and
`main.rs:1416` shows why: the parent **re-invokes its own binary** with `__agent` to spawn the fd-3
supervised worker. It is internal by design and correctly absent from `--help`.

Worse, I acted on the wrong diagnosis before checking it — I added a public `Some("agent")` arm,
which would have exposed the worker-spawn entry point and let a caller bypass the supervisor that
is the entire point of the launcher. Removed. **The absence of one grep hit is not the absence of a
caller**, and a fix applied before the diagnosis is confirmed can be worse than the defect.

**Detector `M4`.** The module-level reachability test cannot see this class: `main.rs` is reachable,
so any function inside it passes. `M4` checks per entry point — every `*_command` function must have
a dispatch arm (internal ones count; `--help` visibility is a separate question). Currently 8 of 8.

**Still open, recorded rather than hidden:** with the allow-list widened, `run --agent codex` reaches
the adapter bridge and fails with exit 6 and *no message*, even though both `claude` and `codex` are
on PATH. The missing-CLI case now names the binary and suggests `--agent freelane`; the deeper
bridge failure does not explain itself yet. `--agent codex` is therefore accepted but not working.

## D34 recurrence — the detector earned itself within the hour

`M3` fired on a real recurrence: the `l-multi` lane's artifact baked
`.worktrees/multi/keel/fleet` into the binary, `swarm status` began exiting 6, and swarm fell
20/20 → 17/20 **with a green build**. Diagnosed in one command by the detector rather than by
another hour of bisection. `bin/merge-lane.sh` now runs `cargo clean -p fleet` on every merge, so
the invalidation is structural instead of remembered.

## D36 — the swarm runs, and produces no evidence

Found by **using fleet as a user**, not by reading its tests. `fleet swarm dispatch` now genuinely
works in the sense that matters least: five agents advance into five different lifecycle states
(`lead→Specified`, `builder→Built`, `verifier→Verified`, `designer→Reviewed`, `meter→Observed`) and
every scorecard fills in `{credited:1, checked:1, total:1}`.

Then compare what each path leaves behind:

    fleet run            ledger/chain.jsonl  attestations/  artifacts/  tasks/  runs/
                         `ledger verify` → rc 0
    fleet swarm dispatch lifecycle/*.state   agents/*.json  artifacts/
                         `ledger verify` → rc 6 · `ledger count` → empty
                         `attest verify <id>` → rc 8 (mismatch)

**The swarm path bypasses the evidence kernel entirely.** Scorecards credit five agents with work
for which there is no ledger row and no verifying attestation. A scorecard that says `credited:1`
with nothing to justify it is the precise substitution this document exists to catch — and it is
worse than an empty scorecard, because it looks like progress.

Contract written before any fix: `X1`–`X4` in `tests/acceptance/swarm.sh`. Currently `X2`, `X3`,
`X4` fail. 20/20 → 21/24.

**One claim of mine in this finding was wrong, and I checked it rather than shipping it.** I first
reported the dispatch artifact id was a canned constant, because two different tasks produced
`0094d223…` both times. They did — but my probe created a *fresh identical repo* for each task, and
the deterministic stub emits the same diff, so identical content correctly hashes to an identical
id. Re-run against one repo, the ids differ (`0094d223…` vs `63ebe799…`). Content-addressing was
right; my experiment was not. `X1` passes and stays in the suite as a regression guard.

That is the second time in two findings that my first diagnosis was wrong (`D35` was the other), and
both were caught by testing the claim instead of acting on it. The failure mode is consistent:
**one surprising observation, one confident conclusion, no control.**

## D36 closed, and the user-session punch list that closing it produced

`X2`/`X3`/`X4` are green. Driving the CLI by hand after the fix:

    fleet swarm dispatch --task "add a --version flag" --repo T
      ledger verify                      rc=0
      ledger count                       6
      attest verify <dispatched id>      rc=0
      ratchet show                       dispatch_acceptance mark=1 set_at=... set_by_change=...
      agents list                        credited:1, faulted:0   (and only WITH an attestation)
    swarm.sh  20/20 -> 24/24

The swarm now goes through the evidence kernel instead of around it, and a scorecard can only
record `credited` when an attestation verifies for that task.

**Still open, found the same way — by using it, not by reading tests:**

1. **`ratchet check` cannot be invoked by a user.** `ratchet show` works, but `check` needs
   `--metrics k=v --change ID` *and* an undocumented scorecard file, and fails with
   `unable to read scorecard` (exit 3) saying neither where the file belongs nor how to create one.
   Discovering the flags took four attempts (`--metric` → `--metrics` → `+ --change` → scorecard).
   Same class as `D21`: an honest exit code with no actionable message. The rising bar is recorded
   but a human cannot yet check against it.
2. **`--agent codex` is accepted and does not work** (`D35`) — reaches the bridge, exits 6, silent,
   with both CLIs on PATH.
3. **`fleet meter show` refuses after a dispatch** — the meter is not wired to the dispatch path, so
   tokenomics still records nothing from real work.
4. **Skills are declared strings.** `agents list` prints `skills=["rust","debugging"]` but there is
   no registry and nothing dispatches on them. `mcp.rs` is reachable (`fleet mcp manifest <lease>`)
   yet no run path invokes a tool through it.

Each is an incremental feature on a working base, which is the right shape now: `fleet run` and
`fleet swarm dispatch` both produce verifiable evidence end to end.

## D37 — the re-run: zero rate 77% → 40%, still uninterpretable, and the pilot lied

Three lanes with failover, everything else held fixed (margin ±0.5, α 0.05, N 128, TOST).

    N=128   zero rate 51/128 = 40%   (D33: 99/128 = 77%)
    distribution {0: 51, 3: 44, 4: 29, 5: 4}
    terse    n=64 mean 2.3281        detailed n=64 mean 1.8594
    difference 0.4688   90% CI [-0.0479, 0.9854]   margin ±0.5
    one-sided p: lower 0.00117 · upper 0.46016     VERDICT: NOT-EQUIVALENT

**The second amendment declared in advance that a zero rate above ~30% makes the result
uninterpretable whatever the verdict.** It is 40%. So this is reported as uninterpretable, and the
`NOT-EQUIVALENT` label is not a finding about prompters. Publishing the verdict without the zero
rate beside it would be the lie the amendment existed to prevent.

**The new failure mode, and it is the reason to write this down.** The pilot precondition passed —
`N=12 min=0 max=5` — and the interim zero rate at N=60 was **15%**, comfortably under the threshold.
By N=128 it was **40%**. The lanes degrade under sustained load: rate limits accumulate over a run,
so early observations are cheap and late ones are not.

**A pilot drawn from the front of a run does not predict the run.** That is a third distinct way
this one requirement has failed to be measurable:

- attempt 1 (`D32`) — no variance at all → the test was undefined
- attempt 2 (`D33`) — variance dominated by a nuisance factor → uninterpretable
- attempt 3 (`D37`) — the *precondition* that was supposed to catch attempt 2 passed on an
  unrepresentative sample → uninterpretable again, and the guard did not fire

The guard needs to be a *rate over the whole run*, not a sample at the start: abort and report when
the cumulative zero rate crosses the threshold mid-collection, rather than discovering it in the
analysis. Recorded as the fix; not implemented in this pass.

Also worth noting against the temptation to read the trend: the difference grew from 0.078 to
0.4688 between runs, with `terse` scoring *higher* both times. With 40% zeros that difference is
most plausibly which prompter happened to draw a working lane, not a prompter effect. I am not
going to narrate it as "juniors do better".

**Requirement 5 stays at 45%.** Three attempts, three uninterpretable results, and each one taught
something real about measurement. The claim itself remains unproven and the honest next step is a
paid, reliable agent — not another free-lane run.

## D38 — 25 copies of one scanner, and the self-test that made removing them safe

`A8` fired on a comment of mine whose percentages had no denominators — the **fourth** detector to
need comment-skipping after `T6`, `B10` and `T20`. Four separate detectors learning the same lesson
is a signal about structure, not four bugs, so I measured instead of patching again:

    25 detectors carried a private copy of the walk + prune list
     4 carried the comment-skip

**21 latent instances of the same false positive**, and 25 copies of one function in a repo whose
stated rule is least self-written code. Extracted to `tests/corpus/_scan.py`: 0 private copies, 26
detectors on the shared scanner.

**The risk was neutering the safety net, so the brief demanded proof and I checked it myself:**

    bash tests/corpus/_selftest.sh   ->  25 of 25 detectors proven to still fire

Then I checked whether that self-test could be trusted, by neutering `A8` to `exit 0`:

    FAIL A8: planted=0 removed=0     ->  24 of 25

It catches a dead detector. That property — not the migration's diff — is what made the change safe
to accept. A migration report claiming "all tests pass" over a suite that no longer tests anything
is the failure this whole document is about.

**Two more corrections of mine in this pass.** The merge conflicted only in `MANIFEST.sha256`, a
*derived* file where both sides held a different `A8` hash; I regenerated it from the files on disk
rather than choosing a side, which is the only correct resolution for generated content. And my
first A8 probe (`"coverage is 87%"`) reported the detector broken — it requires the words
`percent|percentage|rate` nearby, so the probe never matched. Seventh instrument error of the build,
caught in one command. The pattern is stable and worth stating plainly: **my probes fail far more
often than the product does.**

`M2` then earned itself by refusing the tree while the merged worktree was still on disk — 17,066
files, over its 15,000 threshold. Removing it: 13,591 files, 594M, and the corpus back to 6s.

## D39 — the planning gate (reqs 6, 7, 14), and two ways a bypass can neuter its own tests

`fleet run` and `swarm dispatch` now REFUSE a task with no accepted SOW, naming both the id and the
command to create it. That is what turns reqs 6/7/14 from described into enforced: before this, both
ran immediately with no plan and no human gate.

    run, no SOW      rc=7  "refusing execution: task has no accepted SOW"
                           "SOW id to accept: 9e97d99b…"  "Create it with: fleet sow --task …"
    vague task       rc=7  CLARIFYING QUESTIONS — it asks rather than guessing
    complete SOW     rc=9  SOW_READY_AWAITING_REVIEW, with ESTIMATE, alternatives, edge cases
    sow accept       rc=0
    run, accepted    rc=0

**I nearly rejected this as a gate that refuses everything.** Three of my SOWs were rejected in a
row — uncited challenge, then an unnamed alternative, then only one alternative — and I concluded
the generator could never satisfy its own validator. It could: each refusal named exactly what was
missing and walked me to a valid SOW in three steps. Eighth instrument error of the build, and the
same shape as the others: **a confident conclusion from three malformed inputs and no control.**

**Two bypass defects, both mine, and the second is the instructive one.**

The lane left `p0.sh` at 24/5 and called it "the unavoidable repository-rule conflict". It was not
unavoidable — the brief said to wire `FLEET_SOW_BYPASS=1` into the suite setup so p0 keeps testing
the evidence kernel rather than the planning gate. One line.

Then I made the worse version of the same mistake: I set that bypass **globally** at the top of
`swarm.sh`, and my first draft of the `S5`–`S9` block inherited it. `S6` ("run before acceptance is
refused") failed with `rc=0` and caught it — but `S8` ("run proceeds after acceptance") had *passed
for the wrong reason*, on the bypass rather than on the acceptance. **A global test bypass silently
neuters the assertions that test the thing being bypassed**, and only the negative assertion
notices. Every `S` invocation now runs under `env -u FLEET_SOW_BYPASS`.

The bypass is recorded in the receipt, so a run without a plan stays auditable rather than
indistinguishable from one with a plan.

p0 34/34 · swarm 36/36 → 41/41.

## D40 — "an honest exit code with no next step" happened three times, so it became an assertion

`D21` (unset `FLEET_STATE`), `P1`/`P2` (ratchet's missing scorecard) and now the meter's unset
windows were all the same defect: a correct exit code and a message a user cannot act on. Three
instances is a class, not three bugs, so `Q1` asserts the property across every refusal surface
instead of waiting for a fourth patch. The meter now names the env var, the command that records
usage, and restates that an unknown window is `null` and never `0`.

**`Q1` was vacuous twice before it worked, and both failures are worth recording.**

*First:* the actionability test accepted `*/*` — any output containing a slash. Nearly every error
message contains a path, so the assertion could barely fail. Tightened to require a literal command
(`fleet …`) or an env var.

*Second, and worse:* `$FLEET` was unquoted. The repo path contains a space, so `env` failed before
fleet ever ran — and the old `*/*` clause happily matched **`env`'s own error message**. The
assertion was passing on its own breakage, across all eight surfaces, testing nothing. Quoted.

Then it immediately earned itself: `fleet swarm dispatch` with no arguments printed **nothing** and
exited non-zero. Now prints usage with an example. And it caught an error of mine in the other
direction — `skills --check` exits 0 when everything resolves, and a *successful* command owes no
next step; counting it as a refusal was my list's fault, so `Q1` now only tests non-zero exits and
adjusts its denominator accordingly (7, not 8).

Mutation-tested properly on the third try. My first mutation did not compile, so it silently
verified the *unmutated* binary and reported green — the same shape as everything else in this
section. With a compiling mutant (`eprintln!("bad arguments")`):

    not actionable: swarm dispatch -> bad arguments
    FAIL Q1 all refusal surfaces are actionable   1 of 7 give no next step

Restored: 42/42.

**H1 then caught Q1 itself.** The assertion looped with an unquoted `$qc` to word-split into
arguments — exactly the trap that produced several false readings earlier in this build, and H1
flagged it at `swarm.sh:250`. Rewritten to use bash arrays expanded as `"${qcmd[@]}"`, which
removes the hazard rather than exempting it. **A detector that has to be told "this instance is
fine" stops being a detector.**

## D41 — the planner walked the user into a refusal it knew about

Found by running a whole session end to end rather than testing commands one at a time:

    $ fleet plan "add a --version flag"
      commands: 1 planned  ->  1. fleet swarm dispatch
    $ fleet swarm dispatch --task "add a --version flag" --repo T
      fleet: refusing execution: task has no accepted SOW.

Both halves are individually correct and the combination is useless. The planner named the one
command guaranteed to fail, because `D39` put a gate in front of `dispatch` and nothing told the
intent table. **A plan that omits a gate the user must pass through is not a plan** — it is a
sentence that happens to name a command.

`Intent::Change` now plans `sow` → `sow accept` → `swarm dispatch`, three commands with a published
denominator. `N8` asserts the plan names the SOW step, so the two halves cannot drift apart again.

The committed-example unit test caught the change and failed, which is the contract working: the
intent table is a committed mapping, so I updated the example deliberately with the reason rather
than letting the test follow the code silently.

**This is the class of defect only end-to-end use finds.** Every individual assertion was green:
`N1` proved the plan routed, `S6` proved the gate refused. Nothing tested that the plan's output was
*runnable*, because each was tested against its own contract and neither owned the seam.

## D42 — the refusal named the deficiency but not the shape of an answer

`fleet sow --task "add a --version flag"` refused with *"challenge register has no evidence
citation"*. Accurate, and useless: the structured section format it wants was discoverable only by
reading `crew/crew/sow.py`. I found it that way; a user cannot.

The clarifying question now prints a complete worked example — every section, with a real
`file:line` citation and two named alternatives — and states the two rules that trip people
(citations must be real, one option is not a choice).

**`S10` asserts the printed example actually works**, by extracting it out of the refusal and
feeding it straight back in. An example that does not work is worse than no example: it costs a
round trip and the reader's trust in the next one.

Two of my own bugs on the way, both in the extraction rather than the product:
- BSD `sed` has no `\|` alternation in a basic regex, so my first extraction silently dropped every
  section header and kept only the `- ` lines. It looked plausible and was missing half its input.
  Rewritten in `awk`. Same portability class as `T6`'s `stat -f`.
- `S10` failed first with `rc=7` and I nearly recorded the example as broken — I had already
  verified by hand that the same text gives `rc=9`, so the instrument was the suspect, and it was.

## D43 — reqs 12 and 15: a log a human can read, and an undo that keeps the evidence

**`fleet status`** (req 12 — *"done, pending, failed, needs iterations … I should be able to
understand the logs easily"*). Before this, `ledger dump` emitted raw JSONL and `status`, `summary`
and `report` were all unknown commands: the data existed and nothing presented it.

    FLEET STATUS — 2 of 2 tasks from receipts
    ┌ DONE — 1 of 2            t1 good   stub   attest yes   completed; attestation verified
    ┌ PENDING — 0 of 2         (no tasks)
    ┌ FAILED — 1 of 2                    —      —            refused: EMPTY_TASK
    ┌ NEEDS-ITERATION — 0 of 2

Every bucket carries a denominator and every row derives from a receipt. NEEDS-ITERATION is
*derived* — a run that completed but whose attestation does not verify — rather than a field
someone has to remember to set, because a status field that must be maintained is one that goes
stale.

**`fleet rollback --artifact <ID>`** (req 15's missing half). Verified in all four directions:

    first rollback      rc=0   rolled_back artifact=0094d223…
    second rollback     rc=7   refused
    unapplied artifact  rc=7   refused
    attest verify       rc=0   the attestation SURVIVES the undo
    repo               clean   0 dirty files

The artifact and attestation deliberately survive: an append-only ledger that deletes the record of
what was attempted is not append-only. Automatic retry is explicitly out of scope — healing means
restoring a known state, not silently trying again.

**`Q1` earned itself again.** Adding `rollback` and a bad `status` flag to its surface list turned it
red: `2 of 9 give no next step`. Both refused correctly, wrote receipts, and printed **nothing** — a
receipt is for the ledger, a human needs the next step. Both now print usage; `rollback`'s names
`fleet status` as the way to find an id.

**And my probe was wrong again, in the usual way.** I ran rollback twice — once piped to `head`,
once to capture the code — and reported the *second* call's `rc=7` as the first's, nearly filing a
working rollback as broken. Re-measured with each call captured once: `rc=0` then `rc=7`.

## D44 — two tools listed ADOPTED that nothing calls, and a ledger that had drifted again

Re-auditing `CONFORMANCE.md` (the second time it has needed this) found **two entries stale in the
flattering direction** and one real gap hiding behind them.

Stale and now closed: item 1 claimed `oracle_independence` was "real but not adversarially tested".
It is tested — four fixtures, `cargo test -- adversarial` reports **4 passed**, and they were
mutation-tested by forcing the discriminator to always ACCEPT (3 of 4 go red; the fourth *is* the
ACCEPT case). Item 4 lumped `witness`/`conftest`/`rekor` together as unadopted; `witness` signs,
verifies **and rejects a tampered artifact**, and `conftest` gates 3 policies in both directions.

**The real finding underneath.** `opa` and `rekor-cli` were both listed **ADOPTED** and are invoked
by nothing — they appear only in `ADOPTION.md` and `CONFORMANCE.md`. `conftest` embeds OPA's engine,
which is *conftest* working, not `opa`. This is `PRINCIPLES` #1 verbatim: two tools adopted and
defended for a day without either having run. Both rows corrected to `UNADOPTED` with the reason.

**Detector `M5`** now fails when a tool marked ADOPTED has no caller in `bin/`, `policy/`,
`keel/fleet/src/` or `verify.sh` — documents explicitly do not count as callers. It found both on
its first run. Its first version matched no rows at all and **refused rather than passing
vacuously** ("no ADOPTED tools found to check — measuring nothing is a failure"), which is the only
reason I noticed the pattern was wrong instead of shipping a green detector that checked nothing.

Mutation-tested both ways: with a real caller planted, `rc=0`; with the claim restored and no
caller, `rc=1` naming the tool. Corpus now 32 checked, 0 caught.

**The lesson is about the ledger, not the tools.** Both stale entries understated the build and one
overstated it, and none of it was visible without re-reading the file against the tree. An honesty
ledger is only honest at the moment it is written; it needs its own audit, and now it has a detector
for the half that can be mechanised.

## D45 — req 3: the router `ROUTING.md` specified four hours before anything executed it

`fleet route` did not exist. The six-stage design had been researched, every ML router disqualified
on the keyless constraint, and the accepted scheme written down — and nothing ran it. Requirement 3
sat at 55% for exactly that reason: **the design was done and the execution was not.**

Now, and it shows its work at every stage:

    $ fleet route --role builder            (no quota configured)
    stage 1: 2 of 4 remain [codex, sonnet] — explicit role
    stage 2: 2 of 4 remain [codex, sonnet] — safety policy
    stage 3: 2 of 4 remain [codex, sonnet] — local capability
    stage 4: 0 of 4 remain []              — availability/quota
    REFUSE at stage 4 (availability/quota): all eligible lanes are cooling down…

Verified in both directions on every stage that has two:

| stage | refuses | accepts |
|---|---|---|
| 2 safety | `lead` + `implementation` → `REFUSE … LEAD_WROTE_CODE. Fix: --role builder` | `lead` + `general` → `adapter=claude resolved_model=opus` |
| 2 safety | `builder` + `human-only` → rc 7 | |
| 4 quota | no windows → refuse naming the stage | windows set → `stage 6: 1 of 4 [codex]`, rc 0 |
| 5 independence | `--builder-model codex` → verifier set drops codex, `[sonnet]` | distinct models survive |

**No fallback anywhere.** When the set empties, it refuses and names the stage and the fix. A silent
default is how you stop knowing which model ran — the same defect class as `resolved_model` copying
the request instead of reading the response, which `crew/tests/test_adapters.py` exists to catch.

Wired into the planner, so the plan now answers the owner's question in full — *what commands, which
skills, which agent, which model, and why*:

    routed lane: codex (resolved model: codex; decided at stage 6)

Two probe errors of mine, both caught in one command: I guessed `--task-writes-code` (the flag is
`--task-class`) and `planning` (the classes are `general`, `human-only`, `implementation`). Neither
reached a conclusion.

p0 34/34 · swarm 44/44 · clippy clean.

**And the router merge broke the planner, which the N assertions caught.** Wiring routing into
`fleet plan` made it REFUSE whenever no lane could be picked — so a user with no quota configured
could not even see a plan. `N1`, `N3`, `N4` and `N8` all went red at once, which is what a suite is
for: four assertions failing together pointed at one seam, not four bugs.

**A plan is not an execution.** Planning and running are different questions and the planner had
started answering the wrong one. It now reports routing as part of the plan and continues:

    routed lane: UNAVAILABLE — route emptied at stage 4 (availability/quota): …
      fix before running: wait for cooldown/reset or configure a measured window
    commands: 3 planned (denominator: 3)

The gate that must refuse is `run`, and it still does. Also: applying clippy's
`unwrap_or_else(Vec::new)` → `unwrap_or_default()` suggestion blindly broke the build (12 errors) —
the closure's `.collect()` had been inferring its type from `Vec::new`, so it needed an explicit
turbofish. A lint suggestion is a hypothesis about the code, not a patch.

## D46 — the token counts were there all along, and one root cause was showing as two gaps

Chasing why `meter plan` refused, rather than the refusal itself, found that
`https://api.llm7.io` returns `{"prompt_tokens": 5, "completion_tokens": 15, "total_tokens": 20}`
on **every** call and `bin/freelane.sh` was throwing it away. The meter recorded `null` for a lane
that had genuinely been measured — not the meter being wrong, the meter being lied to by omission.

Wired end to end. Both directions, which is the only reason this counts:

    --agent freelane   freelane  310  199690  checked=2 total=2  resolved_model=codestral-latest
    --agent stub       stub      null null    checked=1 total=2  resolved_model=null

**`null` and `0` stay different facts.** `stub` ran and produced nothing measurable → `null`.
`freelane` on a state where it never ran → `0` consumed of a declared window. "We tried and could
not measure" is not "nothing was consumed", and `crew/tests/test_adapters.py` exists because
conflating them once nearly shipped a forged provenance field. The resolved model is recorded
beside the usage, read back from the response and never from the request.

**One root cause was presenting as two separate gaps.** Requirement 5 (parity, 45%) and
requirement 10's earlier refusal both trace to the same thing: *a deterministic stub produces no
variance and consumes no tokens*. Neither the experiment nor the quota planner can get data from
it. That is one fact wearing two masks, and it is why `--agent freelane` mattered more than either
feature looked like it did.

`meter plan` still refuses after a stub dispatch — `measured_task_tokens=null` — and that is
correct. It will not project from unmeasured data. It works the moment there is a real measurement.

## D47 — the agent froze an attested change that implemented nothing, and the model was never at fault

Every structural check passed. The diff was non-empty, applied cleanly, the artifact froze `0444`,
the attestation verified, the oracle returned `ACCEPT`. Ninety-seven added lines, **zero of them
executable** — the freelane agent was commenting out *every line* of the model's reply, including
the fenced code block, and freezing the wrapper.

    before   97 added, 0 executable   ← 100% commented-out prose
    after    47 added, 33 executable

**This is this repository's own thesis, live and self-inflicted.** The freeze/attest/ledger kernel
worked perfectly over a change that did nothing. *A non-empty diff* is a proxy; *a diff that
implements something* is the property, and the kernel could not tell them apart.

**The model was never the problem.** It returned working Rust in a fenced block on every call. The
comment above the offending code even claimed it "freezes a real, prompt-sensitive Git diff rather
than fabricating a successful no-op" — technically true and missing the point. It is worse than a
no-op: a no-op is visibly nothing, this looked like work.

**Two wrong guards before the right fix, both worth keeping.**
1. I filtered `//`-prefixed lines out of the model's RAW reply — but the reply is prose and the `//`
   is added *afterwards*, so every prose line counted as code and the guard never fired. **It tested
   the text after the wrong transformation.**
2. Then I refused when no fenced block was present. It never fired either — a fence was always
   there. That non-firing was the information: the code existed and was being discarded, so a
   refusal was the wrong shape entirely.

The fix is extraction, not refusal: take the fenced code, keep the prose as a comment *beside* it.

**It re-explains all four parity attempts.** `D32` (no variance), `D33` (77% zeros), `D37` (40%
zeros) and today's pilot (all 12 scored 0) — the output-shaped rubric was scoring zero **correctly**,
because there was no implementation to score. The rubric was right the entire time and I spent three
findings blaming the measurement.

## D48 — the zero-inflation was never zeros: `score=0` was the default for a run that never happened

`D33` reported 77% zeros and `D37` 40%, and both blamed the free lane's reliability. Both were
wrong about *what* the zeros were. In `bin/parity-run.sh`:

    score=0                       # <- the DEFAULT
    ...
    if [ -n "$artifact_id" ]; then score=$(score_observation ...); fi

A run that **failed** never reached the scorer, so it was recorded as a genuine `score: 0`. Every
rate-limited, timed-out or refused invocation entered the dataset as a real observation of a
worthless diff. The experiment was averaging over runs that never happened.

Proven, not inferred: scoring a real freelane artifact by hand with the harness's own scorer gives
**5 of 10**, while the pilot recorded 12 consecutive zeros. Successful runs score ~5; the zeros were
absences wearing the same value.

Fixed: a failed run is now SKIPPED and counted separately —
`parity: run failed (rc=7) — observation SKIPPED, not scored 0` — and the run refuses (exit 6) if
everything was skipped, because measuring nothing is a failure. **Same null-is-not-zero rule as the
meter**, and this is the third place in the build where conflating them produced a confident wrong
number.

Two of my own bugs fixing it: `skipped` was declared beside the other counters *after* the pilot
phase that increments it, so `set -u` turned the first skipped run into an unbound-variable abort
instead of the message. Moved above the pilot.

**Still open, and now honestly visible instead of hidden in fake zeros:** repeat `--agent freelane`
runs against an already-modified repo refuse with `rc=7` and print **no reason** — the `D40`/`Q1`
class on a surface `Q1` does not cover. The refusal is probably correct (an empty diff against a
dirty tree) but it must say so. Requirement 5 stays at 45%: the harness now reports honestly that
it cannot collect, which is strictly better than reporting a number it invented.

## D49 — the D40 class was still hiding on the busiest command in the tool

`Q1` covered nine refusal surfaces and `run` was not one of them. Extending the list found three
silent refusals on the command every user touches first:

- **`TARGET_REPO_NOT_CLEAN`** — the commonest refusal in practice, because the *previous* run leaves
  the tree modified. It wrote a receipt and printed nothing, so a second run looked like a silent
  crash. It now explains that fleet freezes against a known starting point, that a dirty tree would
  put work fleet did not do inside the attestation, and gives the two `git -C <repo>` commands.
- **`--task` with no value** — the likeliest typo in the tool, exited 7 in silence.
- **an unknown flag** — same.

All three now print usage from one shared `RUN_USAGE` const.

**Two of my own errors placing the fix**, both the same shape: I patched a path that was not the one
executing. First I added the message to `NO_WORK_LANDED` (exit 6) when the actual refusal was
`TARGET_REPO_NOT_CLEAN` (exit 7) — I had assumed the reason instead of reading it, and the ledger
told me the truth in one command (`"reason":"TARGET_REPO_NOT_CLEAN"`). Then the message landed
*before* the `if` that guards the case rather than inside it, so it compiled and never printed.

**The pattern worth keeping:** `Q1` only tests the surfaces in its list, so the class it was built to
eliminate survived for eight findings on the one command nobody thought to add. A property assertion
is only as good as its enumeration, and the enumeration needs the same scrutiny as the property.
`Q1` now covers 11 surfaces and publishes the count.

## D50 — the router could never select the only lane that was actually up

Found by running a whole session rather than a command: `fleet plan "add a --version flag"` reported
`routed lane: UNAVAILABLE — route emptied at stage 4` while `freelane` was up, funded, and had just
recorded 310 real tokens through the meter. The one working lane sat outside the router's table.

Two things were true and neither was visible from either side alone:

1. **`freelane` was absent from `ORDER`.** The router's universe was `{codex, sonnet, claude, opus}`
   — the authenticated CLIs only. Added, ordered **last**, because it is the keyless fallback: it
   should win when the authenticated lanes are unavailable and never over them.
2. **The capability probe only looked on `PATH`.** `freelane` is `bin/freelane.sh` inside the repo,
   so stage 3 dropped it every time. Stage 3 was correct by its own rule; the rule did not know
   repo-local adapters exist.

Preference verified in all three directions:

    codex funded too    -> adapter=codex        (authenticated wins)
    only freelane       -> adapter=freelane     (keyless fallback)
    codex in cooldown   -> adapter=freelane     (fallback on cooldown, not just on absence)

**And a claim of mine that shrank on contact.** I first wrote that `sonnet` and `opus` were
"routable but not runnable" — a vocabulary mismatch between the router and `--agent`. Wrong:
`sonnet` is a *model* served by the `claude` *adapter*, which is why stage 3 keeps it when the
`claude` CLI is present and drops it when PATH is bare (verified: `2 of 4` → `0 of 4`). The router
speaks models, `--agent` speaks adapters, and both are right. Only the `freelane` half was real.

That is the fifth time this build a confident first diagnosis shrank or inverted under one probe.
The probe cost one command every time.

## D51 — the SOW gate required an input the lifecycle could not store

The complete flow — plan, sow, accept, dispatch against a real model — died at the last step with:

    fleet: environment fault: LIFECYCLE_IO: File name too long (os error 63)

`safe_task_name()` sanitised the task text into a filename but never **bounded** it. A structured
SOW (leaves, challenges, alternatives, estimates, edge cases) runs to hundreds of characters, so
the state file name grew with the task and blew past the filesystem limit.

**The two features were individually correct and jointly impossible.** `D39` made `run` and
`swarm dispatch` REQUIRE a structured SOW; the lifecycle could only store short task names. Each
had tests, both passed, and the product could not complete a single realistic task. Only running
the whole sequence surfaced it — the same shape as `D41`, where the planner named a command the
gate would refuse.

Fixed with a bounded name: a 48-character readable prefix plus 16 hex of a BLAKE3 digest of the
FULL id, so two long tasks sharing a prefix cannot collide.

The flow now completes:

    plan            routed lane: freelane (resolved model: codestral-latest; stage 6)
    sow             rc 9 — awaiting review
    sow accept      rc 0
    swarm dispatch  artifact=24b7592…  agent=freelane  checked=5 total=5
    ledger verify   rc 0
    meter show      freelane  510 used  199490 remaining  resolved_model=codestral-latest

English in, routed to a real keyless model, gated on a human-accepted plan, frozen, attested, and
metered — with every number traced to a receipt.

## D52 — the router refused an ordinary task, and widening it broke the refusal

Testing a *different* task on a *different* language — `handle division by zero in the calculator`
against a Python repo — the intent table refused it. The `Change` verb list held ten words and
missed most of the language people use for implementation work: handle, support, validate, delete,
extract, guard, migrate, replace, wire.

Broadened from real phrasings. **And it immediately broke the thing that mattered more.** I had
added `"make "`, which matched `make me a sandwich` — the canonical control for "an unrecognised
prompt must refuse rather than guess". `N5` and `N6` went red in the same run.

`"make "` is now deliberately absent, with the reason in the source. **A table that must never
guess should lose recall rather than gain a false positive**; `make the parser stricter` is
expressible as `change the parser to be stricter`, and no phrasing of `make me a sandwich` should
route to a builder.

    handle division by zero        rc 0      make me a sandwich   rc 7
    support UTF-8 filenames        rc 0
    validate the config on load    rc 0
    delete the dead retry path     rc 0

`N9` locks the four real phrasings; `N5`/`N6` still lock the refusal. Both directions, in one
suite, so widening the table again cannot quietly cost the guard.

**This is the deterministic router's real tradeoff, stated honestly.** `ROUTING.md` chose a
committed table over an ML router for auditability, and this is the bill: the table only knows what
it has been told, and every widening is a chance to swallow something it should refuse. That is
still the right trade — a wrong route that *refuses* costs a rephrase, a wrong route that *proceeds*
spends tokens and lands a diff — but it is a cost, not a free win.

## D53 — the first error a user saw was never the real one

Probing the cases the spec never enumerated — empty, huge, unicode, no-git, missing — found that a
**non-existent repo** and a **directory that is not a git repo** both refused with:

    fleet: refusing execution: task has no accepted SOW.

Accurate in the sense that no SOW existed, and useless: writing a SOW cannot make `/nonexistent`
exist. The policy gate ran before basic input validation, so the advice a user received first was
advice that could not possibly help.

Cheap structural checks now precede the policy gate, on both the `run` and `swarm dispatch` paths:

    --repo /nonexistent   ->  "--repo /nonexistent does not exist (or is not a directory)."
    --repo <not a git>    ->  "is not a git repository … git -C <repo> init && git commit --allow-empty"

`E1`/`E2` lock the ordering. The other three probes behaved correctly already: an empty prompt
prints usage, a 3000-character prompt and a Spanish prompt both refuse with near matches rather
than guessing.

**One of my own results in that batch was a bad fixture, not a defect.** A "valid repo still runs"
check returned `rc=6`; the fixture had created a file named `f` and no `main.rs`, and the stub
appends to `main.rs`. A properly shaped repo registers an artifact normally. Checked before filing
it — sixth time this build that a suspected regression was my test setup.

**Ordering diagnostics is a real property, not politeness.** The error a user reads first should be
the one whose fix unblocks them; anything else sends them to do work that changes nothing.

## D54 — three concurrency races, stacked, found by running eight dispatches at once

The L8 rubric names *concurrent* among the cases a spec never enumerates. Eight simultaneous
`swarm dispatch` calls against a cold state directory found three races, each hidden behind the
one above it.

**1. TOCTOU on the agent store.** `if !agents_dir.exists() { seed() }` — one process created the
directory, another saw it exist, skipped seeding and loaded nothing: *"agent store contains zero
agents"*. Existence is not the invariant.

**2. Lost updates on the scorecard.** A read-modify-write of a per-agent JSON file with no lock.
Concurrent dispatches clobbered each other, and a reader catching a half-written file failed to
parse it: *"cannot record verified agent scorecard"*. Now serialised on a per-agent lock, and the
counts are consistent afterwards (`credited:7` across all agents for 7 successes).

**3. Partial reads of a seeding store.** Seeding writes five agent files one at a time, so a reader
could see one: *"a swarm requires at least two agents; found 1"*, and *"swarm has no designer
agent"*. Fixed by seeding into a staging directory and renaming it into place — a reader now sees
either no store or the complete one.

**And a fix of mine that only half-worked, caught by measuring instead of assuming.** After the
atomic-rename change the rate was still 4 in 32, because the *fast path* returned on `!is_empty()`
while only the *locked* path checked completeness. **Two checks for one invariant is one check too
many.** Both now call `is_complete()`, which compares against the registry's own agent count.

Measured, not claimed:

    before  1 in 4 failed          (first observation)
    after   3 of 48 = 6%           (six trials of eight concurrent dispatches)
    ledger  verify rc 0 in every trial — the hash chain never broke

**The ledger was right all along.** It has real locking and survived every trial; everything that
broke was state added later without it. And 6% is not zero — I am recording the residual rate rather
than calling this fixed, because the next reader deserves the number I actually measured.

**Correction to the fix above.** The completeness test broke two unit tests
(`over_capacity_is_refused`, `below_bar_names_submission_and_bar`): they deliberately construct a
**2-agent store**, and `is_complete()` decided that was a torn read and silently re-seeded it up to
five, so a refusal-on-over-capacity assertion started passing. **The assertions were right and my
fix was wrong.**

The resolution is that the check was redundant once seeding became atomic: a staging directory
renamed into place means a partial store *cannot be observed*, so "fewer agents than the registry"
is a deliberately small swarm and not a torn read. Trigger reverted to emptiness, atomic seeding
keeps the invariant, all 84 unit tests pass, and the measured concurrency rate is unchanged at
**3 of 48 (6%)** — the fix that mattered was the rename, not the extra test.

**Resolved: 0 of 128.** The residual rate would not fall because I had made the *locked* seeding
path atomic and left the *eager* one — `if !agents_dir.exists() { seed() }` — writing five files
one at a time into the live directory. That was the exact partial-read window the atomic path
existed to remove, sitting in the branch that runs first on every cold start.

Two more fixes were needed and one was a red herring, which is worth separating:

- **Every seeding route must be the atomic one.** Fixing only the path I was looking at left the
  common path broken; the rate barely moved (5 of 64 → 7 of 64, i.e. noise) and I nearly concluded
  the fix had failed rather than that it had not been applied everywhere.
- **`seed_atomically` must never destroy the live store.** It did `remove_dir_all` then `rename`,
  which is its own window. It now renames only into a free slot and keeps a winner's store if it
  loses the race.
- **Atomic per-agent writes** (`write_atomic`, temp + rename) removed `"invalid agent record"`,
  a genuinely separate defect on the *update* path — but it did **not** move the aggregate rate,
  because the seeding window dominated. A real fix that changes no number is still a real fix; it
  just was not the one that mattered here.

    measured: 1 in 4  ->  3 of 48 (6%)  ->  0 of 128, ledger verify rc 0 in every trial

**Guarded, and the guard is proven to fail.** `CC1`–`CC3` run six concurrent dispatches against a
**cold** store — the vulnerable moment, since every process races to seed — and assert all six
succeed, the ledger still verifies, and the store ends complete. Mutation-tested by putting the
non-atomic `seed()` back in the eager branch: `CC1` caught it. A regression test for a race is
worthless unless you have watched it fail on the original defect.

One naming fix while adding them: I labelled the new assertions `C1`–`C3`, colliding with the
existing compile-fail `C1`/`C2`. Two different checks under one id makes a failure report
ambiguous; renamed to `CC1`–`CC3`. An audit for other collisions found only `R1`, which is the
deliberate five-role loop (`R1:lead`, `R1:builder`, …). swarm.sh is now 50 assertions.

## D55 — the parity harness predated the SOW gate, and the skip had no reason attached

Attempt five at requirement 5. Two defects in the harness, one of them the exact class this
document keeps recording.

**1. The harness predated `D39`.** It called `fleet run` without `FLEET_SOW_BYPASS`, so every
observation was refused with "task has no accepted SOW" and — correctly, thanks to `D48` — skipped.
The experiment collected nothing and refused. **Two features individually correct and jointly
impossible**, for the third time (`D41`, `D51`, now this). The bypass is right here: S4b measures
whether two *prompters* produce the same output, not whether the planning gate works, and the gate
has its own assertions in `S5`–`S9`. It is recorded in every receipt, so these runs stay auditable.

**2. A skipped observation printed no reason.** `D48` made failures visible as skips rather than
fake zeros, which was the right fix — and then the skip said only `rc=6`. Anyone debugging the next
empty experiment hits a dead end. The `D40` class, inside the harness this time. It now prints the
underlying error, which immediately gave up the answer:

    parity: run failed (rc=6) — observation SKIPPED, not scored 0
      parity:   fleet: agent freelane exited without an fd-3 result…

**What the honest answer turns out to be.** Sequential runs work — four in a row, `rc=0` each, and
the two real prompter phrasings produce genuinely *different* artifacts (`49de112c` vs `d548bc2b`),
which is precisely the variance S4b needs. Twelve rapid pilot calls do not: the free lane
rate-limits and the adapter returns no fd-3 result.

So requirement 5 is not blocked by the harness any more, nor by zero-inflation, nor by the rubric,
nor by the agent discarding code. **It is blocked by call rate on a free endpoint** — and every
earlier explanation I gave for it (`D33` reliability, `D37` a degrading pilot, `D48` fake zeros,
`D47` no implementation) was a real defect stacked in front of this one. Each fix removed a layer
and revealed the next.

**Requirement 5 stays at 45%,** with the remaining obstacle now precisely stated and cheap to test:
a paced or paid agent, not another free-lane run. The harness reports honestly when it cannot
collect, which is the property that took five attempts to earn.

## D57 — attempt six: the harness is right, the lane is not, and that is now the honest answer

The paced pilot passed once — `N=7 min=4 max=6`, real spread — and the next identical pilot collected
**3 of 12** and refused against its floor of 6. Same pacing, same prompts, same endpoint. The free
lane's success rate is not merely rate-limited; it is **variable**, and 8s of pacing does not fix it.
Testing 20s pacing needs longer than a single command budget allows, so I have not established
whether more pacing would clear it — I am not going to claim it would.

**What is now true, and worth more than a verdict.** The harness reports its own inability precisely:

    pilot collected 3 of 12 scoreable observations; 6 is the floor
    parity: invariant violation: pilot collected too few scoreable observations to judge
    parity:   the skip reasons above say why. A rate-limited lane needs PARITY_PACE_S raised.

A refusal, the shortfall, the reason, and the knob to turn. Compare `D33`, where the same underlying
condition produced *"NOT-EQUIVALENT, 77% zeros"* — a number that looked like a finding and was an
artefact. **Six attempts converted a confidently wrong answer into a correctly stated inability**,
and every layer removed on the way was a genuine defect: the agent discarding code (`D47`), fake
zeros (`D48`), the missing SOW bypass (`D55`), a fixed-count pilot assertion (`D56`).

**Requirement 5 stays at 45%.** The claim is unproven and the obstacle is now a single sentence: the
only keyless lane cannot sustain 12 consecutive calls reliably. A paid lane with `PARITY_PACE_S=0`
settles it in twenty minutes; nothing in fleet needs to change first. That is the difference between
a blocked requirement and an unknown one.

## D58 — the README's 60-second quickstart did not run

The first thing any reviewer copy-pastes. It called `fleet run` directly, and `D39` had put the
planning gate in front of it, so it failed with *"task has no accepted SOW"*. **Nothing tested the
README, so nothing noticed.** Fourth instance of one class — a later feature breaking an earlier
flow that no assertion covered (`D41`, `D51`, `D55`, this) — and the worst-placed of the four,
because it is the entry point.

Rewritten to the real flow (`sow` → `sow accept` → `run` → `ledger verify` → `attest verify` →
`status`), which is more honest anyway: the planning gate is the product, not an obstacle to hide.
`FLEET_SOW_BYPASS=1` is documented beside it for scripts and CI.

**`tests/acceptance/readme.sh` extracts the shell block out of `README.md` and runs it.** It cannot
drift from the document because it *is* the document — a `fleet` shim on `PATH` makes it behave as
an installed binary would. Wired into `verify.sh` as a gate stage and mutation-tested: commenting
out the `sow accept` line turns it red.

Three of my own errors getting the quickstart to actually pass, each caught by the new test rather
than by reading:
- the challenge citation `main.rs:1` resolves against the **fleet repo root**, not the demo's cwd,
  so a temp-repo path can never validate. A corpus id (`A1`) is context-independent.
- `sow` prints its id on **stderr**, not in the JSON on stdout; my extraction read the wrong stream.
- the artifact id had to be anchored to the bare `^artifact=<64 hex>$` line rather than the
  progress lines above it.

Every one of those is a defect a reader would have hit on their first minute with fleet, and none
was visible from the source — only from running what the document says to run.

## D59 — the security document described two commands that do not exist

`D58` raised the obvious follow-up: if the README's commands were broken, what about every other
document? Sweeping every backticked `fleet …` in every file against the real binary found **20 of
23 existed**. The three missing were one deliberate example of an unknown command — and two claims
in `docs/SECURITY.md`:

> "fleet attest self" records an in-toto Statement for the running binary … "fleet sbom" emits
> a CycloneDX inventory from Cargo's locked dependency graph.

(Quoted, not backticked: by the convention `M6` enforces, backticks mean the command exists.)

**Neither command exists.** Not deprecated, not broken — never built. A security document is the
one surface a reviewer trusts without re-checking, so overstating there is worse than overstating
anywhere else, and nothing read the docs so it would have survived indefinitely.

Corrected in place with the reason rather than deleted, and what *is* true stated plainly:
`cargo-audit`, `cargo-deny` and `gitleaks` are required gate stages; supply-chain attestation of
the fleet binary itself remains unbuilt.

**Detector `M6`** now fails when a documented command does not exist, with a published denominator.
Mutation-tested: adding `` `fleet nonexistentcmd` `` to any document turns it red.

It also caught **my own correction**, which backticked `fleet sbom` while explaining that it does
not exist — the fifth time a check has fired on the documentation of the thing it checks
(`T6`, `B10`, `T20`, `A8`, now this). Resolved by convention rather than by exempting the file:
**backticks mean "this exists"**, so a withdrawn claim is quoted, not backticked. That is a better
rule than any suppression would have been, and `M6` enforces it.

**Two more of mine while building `M6`, both worth keeping.** The detector scans documents for
backticked commands, and my own `D59` write-up quotes the phantom ones — so it flagged itself.
Resolved by SCOPE rather than suppression: `M6` checks documents that describe **current
capability** (README, SECURITY, ADOPTION, ROUTING …) and excludes `DELTA.md`, which is a historical
record of defects and necessarily quotes broken things. Scoping a check to the documents whose
claims must be true is not the same as exempting a document that fails it.

Then `$DOCS` as a string split on the space in "Principal Engineering", so `grep` read nothing.
**`M6` refused — "found no documented commands to check — measuring nothing is a failure" — instead
of passing vacuously, and that refusal is the only reason the bug was visible.** Twelfth
path-or-word-splitting error of this build, and the rule that caught it is the same one that has
caught several others: never let a check report success on an empty measurement.

## D60 — the headline feature was invisible in `--help`

`M6` checks documented-but-missing. The inverse turned out to be worse: **five reachable commands
absent from `--help`** — `plan`, `meter`, `lifecycle`, `graph`, `completions`.

`fleet plan` is the natural-language front door, the thing the owner asked for in the words *"I
should be able to prompt text and fleet should be able to take care of the rest"*. It routes four
intents, names the agent, skills and lane, refuses an unmatched prompt with candidates, and has
nine assertions behind it — **and a user running `fleet --help` would never learn it exists.**

A feature nobody can find is not shipped. Every one of the five was added late, wired into
`dispatch()`, tested, documented in `DELTA.md`, and never added to the one surface a new user
actually reads. The gates were green throughout because nothing compared the dispatch table to the
help text.

`M7` now does, with a published denominator: **24 of 24**. Internal verbs (`__agent`, the worker
protocol words) are listed as exempt *in the detector, with the reason*, rather than being silently
skipped. Mutation-tested by hiding `plan` again — it goes red naming the command.

**Together `M6` and `M7` close the loop in both directions:** nothing documented may be missing, and
nothing reachable may be undocumented. That pairing is the general fix for a class that produced
`D58`, `D59` and this one in a single sweep — three findings, all in the layer between the working
code and the person trying to use it.
