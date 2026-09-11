# fleet — Tech Debt & Defect Register

> Every row has evidence: a reproducing command and its real output. No evidence, no row.
> Re-run the reproduction before closing an item.
>
> **Why this file exists.** Everything here was found on **2026-09-10**, the first time `fleet`
> was pointed at a repo outside this one — `posx-mokobara-backend`, a Node 20 / TypeScript /
> Medusa v2 service with ~1,200 source files. Seven defects in about forty minutes of ordinary
> use; fixing them and re-running surfaced two more (FD-8, FD-9), both found *by* the fixes.
> None of the nine is reachable by `cargo test --workspace`, which was green at
> `babce1c8edeb` before and after. That is the same class as the SOW's own Finding 0
> (`fleet swarm` green on 351 tests while the real child died parsing its arguments):
> *a proxy is not the property.*
>
> Raw record with full reproductions: [`docs/objections/posx-first-external-use.md`](docs/objections/posx-first-external-use.md).
> Proposed `DELTA.md` rows are at the end of that file — `DELTA.md` is lead-only, so they are
> handed over rather than appended.

## Summary

| ID | Severity | Area | Title | Effort | Status |
|---|---|---|---|---|---|
| FD-1 | P0 | fleet-plan | `fleet sow` panics on any non-ASCII character | S | fixed 2026-09-10 |
| FD-6 | P0 | dispatch | Gate commands hardcoded to `cargo test`; 5 of 8 gates cannot run on a non-Rust repo | M | fixed 2026-09-10 |
| FD-8 | P0 | fleet-verify | Gate **result parsers** are still libtest-shaped, so a configured non-Rust gate can never publish a denominator | M | **open** |
| FD-3 | P1 | dispatch | A REFUSED SOW is written to memory and poisons the next attempt | S | fixed 2026-09-10 · **needs a lead decision** |
| FD-2 | P1 | fleet-plan | `source_intent_hash` refusal names a file nothing reads, and echoes the caller's own flag as "expected" | S | fixed 2026-09-10 |
| FD-5 | P1 | fleet-cli | `--json` carries strictly less than the human summary | S | fixed 2026-09-10 |
| FD-4 | P2 | pipeline | `FLEET_STREAM_DIR` writes nothing; parent dir never created | S | fixed 2026-09-10 |
| FD-9 | P1 | pipeline | `fleet run` never passes a role, so classification always refuses while the stage reports pass | S | **open** |
| FD-10 | P1 | sandbox / fleet-worker | `swarm --agent claude\|codex` cannot authenticate — HermeticEnv wipes `~/.claude` and `~/.codex` | M | **open — needs a design call** |
| FD-7 | P2 | dispatch | `cargo` reported missing when installed outside `PATH` | S | fixed 2026-09-10 |

Severity: **P0** blocks the stated use case · **P1** will cost a user a debugging cycle ·
**P2** wrong-but-survivable

Seven fixed 2026-09-10, in the working tree, uncommitted and unpushed. Two open. Two items need a
lead decision, both flagged in place: the pre-authored test FD-3 turned red, and whether a
required gate that SKIPs should fail `fleet run` (it currently does not, though `fleet gate` and
`fleet oracle` both exit 3 for it — the pipeline is the odd one out, and that contradicts r6/r10).

---

## FD-1 — `fleet sow` panics on any non-ASCII character

- **Severity:** P0 · **Effort:** S
- **Evidence:**
  ```
  thread '<unnamed>' panicked at crates/fleet-plan/src/intake/sow_checks.rs:25:17:
  end byte index 8 is not a char boundary; it is inside '—' (bytes 6..9 of string)
  ```
  `has_line_prefix_ci` slices `l[..prefix.len()]` by **byte** index against a `&str` that may hold
  multi-byte UTF-8. Reproduces with `—`, curly quotes, an accented character, or an emoji near the
  start of any line of the SOW text.
- **Why it matters, beyond the crash:** it is a panic, not one of the typed exits `AGENTS.md` r7
  defines (0/3/6/7/8), so a caller scripting `fleet sow` gets an unclassifiable failure; and it
  violates r8 — a panic writes no receipt. An em-dash is not an exotic input, it is what a person
  writing a real SOW in a real editor produces. The gate that exists to improve written specs
  could not read normally-typed English.
- **Fix (applied):** `l.get(..n)` / `l.get(n..)` with `matches!`. `None` on a non-boundary index
  yields the same verdict as "this line does not start with the prefix", so the ASCII
  case-insensitive semantics and the non-blank-remainder rule are byte-identical to before.
- **Audit — the important half.** `has_line_prefix_ci` turned out to be the **only** reachable
  defect of this class in `fleet-plan` + `fleet-memory`. Every other candidate was inspected and
  is provably boundary-safe, and none was touched: `intake/pattern.rs:54` and
  `intake/pattern_custom.rs:8,10,15` take indices from `char_indices` via `word_runs`;
  `pattern_custom.rs:16,19,20` use `find(lit) + lit.len()`, the end of a real match;
  `find_cli_flag` (`pattern_custom.rs:31-45`) works on `as_bytes()` and cannot land mid-char
  because continuation bytes fail `is_flag_char`; `sow_checks.rs:42-43` is pure `&[u8]`;
  `ready_gate/gate_text.rs:42,47` uses `match_indices` positions; `module_brief_core.rs:41,46`
  guards its `s[3..]` / `s[7..]` with the matching `starts_with`. `fleet-memory` indexes no
  `&str` at all. Sweeps of `fleet-scan/src`, `src/dispatch` and `src/print` found zero hits.
- **Recurrence guard:** `crates/fleet-plan/tests/intake_sow_non_ascii.rs` — em-dash heading, curly
  quotes, `é`, an emoji, plus a loop over lines whose prefix boundary splits each kind.
  Watched-fail proof: with only `sow_checks.rs` restored from HEAD, `0 passed; 4 failed` with the
  original panic text for both `—` and `"`; with the fix, `4 passed`.
- **Out of class, left alone:** `sow_checks.rs::section_body` does `body.push_str(l)` with no
  separator, concatenating a section's lines without newlines. Harmless today — only
  `trim().is_empty()` is asked of the result — but it will bite whoever first reads a body for
  content.

## FD-2 — `source_intent_hash` refusal sends the reader to a file that does not exist

- **Severity:** P1 · **Effort:** S · **Status:** fixed 2026-09-10
- **Correction to the original report.** I first logged this as *"the gate cannot pass."* That was
  wrong, and the way it was wrong is the actual defect. The gate was always passable: the digest
  must be declared as a `source_intent_hash:` line **inside the `--text` body**, exactly as
  `docs/USING-FLEET.md:140-142` documents. My `intent.txt` experiment could never have worked.
- **Evidence:** `fleet sow --text "$(cat sow.md)" --intent-hash <anything>` →
  `REFUSED: source_intent_hash does not match intent.txt (expected <the value just passed>)`.
  Two defects, one line apart in `crates/fleet-plan/src/intake/sow.rs:30-38`:
  1. the message interpolates the **caller's own flag** behind the word "expected", so a reader
     assumes fleet computed it;
  2. it names `intent.txt` — **a file nothing in this codebase ever opens.** Grepping the repo,
     the only occurrences of that string were this message and prose describing the *bash*
     original (`intake.sh:50`'s `hash_file` subprocess). `fleet-plan`'s own `lib.rs` invariant is
     that it opens no file, no socket, no process, and `BLUEPRINT.md:623` assigns the `shasum` to
     the caller.
- **Why it matters:** the filename outlived the implementation it described. An error message is
  the only documentation a user reads at the moment they are stuck, and this one pointed away
  from the single line they were missing. Cost me a full debugging cycle chasing a hash fleet had
  never computed and a file that has not existed since the bash port.
- **Fix (applied):** the refusal now names **both** sides and states explicitly that nothing is
  read from disk. When *neither* side carries a hash — no `source_intent_hash:` line and an empty
  `--intent-hash` — the check is **skipped, not failed**, so a caller with no intent provenance
  still gets a content verdict. It remains a refusal (exit 7), never exit 3: there is no tool or
  file whose absence this check can observe, so an environment fault is not representable here.
  Extracted to `crates/fleet-plan/src/intake/sow_intent.rs` rather than inlined — `sow.rs` was
  already at 64 of its 80 lines.
- **Real output after the fix:**
  ```
  REFUSED: source_intent_hash mismatch: the SOW body declares "abc123", --intent-hash carried
  "deadbeef". Both are compared as literal strings; nothing is read from disk.

  REFUSED: source_intent_hash: the SOW body declares no `source_intent_hash:` line, but
  --intent-hash carried "deadbeef". Add a line `source_intent_hash: deadbeef` to the SOW text
  itself -- this check reads only the --text you passed, no intent.txt is read from disk.
  ```
- **Recurrence guard:** `crates/fleet-plan/tests/intake_sow_intent_hash.rs` — 4 tests: mismatch
  names both sides and denies reading disk; a missing line is told to add one; skip when neither
  side carries a hash; match passes with a case-insensitive key and both sides trimmed.
- **Generalisable lesson, worth more than the fix:** a string that names a file is a claim about
  behaviour, and nothing type-checks it. This one survived a whole language port. Any error text
  naming a path should be generated from the path the code actually opens, or it will drift.

## FD-3 — A REFUSED SOW is written to memory and poisons the next attempt

- **Severity:** P1 · **Effort:** S
- **Evidence:** submit draft A → REFUSED on structure. Fix it, submit again → REFUSED again, now
  with an extra finding:
  `ambiguity (Memory): This looks like a prior decision -- which one governs? -- Similar to a
  remembered item: "<the full text of rejected draft A>"`.
  The memory gate matched the corrected draft against the rejected draft it had just stored.
- **Why it matters:** iterating toward a passing SOW is the intended workflow, and every
  iteration currently raises the noise floor for the next one. Only accepted SOWs are decisions;
  a refusal is the *absence* of a decision, and storing it inverts what the memory gate is for.
- **Root cause — not where I guessed.** Not in `fleet-plan` or `fleet-memory` at all:
  `src/dispatch/plan_cmd.rs:42-46` called `record_sow_refusal(state_dir, &args.text)` **in the
  refusal branch, deliberately** — the comment called it "lesson-worthy". The store is
  `<state_dir>/memory/sow.json` via `dispatch/memory/write.rs`. On the next run `MemoryProbe`
  (`fleet-scan/src/memory.rs:29`, cosine > 0.75) recalls it and emits a `High`-severity
  "This looks like a prior decision".
- **Fix (applied):** `src/dispatch/memory/write.rs` now has one private `record` behind two named
  entry points — a new `record_accepted_sow`, and `record_sow_refusal` kept under its original
  name because `src/pipeline/teach_stage.rs:15` uses it for genuine pipeline-stage failures,
  which *are* lessons. `plan_cmd.rs` calls `record_accepted_sow` in the **accepted** branch only.
- **Recurrence guard:** `src/tests/sow_refusal_is_not_remembered.rs`, driving the real binary —
  a refused SOW writes no `memory/sow.json`; the exact corrected-draft-collides repro; and an
  accepted SOW is still remembered, so the write path stays wired.
- **⚠️ This turned a pre-authored acceptance test red, and it was left red.**
  `src/tests/sow_memory_influence.rs:34` asserts precisely the behaviour this row calls wrong:
  run a refused draft twice against one state dir and demand the second run print
  *"This looks like a prior decision"*. Under "only accepted SOWs are remembered" nothing is
  written and nothing is recalled. `AGENTS.md` hard rule 1 forbids editing a pre-authored
  acceptance test to make it pass, so it stands red and is **the lead's to re-author or bless**.
  Attribution proven: with only the three `src/dispatch` files reverted it passes; restored, it
  fails; it was green in the pre-change baseline. Its underlying property — that fleet-memory is
  really wired into `sow`, and a run-1 write is recalled in run 2 — is preserved and covered by
  the new `an_accepted_sow_is_remembered`. The natural replacement is the same test over a
  **valid** SOW.
- **Behaviour change worth a decision:** resubmitting an already-accepted SOW verbatim is now
  flagged as a prior decision. That is arguably what "prior decision" should mean, but it is new.

## FD-4 — `FLEET_STREAM_DIR` silently writes nothing

- **Severity:** P2 · **Effort:** S
- **Evidence:** `fleet run --help` documents it as mirroring ledger receipts as NDJSON. Set to an
  absolute path under an existing parent, across a full pipeline run to the verify stage: the
  directory was never created, no bytes written, no warning, exit code unaffected.
- **Why it matters:** the silence, not the missing file. A documented switch that does nothing and
  says nothing is indistinguishable from a switch that worked and produced nothing interesting.
- **Root cause:** `crates/fleet-stream/src/sinks/file.rs:29-33` opens with
  `OpenOptions::create(true)`, which creates the *file* but never its parent *directory*, and
  `src/pipeline/stream_flush.rs:29` never created the directory the env var names. It was not
  quite silent — `maybe_flush` surfaced `note: fleet-stream: No such file or directory
  (os error 2)`, with no path and no variable name in it, which is silence for practical purposes.
- **Fix (applied):** `fs::create_dir_all(stream_dir)` at the top of `flush`, and the failure note
  now reads `FLEET_STREAM_DIR=<path>: <reason>`. Both halves — create it, or fail naming it.
- **Verified 2026-09-10** against a deliberately two-level-deep non-existent path: directory
  created, 11 blake3-chained NDJSON receipts written (`prev_hash: GENESIS` at seq 0), plus
  `cursors/file.cursor`.
- **Recurrence guard:** `src/tests/stream_ndjson_missing_dir.rs` — a deep non-existent dir is
  created and written; a dir that *cannot* be created (parent is a regular file) prints a note
  naming `FLEET_STREAM_DIR=` and the path, and still does not fail the run. Watched-fail proven
  by removing only the `create_dir_all` line.

## FD-5 — `--json` carries strictly less than the human summary

- **Severity:** P1 · **Effort:** S
- **Evidence:** the complete stdout of a run whose stderr summary carried six stage results, eight
  gate verdicts with scores, and a refusal reason:
  ```json
  {"task":"…","final_stage":"Verify","classification":null}
  ```
  `classification` is `null` even though the `classify` stage PASSed.
- **Why it matters:** it inverts the point of the flag. Everything meant to automate fleet — CI,
  the central server in the SOW's phase 4, another agent — gets less than a human reading stderr.
- **Root cause:** `src/pipeline/event.rs:36-45` — `PipelineOutcome` had three serialized fields
  and `result` is `#[serde(skip)]`, so the refusal never reached JSON at all. Per-stage outcomes
  existed only as transient `Event`s printed to stderr; per-gate verdicts only inside
  `pipeline/verify_stage.rs`'s loop.
- **`classification: null` was a different bug from the one I reported.** It is a **resume
  artifact**: `stage_loop.rs` `continue`s past any stage the step log already marked done, and my
  two invocations shared a `--task` string, so `Classify` never re-ran. A resumed stage now
  publishes `outcome: resumed` and an absent `elapsed_ms` rather than a fabricated `0` (rule 9).
- **Fix (applied):** new `src/pipeline/records.rs` (`StageRecord`, `GateRecord`) and
  `run_records.rs`. `PipelineOutcome` keeps its three existing keys and gains `stages`, `gates`,
  `refusal`. Gate records are built from the **same** `print::verify_report::line_for` event the
  human path renders, so the two cannot drift. `GateRecord.env_fault` marks a
  `Skip{was_required:true}`, so a caller can tell *"8 gates, none failed"* from *"8 gates, 5 never
  ran"*.
- **No contract touched:** `crates/fleet-types/contracts/*.json` has no hit for `final_stage`,
  `classification` or `pipeline`. `PipelineOutcome` is not a contract shape, so no ADR is needed.
- **Verified 2026-09-10** — real output now carries 6 top-level keys, 7 stage records, 8 gate
  records with denominators, and the refusal string. It also surfaced something previously
  invisible: `classification.role` is `"invalid"`, refusal *"role has no eligible tier set"* —
  `fleet run` never passes a role. Separate finding, logged as FD-9.
- **Recurrence guard:** `src/tests/run_json_reports_stages_and_gates.rs`, real binary, both the
  clean and refused shapes.

## FD-6 — Gate commands are compile-time constants; 5 of 8 gates cannot run on a non-Rust repo

- **Severity:** P0 · **Effort:** M
- **Evidence:** real verify stage against the Node repo —
  ```
  SKIP gate unit tests -- unit tests unavailable
  SKIP gate mutants -- mutants skipped: set FLEET_MUTANTS=1 to run it
  PASS gate semgrep 1529/1529
  FAIL gate trivy 0/10 -- NonZeroExit(1)
  SKIP gate recur -- recur: no applicable input in this repo
  PASS gate detectors 111/111
  SKIP gate policy -- policy unavailable
  FAIL gate corpus 25/27 -- NonZeroExit(1)
  ```
  The unit-tests gate shells out to `cargo test --workspace`, hardcoded in the registry's `static`
  `GATES` array. A TypeScript repo has no such command, so the gate reports SKIP — `checked == 0`.
- **Why it matters — this contradicts two of this repo's own hard rules:**
  - `AGENTS.md` r6: *"A gate/check that examined zero inputs FAILS. Never passes."*
  - `AGENTS.md` r10: *"Publish the denominator. A verdict carries `{checked,total}`; `checked==0`
    is a failure."*

  And it contradicts the crate built to enforce them. The SOW §04 cites `fleet-verify`'s
  `zero_exit_with_zero_total_is_fail_not_pass` as evidence fleet is *"built to catch the 'measured
  nothing' bug on purpose."* On this run four gates measured nothing and the pipeline treated all
  four as non-events — the invariant is enforced **inside** a gate's own result and not **across**
  the gate set.
- **Consequence for the estate:** fleet cannot currently serve as the pre-merge gate for the 24
  non-Rust POSX repos it is intended for. Not because the gates are wrong — because their commands
  are baked in at compile time.
- **Fix:** a per-repo config discovered in the target repo overriding the command for a named
  gate, with today's hardcoded values as the defaults so a Rust repo's behaviour is byte-identical
  when no config exists. The override must let a repo declare **what its unit-test command is** —
  never let a repo opt out of having one. `GATES` is already a parameter to `run_all` /
  `run_pipeline` and a `NO_GATES` empty-slice pattern exists in the tests, so the seam is there.
- **Fix (applied):** per-repo `<repo>/.fleet/gates.toml`, discovered in the target repo:
  ```toml
  [gates."unit tests"]
  command = ["npm", "run", "--silent", "test:unit"]
  probe   = "npm"
  ```
  `.fleet/` because this estate already puts per-repo fleet inputs there
  (`fleet-worker`'s `.fleet/agents.toml`, `.fleet/skills.toml`) — one fleet directory beats a
  third top-level dotfile in someone else's repo. Documented in `docs/GATES-CONFIG.md`.
- **Semantics, stated deliberately:** a repo may only **replace** a known gate's command and
  probe. It may not add, delete, or downgrade a gate, and there is **no `enabled = false`** —
  under r6 an opt-out switch is precisely the check that is cheaper to fake than to satisfy. A
  repo declares *what its unit-test command is*, never *that it has none*. An unknown gate id or
  an empty command is a hard **exit 3**, not a no-op, so a typo cannot silently leave the gate on
  `cargo test`.
- **`fleet-verify` was not touched, and could not be.** The suggestion to make `GateSpec` own its
  strings is unreachable without editing pre-authored tests, which `AGENTS.md` r1 forbids:
  `crates/fleet-verify/tests/registry_integrity.rs:15-18` matches `GateCommand` **exhaustively
  over two variants**; `classify_rules.rs:21` and `orchestration_probe_skip.rs:15,58` construct
  `GateCommand::OnPath(&["true"])` from `&'static [&'static str]` literals; both also build
  `GateSpec` with five-field struct literals. So no new variant, no new field, no owned payload.
  Instead `src/dispatch/gate_config.rs` assembles a `Vec<GateSpec>` at the CLI layer
  (`GATES.to_vec()` + overrides) and passes it to the existing `run_all` / `run_pipeline`
  parameter — the seam the `NO_GATES` test pattern already uses. Config-derived strings are
  `Box::leak`ed: bounded, one-shot, and the codebase's own idiom
  (`orchestration_probe_skip.rs:55` leaks a generated gate id the same way).
- **If the owned shape is ever wanted**, those three pre-authored tests must be re-authored by
  the lead first. Recorded here so the constraint is not rediscovered.
- **The r6/r10 aggregate now exists and fires.** Real output on the Node repo:
  ```
  gates    1 attempted -- 0 passed, 1 failed, 0 skipped
  checks   0 performed  (no gate examined any input -- treat this run as a failure, not a pass)
  ```
- **Verified 2026-09-10:** `fleet gate --id "unit tests" --repo posx-mokobara-backend` runs
  `npm run --silent test:unit`, not cargo. It then fails on FD-8 — see below.
- **New dependency:** `toml = "0.8"` in `src/Cargo.toml` (already in `Cargo.lock` via
  `fleet-worker`); `fleet/Cargo.lock` is modified as a result.

## FD-8 — Gate result parsers are still libtest-shaped, so a configured non-Rust gate can never pass

- **Severity:** P0 · **Effort:** M · **Status:** **open** — found by verifying the FD-6 fix
- **Evidence:** with `.fleet/gates.toml` pointing the unit-tests gate at the Node repo's real
  suite, the command runs and the suite genuinely passes:
  ```
  $ npm run --silent test:unit
  Test Suites: 26 passed, 26 total
  Tests:       671 passed, 671 total
  ```
  but fleet reports:
  ```
  $ fleet gate --id "unit tests" --repo .
      .... gate npm run --silent test:unit -- running
      FAIL gate unit tests -- Unparseable
      checks 0 performed  (no gate examined any input -- treat this run as a failure, not a pass)
  exit=6
  ```
- **Root cause:** `crates/fleet-verify/src/parsers.rs:73-78`. `unit_tests` looks for libtest's
  fixed `"N passed; M failed;"`. Jest prints `"671 passed, 671 total"` — comma, not semicolon,
  and *total* rather than *failed*. Every parser in that file is hardcoded to one tool's output
  shape: `mutants` wants `caught=`/`total=`, `semgrep` wants `" files scanned"`, `corpus` wants
  `DENOMINATOR checked=`.
- **Why this is P0 and not a footnote:** FD-6 made the **command** per-repo and left the
  **denominator parser** global. Those two have to move together. A Node repo can now be told to
  run its own tests and still cannot produce a verdict, so the gate is unusable in practice on
  exactly the 24 repos FD-6 was fixed for. FD-6 is half a fix without this.
- **The behaviour is correct, which is the good news.** It fails closed at exit 6 (invariant
  violation) rather than passing a run it could not measure, and the aggregate line says so
  plainly. r6 and r10 are doing their job — they are just now blocking the thing we wanted
  to enable.
- **Fix sketch:** the per-gate config needs to carry the denominator alongside the command —
  either a named built-in parser (`libtest` | `jest` | `pytest` | `go-test`), or an explicit
  regex with two capture groups. A named set is safer: a regex in a repo-owned config is a way
  to fake a denominator, and r6 exists precisely to stop that. Whatever the shape, it must be
  impossible to declare "no denominator".
- **Recurrence guard:** a test per supported runner asserting a real captured stdout sample
  parses to the right `{checked,total}` — and one asserting an unrecognised shape is
  `Unparseable`, never a pass.

## FD-9 — `fleet run` never passes a role, so classification always refuses

- **Severity:** P1 · **Effort:** S · **Status:** **open** — surfaced by the FD-5 fix
- **Evidence:** now that `--json` carries the classification, a normal `fleet run` shows:
  ```json
  "classification": {"role": "invalid", "selected_adapter": null, "resolved_model": null,
    "refusal": {"stage": 1, "stage_name": "explicit role",
                "reason": "role has no eligible tier set",
                "fix": "pass a role of lead, builder, verifier, designer, or meter"}}
  ```
  All six routing stages report `checked: 0, total: 5`.
- **Why it matters:** the classify stage reports **pass** while its own payload is a refusal, and
  no model is ever resolved. This was invisible for as long as `--json` omitted the field — which
  is the argument for FD-5 in one line: *the fix to the reporting immediately found a defect in
  the thing being reported.*
- **Fix sketch:** either `fleet run` takes `--role` (defaulting to `builder`), or the classify
  stage's refusal must fail the stage rather than passing with an invalid role.

## FD-7 — `cargo` reported missing when it is installed

- **Severity:** P2 · **Effort:** S
- **Evidence:** `fleet doctor` prints `cargo: missing` on a machine with cargo 1.98.1 at
  `~/.cargo/bin/cargo`. Fleet resolves it from `PATH` only; `~/.cargo/bin` is not on the default
  non-login `PATH` on macOS.
- **Why it matters:** it compounds FD-6. The unit-tests gate degrades to SKIP for what is really
  an **environment fault** (r7 exit 3), and reports it as a benign skip. r7 says an environment
  fault must never be reported as an agent failure; the inverse holds too — it must not be
  laundered into a non-failure.
- **Root cause:** three separate `which`-only lookups — `src/dispatch/ops_cmd.rs:37` (`doctor`),
  `which_probe.rs:31` (the gate probe), `mutants_probe.rs:27`. `which` reads `$PATH` only, and
  rustup's `~/.cargo/bin` is on `PATH` only once a shell profile has been sourced.
- **Fix (applied):** new `src/dispatch/tool_path.rs` — `find()` tries `$PATH`, then
  `$CARGO_HOME/bin`, then `~/.cargo/bin`; `searched()` renders the list for the miss message.
  All three call sites use it, and `verify_runner_bounded.rs`'s spawn was routed through
  `tool_path::resolve_bin` too — otherwise a probe answering *"cargo is installed"* would be
  followed by a spawn failing `ENOENT`. `doctor` now prints where it found the tool, in both the
  human and `--json` paths (new `cargo_path` / `git_path`).
- **Verified 2026-09-10**, with rustup deliberately off `PATH`:
  ```
  $ env PATH=/usr/bin:/bin fleet doctor
  cargo: found (/Users/rachitshah/.cargo/bin/cargo)
  git: found (/usr/bin/git)
  ```
  and a genuinely absent tool now names where it looked:
  `SKIP gate policy -- policy not found: no ` + "`conftest`" + ` in $PATH, /Users/…/.cargo/bin`
- **Recurrence guard:** `src/tests/cargo_found_outside_path.rs` — 3 tests against the real binary
  with a `PATH` excluding rustup.
- **Left alone, flagged:** `doctor` still exits 0 while reporting a miss. Outside the scope of
  this row, and `build_identity_present.rs` asserts `doctor --json` succeeds — so changing it is
  a lead decision.

---

## FD-10 — `swarm --agent claude|codex` cannot authenticate — HermeticEnv wipes `~/.claude` and `~/.codex`

- **Severity:** P1 · **Effort:** M · **Status:** **open — needs a design call**
- **Area:** sandbox / fleet-worker
- **Context:** PR #9 (`feat/swarm-agent-flag-and-non-tty-invocation`) wires
  `fleet swarm --agent claude|codex|freelane` end-to-end at the CLI/dispatch layer. The flag
  parses, the adapter is selected, and the lane spawns — but the two real-agent adapters
  (`claude` and `codex`) exit 1 on first invocation because they cannot find their auth on disk.
- **Evidence / reproduction:**
  ```
  $ fleet swarm --repo /tmp/scratch-repo --task "edit README" --role builder --agent claude
      [builder-*] spawned
      [builder-*] outcome: Failed { reason: "claude -p exited 1: not authenticated" }
  EXIT:7
  ```
  Same shape with `--agent codex` — the `codex` CLI reads `$HOME/.codex/auth.json` and finds
  nothing.
- **Root cause:** `crates/fleet-worker/src/sandbox/hermetic_env.rs:48-61` (`HermeticEnv::apply`)
  calls `command.env_clear()` and then points `HOME` at a fresh per-lane tempdir built in
  `build(root)` at `hermetic_env.rs:24-38` (`root.join("home")`). The header comment on
  `hermetic_env.rs:2-4` states this is deliberate: *"a spawned CLI never sees the real `HOME`."*
  The `claude` CLI keeps its OAuth token / config under `$HOME/.claude/` and the `codex` CLI
  keeps its credentials under `$HOME/.codex/`; both directories are absent inside the hermetic
  tempdir, so both CLIs exit 1 before doing any work. `freelane` (the keyless default) is
  unaffected because it does not read `$HOME`.
- **Why this is a design call, not a patch:** the reason `HermeticEnv` exists is to keep a
  spawned lane from reading arbitrary secrets, dotfiles, or shell state out of the user's real
  `$HOME`. Punching two directory-shaped holes in it is exactly the kind of decision the auto-mode
  classifier is meant to refuse without a human — it changes the sandbox contract that the rest
  of `fleet-worker` was written against, and it is not obviously the *right* trade-off for every
  deployment (a CI runner vs. a developer laptop want different answers).
- **Candidate mechanisms (each has a real trade-off — reviewer picks):**
  1. **Read-only bind/symlink allowlist.** After `build(root)`, symlink (or `bind_mount` on
     Linux) the real `~/.claude` and `~/.codex` into the tempdir's `home/`. Cheapest to
     implement, keeps the rest of `$HOME` invisible. Cost: the lane can now read *and write* the
     real auth files unless the symlink target is made read-only per-lane; a rogue prompt can
     exfiltrate the OAuth token by reading the file and printing it.
  2. **Env passthrough via a config-dir override.** If the `claude` CLI honours a
     `CLAUDE_CONFIG_DIR` (or equivalent) env var, forward that value from the parent env in
     `HermeticEnv::apply` instead of touching `$HOME`. `codex` would need the equivalent
     (`CODEX_HOME` / similar). Cost: needs verification that both CLIs actually support such an
     override — if not, this mechanism is off the table; and it still exposes the token file to
     the lane's process.
  3. **Wholesale opt-out of hermetic env for the two named adapters.** In the adapter's
     `Command`-building path, skip `HermeticEnv::apply` entirely and inherit the parent env when
     `--agent` is `claude` or `codex`. Simplest, worst for isolation — the lane inherits the
     whole user env, not just the two auth dirs. Explicitly the wrong choice for a shared or
     CI-runner deployment.

  None of the three is free. Option 1 with a read-only bind is the least-bad default for a
  single-developer laptop; option 3 is defensible for local `--agent claude` iteration and
  indefensible for anything else. The register does not pick one.
- **Reviewer / lead decision needed:** which of {read-only allowlist, config-dir env
  passthrough, per-adapter opt-out} to adopt — and how to gate it (build feature, runtime flag,
  per-repo config in `.fleet/`). This is *agent utility vs. sandbox integrity*: making
  `--agent claude|codex` work end-to-end is the point of PR #9, but every mechanism above widens
  what a spawned lane can read, and the whole reason `HermeticEnv` exists is to narrow that.
  A human/reviewer sets the ratio; this row does not.
- **Workaround today:** use `--agent freelane` (the default). It is keyless and does not read
  `$HOME`, so the hermetic env does not block it.
- **Not fixed here on purpose.** This entry is doc-only. Editing `hermetic_env.rs` to allowlist
  auth dirs is the fix; making that choice unattended would be the wrong shape of change under
  auto mode, which is why the classifier refused it.

---

## What worked

Recorded because a register of only defects misrepresents the tool.

`fleet sow`'s content gates rejected the first SOW draft with ten specific violations plus an
ambiguity finding, and every one was correct — the draft genuinely had no measurable acceptance
threshold and no explicit non-goals. Rewriting to clear them produced a materially better
document. The ambiguity check (*"no stated success metric — work could ship and still miss the
goal"*) is the kind of finding a human reviewer skips. `semgrep` (1529/1529) and `detectors`
(111/111) both ran cleanly against a TypeScript tree with no configuration.

The SOW gate is the part of fleet that already pays for itself on an external repo. The verify
stage is the part that does not yet leave this tree.


---

## Verification run — same use case, before and after

Both runs: `fleet run --repo posx-mokobara-backend`, the Shopify customer-order-history task.
Before is `babce1c8edeb` clean; after is the same commit with the seven fixes in the working tree.
`FLEET_LANE_BUDGET_MB=1024` on the after-run because this box sits within ~40 MB of the 2048 MiB
per-lane floor and every capacity-gated invocation otherwise refuses at exit 7 — the product's
own documented knob, not a workaround for the fixes.

| | before | after |
|---|---|---|
| `fleet sow`, em-dash in the text | **process panic**, no exit code, no receipt | `ok: sow valid`, **exit 0** |
| `fleet sow` intent hash | refusal naming a nonexistent `intent.txt` | names both sides; skips when neither carries a hash |
| memory after a refusal | rejected draft stored, poisons the next run | nothing written; accepted SOWs still stored |
| `fleet doctor` | `cargo: missing` | `cargo: found (/Users/…/.cargo/bin/cargo)` |
| unit-tests gate | `SKIP -- unit tests unavailable` (`cargo test`) | runs `npm run --silent test:unit` |
| policy gate skip reason | `policy unavailable` | ``policy not found: no `conftest` in $PATH, /Users/…/.cargo/bin`` |
| `FLEET_STREAM_DIR` | nothing created, nothing written | dir created two levels deep, 11 blake3-chained receipts, `cursors/file.cursor` |
| `--json` keys | `task`, `final_stage`, `classification`(null) | + `stages` (7), `gates` (8, with denominators and `env_fault`), `refusal` |
| aggregate denominator | absent | `checks 0 performed (no gate examined any input -- treat this run as a failure, not a pass)` |

The after-run still refuses, and that is the correct outcome: `trivy` and `corpus` are the repo's
pre-existing failures, `unit tests` is FD-8, and `recur` reports `MeasuredNothing`. What changed
is that **every one of those is now visible and typed**, where before four gates silently
reported SKIP and the run looked like it had been checked.

### Real after-run verify stage

```
FAIL gate unit tests -- Unparseable
SKIP gate mutants -- mutants skipped: set FLEET_MUTANTS=1 to run it
PASS gate semgrep 1537/1537
FAIL gate trivy 0/10 -- NonZeroExit(1)
FAIL gate recur -- MeasuredNothing
PASS gate detectors 111/111
SKIP gate policy -- policy not found: no `conftest` in $PATH, /Users/rachitshah/.cargo/bin
FAIL gate corpus 25/27 -- NonZeroExit(1)
REFUSED verify: gate(s) failed: unit tests: Unparseable; trivy: NonZeroExit(1);
                recur: MeasuredNothing; corpus: NonZeroExit(1)
```

### Repo checks

```
cargo build --release                                  Finished
cargo clippy --workspace --all-targets -- -D warnings  clean
find src crates -name '*.rs' | xargs wc -l | awk '$1>80 && $2!="total"'   (prints nothing)
cargo test --workspace --no-fail-fast                  533 passed, 2 failed
```

The two failures, both left red on purpose:

1. `sow_memory_influence` — a **pre-authored** acceptance test asserting the exact behaviour FD-3
   calls wrong. `AGENTS.md` r1 forbids editing it green. Lead's to re-author or bless.
2. `mutants_gate_actually_runs_when_opted_in` — pre-existing; `cargo-mutants` is genuinely not
   installed on this machine. Proven red in the baseline before any edit.

A plain `cargo test --workspace` on this box fails 19 targets, *all* with
`system capacity check failed: available memory ~2010 MiB is below one lane's budget of 2048 MiB`.
That is the machine, not the code; `mutants_skip_reason` (×2), `capacity_refusal_real_binary` and
`verify_gate_scopes_to_repo` all pass in isolation or with the budget override.
