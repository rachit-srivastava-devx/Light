# F10 — P0 capstone: spoken → frozen → built → attested → PR

**Lane:** `lane/F10-capstone` · worktree `Light/.worktrees/F10-capstone` (created, at `9efba27`)
**Repo scope:** `orb`+`fleet` · **Deps:** F01–F09 (all ☑, merged to `master`)
**Blueprint:** `blueprints/Speed-of-Thought-L8-Deep-Dive/PHASES-TO-USABLE.md` §2 (P0), lines 57–91
**Written by:** lead-architect (Opus 5), 2026-09-07. **Contract-only; the tree diff is this file plus
`status/` plus `FEATURES.md`'s F10 row.**

> This is the **P0 exit trigger**. It is the one lane whose failure mode is not "a bug" but
> "nine components that were each verified in isolation and never composed." The single
> most-repeated failure in this estate is *a proxy passed for the property*
> (`proxy-is-not-the-property`: an API 200 is not a rendered page, a DOM count is not a paint).
> F10 is exactly where that trap lives: **a green stub-driven integration test is a proxy for
> "it built a real module."** §6 splits the lane into two tiers precisely so that proxy can
> never be reported as the property.

---

## 0. Preflight — what I measured, not what the dispatch claimed

The dispatch asked one decisive question: *is the freeze→handoff→SOW→lane→PR chain already
automatic, or does a human/script invoke each stage?* I read the merged code on `master`
(`9efba27`), not the contracts. Everything in §3 carries a `file:line` citation I opened.

Three claims I did **not** take from a subagent and re-verified by hand, because the contract's
shape turns on them:

| claim | verified at |
|---|---|
| `fleet run` prints the artifact id machine-parseably | `main.rs:1548` — `println!("artifact={}", evidence.artifact_id);` |
| the 8-element attestation is **complete** after a plain `fleet run` (no separate `oracle`/`adjudicate` needed) | `main.rs:1938-1939` — `write_json_atomic(&att_path, …)` then `adjudicate_command_inner(&artifact_id, announce)?` **synchronously, inside `run_with_evidence`** |
| `--agent stub` lands a real, non-empty diff | `main.rs:3199-3207` — appends `b"// fleet stub change\n"` to `<repo>/main.rs` |

One correction I had to make to my own model mid-read: I first assumed `fleet run` leaves the task
at `Attested` and that reaching `Accepted` needed `fleet adjudicate`. **Wrong** — `lifecycle::drive_run`
runs the full ladder to `Accepted` itself (`lifecycle.rs:1143-1147`), and `fleet adjudicate` never
touches lifecycle state. Recorded because a builder who inherits my first model will insert a
command that does nothing.

---

## 1. Restatement

One real, small module goes **typed/spoken description → orb build-mode dialogue → ❄ freeze →
fleet SOW → human accept → lane execution on the unchanged `run_with_evidence` kernel →
8-element attestation → a real PR carrying that attestation**, and nobody wrote the spec. Measure
the wall-clock.

F10 is **integration proof**, not new product surface. It writes **zero lines of Rust and zero
lines of Python** (§9 makes that a checkable gate).

---

## 2. C1 verdict (registry-first, C1/L2)

**`install` for six of seven hops; `build-new` for exactly one thin artifact — a lane driver —
plus the acceptance suite that proves the chain.**

Said plainly, as the dispatch asked: **this is *almost* "just an integration test with a thin
driver," and I am not going to inflate it.** Four of the seven stage-to-stage hops are already
wired and automatic in merged code. Two are unwired — and they are unwired because **nothing in
this tree ever drives them**, not because a seam is missing. One is manual **by law** and must
stay that way.

The new code is:

1. `fleet/bin/f10-lane-drive.sh` — ~120 lines of shell. Walks the two unwired hops (H5, H7). It
   invents no logic: it reads a SOW record, invokes two existing CLI commands, parses their two
   existing machine-readable output lines, and refuses loudly on every failure path.
2. `fleet/tests/acceptance/f10-capstone.sh` — lead-owned. The suite in §7.
3. One additive `stage` line in `fleet/verify.sh`.
4. `Light/docs/evidence/F10-p0-exit.md` — the Tier-B measurement (§6.2).

There is **no new abstraction, no new keel subcommand, no new relay endpoint, no new schema.**
§4(d) kills the keel-subcommand option explicitly.

---

## 3. Findings that change this lane (each proved against the tree, not recalled)

### 3.1 The chain, hop by hop — the answer to the ground-truth question

| hop | from → to | wired? | evidence |
|---|---|---|---|
| **H1** | dialogue turn → the stopping rule returns `Move.Freeze` | **AUTO** | `orb/backend/relay-py/src/orb_relay/app.py:1077` (`next_move`), `:1109` (`if isinstance(move, Move.Freeze)`) |
| **H2** | `Move.Freeze` → `hand_off(brief)` | **AUTO** | `app.py:1132-1133` — inside the `/v1/respond` turn handler, wrapped in `try/except` so a handoff fault can never swallow the freeze |
| **H3** | `hand_off` → `fleet freeze stamp` → `fleet sow --lld` → a record on disk with `accepted: null` | **AUTO** | `build/handoff.py:114` (stamp), `:127` (`sow --lld`); record written by `sow::write_atomic`, `sow.rs:83-91`; `accepted` set to `null` at `sow.rs:79` |
| **H4** | SOW record → **accepted** | **MANUAL — BY LAW** | gate: `main.rs:665-722` (`enforce_accepted_sow`); the only producer is `fleet sow accept --id <ID>` (`main.rs:625-657`). `sow::review_status` refuses unless `accepted` carries `by`+`at`+`receipt` (`sow.rs:93-116`) |
| **H5** | accepted SOW → `run_with_evidence` | **NOT WIRED** | `fleet run` gates on H4 at `main.rs:113` then calls `lifecycle::drive_run` → `run_command` → `run_with_evidence` (`main.rs:1547`). **Nothing invokes it.** No daemon, poller, or watcher over `$FLEET_STATE/sows/` exists anywhere in the tree (searched; the loops in `repl.rs:148` and `console.rs:338` are display-only) |
| **H6** | `run_with_evidence` → complete 8-element attestation **and** lifecycle `Accepted` | **AUTO** | attestation written `main.rs:1938`, `adjudicate_command_inner` runs synchronously `main.rs:1939`; `Accepted` reached inside `drive_run` at `lifecycle.rs:1143-1147` |
| **H7** | `Accepted` → PR | **NOT WIRED** | `pr_emit_run` has exactly **two** callers — `pr_emit_command` (`main.rs:5259`) and the test probe (`main.rs:5704`). **`run_with_evidence` never calls it.** `fleet pr emit` is a separate top-level command (`main.rs:143`) |

**So: four hops automatic, one manual by law, two unwired.** H5 and H7 are F10's entire build-new
scope, and each is one existing CLI invocation away from closed.

### 3.2 H4 is not a gap — it is A15/D4, and F10 must not close it

`enforce_accepted_sow` is not an oversight. It refuses execution unless a human ran
`fleet sow accept`, **and** the acceptance receipt is present in the ledger as a `gate_verdict` /
`SOW_ACCEPTED` row whose `hash` matches the record's `receipt` (`main.rs:692-709`). That is the
constitution's human-merge law applied at the spec boundary. A driver that self-accepts would
delete the only human gate in the chain. **F10-T9 and M4/M10 exist to make that impossible.**

The blueprint agrees, in its own P0 caveat: *"the review still needs your eyes (trust is P1)"*
(`PHASES-TO-USABLE.md:91`).

There **is** an escape hatch, `FLEET_SOW_BYPASS=1` (`main.rs:668-683`), which `p0.sh:31` uses.
It writes a visible ledger receipt (`"visible":true`). **F10 may not use it in any tier.** F10's
whole claim is that the SOW came from the orb and a human accepted *that* SOW; bypassing would
make the claim vacuous. F09's script already proved the real `fleet sow accept --id <sow_id>`
path exits 0 on an orb-produced SOW (`tests/acceptance/orb-freeze-to-sow.sh:232`), so the real
path is known to work.

### 3.3 The task text is the join key, and it lives in the SOW record

`sow::id_for_task` is `blake3(task)` (`sow.rs:25-27`), and `ready_record` stores the compiled task
text verbatim under `"task"` (`sow.rs:72-81`). So the driver never needs to recompile anything: it
reads `.task` off the record and passes it to `fleet run --task` and `fleet pr emit --task`, and
`enforce_accepted_sow` recomputes the same id. F09's script already does exactly this read
(`orb-freeze-to-sow.sh:249`). **This is why F10 needs no new schema and no new plumbing.**

Consequence the builder must not miss: `fleet sow accept` and `fleet run` **must share one
`$FLEET_STATE`**, because the gate looks for the accept receipt in that state's ledger (§3.1 H4).

### 3.4 What F09's drive script already built, that F10 extends rather than rewrites

`fleet/tests/acceptance/orb-freeze-to-sow.sh` (321 lines) already: builds the keel binary, boots a
**real** `uvicorn` relay and a **real** disposable stub gateway as two genuine OS processes, drives
real `curl` POSTs to `/v1/respond` until the freeze turn, reads `freeze.node_id` / `handoff.sow_id`
off the real response, asserts the record's `lld_provenance` against the real freeze ledger, runs a
real `fleet sow accept`, and includes a negative control (a `chmod -x`'d disposable binary copy)
plus a recovery check. **F10's script reuses that pattern; it does not reimplement it and does not
edit it.**

### 3.5 `fleet run` is already proven end-to-end by `p0.sh` — with a bypass

`tests/acceptance/p0.sh:42` already drives `fleet run --task "add a --version flag" --repo "$TARGET"
--agent stub`, extracts `artifact=<64-hex>`, and verifies the frozen artifact and the attestation
(stages A–C). It does this with `FLEET_SOW_BYPASS=1` (`p0.sh:31`) because it is testing the evidence
kernel, not the planning gate. **F10 is the first and only test in this tree that drives that same
kernel with a real, orb-originated, human-accepted SOW.** That is the delta, and it is the whole
point.

### 3.6 `fleet pr emit`'s preconditions are all already satisfiable after a plain `fleet run`

`pr_emit_run` (`main.rs:5265-5404`) requires, in order: lifecycle state exactly `Accepted`
(`:5283`) — satisfied by `drive_run` (§0); a valid 64-hex artifact id whose bytes on disk re-hash
to it (`:5293-5317`); `attest_verify_inner` ok (`:5320`); every one of `AttestationBundle`'s 8
`REQUIRED` elements present (`:5346`, list at `lifecycle.rs:357-366`: `sow`, `blind_suite`,
`independent_verification`, `adequacy`, `blast_radius`, `rollback`, `cost`, `oracle_independence`);
a non-empty diff (`:5364`). **No intermediate command is needed.** The driver is genuinely two
invocations.

---

## 4. Killed alternatives

**(a) Write a new orchestration layer — a daemon/poller watching `$FLEET_STATE/sows/` that fires
the lane when a record becomes accepted. KILLED.**
Four of seven hops are already wired and the two gaps are *one CLI call each* (§3.1). A poller
would (i) reimplement `enforce_accepted_sow`'s gate in a second place, where it can drift;
(ii) turn the one human gate in the system into a race — a poller that sees `accepted` has no way
to know the human meant *this* artifact, and A15 exists precisely to stop that; (iii) add a
long-lived process this build has no supervisor, restart policy, or crash story for.
*Revive trigger:* **P3**, when `swarm dispatch` actually spawns N concurrent worktree lanes
(`PHASES-TO-USABLE.md:158-160` — today it is sequential bookkeeping). A scheduler is real work
*there*, with real justification. Building it here would be scope inflation dressed as architecture.

**(b) Fake or mock any stage to make the demo easier. KILLED — with exactly one inherited
exception.**
The one exception is the `gh` CLI, shimmed as F08's own contract already shims it: a script on
`PATH` that logs its argv and prints a fixed PR URL (`tests/f08_pr_emit.rs:101-119`). Everything
else in F08's harness is real — real `git init`, real bare-repo `origin`, real `git push`
(`f08_pr_emit.rs:86,94`). F10 inherits that boundary and no more: real relay process, real stub
*gateway* (an LLM stand-in, not a stage stand-in), real keel binary, real git, real ledger, real
attestation, real diff. **Tier B (§6.2) removes even the `gh` shim.**
Why this is the hill: a mocked stage in the *capstone* would make every one of F01–F09's
verifications unfalsifiable *in composition* — which is the exact thing F10 exists to test. This
tree has already paid for that lesson twice (`adoption-requires-a-real-run`: two orchestration
tools adopted and defended for a day without either ever executing).

**(c) Skip PR-emit/attestation and call freeze→SOW alone "done". KILLED.**
That is F09's boundary verbatim — already built, verified, and merged (`FEATURES.md` F09 row). An
F10 that stops at the SOW re-ships F09 under a new number. The blueprint names the exit as
`spoken idea → frozen → built → attested → PR` and says *"Measure the freeze→attested-PR
wall-clock"* (`PHASES-TO-USABLE.md:85-87`). A PR with no attestation attached is also not the
property — hence F10-T3 asserts the attestation is *in the PR body*, not merely on disk.

**(d) Make the driver a keel subcommand (`fleet lane run --sow <id>`) instead of a shell script.
KILLED.**
New CLI surface is F08-shaped work: its own usage text, its own refusal taxonomy, its own receipt
kinds, its own tests — and it would put F10's glue inside the kernel that F10's own row says to
reuse unchanged ("reuse (kernel) + integrate"). A shell driver is honest about being glue and is
exactly what F11's status echo can shell out to.
*Revive trigger:* F11, if the status echo needs structured lifecycle events rather than two parsed
stdout lines. Promote it then, with its own contract.

**(e) Remove the H4 human-accept gate so the headline wall-clock is "pure machine time". KILLED.**
A15/D4 (§3.2). It would also make the headline number a lie by omission. The contract instead
requires **two** clocks (F10-T5) with the human gate named as sitting between them — per
`clock-must-start-at-the-users-action`, a latency number without its start point is a proxy.

**(f) Report Tier A (stub agent) as the P0 exit. KILLED — see §6.** This is the one that would
actually have happened by default, so it is a contract clause, not a note.

---

## 5. The interface (exact)

### 5.1 `fleet/bin/f10-lane-drive.sh` — the lane driver

```
usage: f10-lane-drive.sh --sow <64-hex> --repo <PATH> --base <BRANCH>
                         [--agent <stub|env-probe|claude|codex>]   # default: stub
                         [--head <BRANCH>]                        # default: fleet/f10-<sow[0:12]>
                         [--since-ms <EPOCH_MS>]                  # the freeze instant, for ms_freeze_to_pr
```

Reads `$FLEET_STATE` from the environment (never derives or creates it — §3.3/§12.3).

**Steps, in order. Every one is an existing command; the driver adds no logic of its own.**

1. Read `$FLEET_STATE/sows/<sow>.json`. Absent → refuse `SOW_NOT_FOUND`.
2. Require `.accepted` to be a non-null object with `by`, `at`, **and** `receipt`. Otherwise refuse
   `SOW_NOT_ACCEPTED`, and print the exact remedy `fleet sow accept --id <sow>`.
   **The driver must never run `fleet sow accept` itself** (§3.2).
3. `task="$(…read .task…)"`. Empty → refuse `SOW_TASK_EMPTY`.
4. `fleet run --task "$task" --repo <repo> --agent <agent>`; capture status **directly, never `$?`
   after a pipe** (§12.5). Non-zero → refuse `RUN_FAILED`, echoing fleet's own exit code and its
   last output lines verbatim.
5. Parse `artifact=<64-hex>` from that output (`main.rs:1548`). Absent → refuse `NO_ARTIFACT_ID`.
6. `fleet pr emit --task "$task" --artifact <aid> --repo <repo> --base <base> --head <head>`.
   `--head` is **mandatory in practice** — see §12.1.
7. Parse `pr_emit: state=Proposed branch=… pr_url=… commit=… changed_files=…` (`main.rs:5389-5392`).
   If instead `pr_emit_refused: code=…` appears, refuse `PR_EMIT_REFUSED` carrying that code
   **verbatim**. Any other outcome → `PR_EMIT_FAILED`.

**Success — exactly one line, and only if a real `pr_url` was parsed:**

```
f10_lane: tier=<A|B> agent=<name> sow=<64hex> artifact=<64hex> pr_url=<url> branch=<b> commit=<sha> changed_files=<n> ms_run=<n> ms_pr=<n>[ ms_freeze_to_pr=<n>]
```

`ms_freeze_to_pr` is emitted **only** when `--since-ms` was given. Exit `0`.

**Refusal — exactly one line, and no `f10_lane:` line ever:**

```
f10_lane_refused: code=<CODE> detail=<one line>
```

`CODE ∈ { SOW_NOT_FOUND, SOW_NOT_ACCEPTED, SOW_TASK_EMPTY, RUN_FAILED, NO_ARTIFACT_ID,
PR_EMIT_REFUSED, PR_EMIT_FAILED }`. Exit non-zero. **A refusal must never print a duration field**
(F10-T5's do-nothing control).

**The load-bearing invariant, stated as one sentence a verifier can test:** *the driver prints
`f10_lane:` if and only if a real `pr_url` came out of a real `fleet pr emit` that a real
`fleet run` fed.* M2 and M3 attack exactly this.

### 5.2 `fleet/tests/acceptance/f10-capstone.sh` — lead-owned, read-only to the builder

Structure mirrors `orb-freeze-to-sow.sh` (§3.4) and inherits `p0.sh`'s git-context refusal
(§12.6). Emits, in addition to per-test `ok`/`FAIL` lines:

```
f10_measure: ms_speak_to_freeze=<n> ms_freeze_to_pr=<n> turns=<n>
f10_denominator: prs=<n> branches=<n> artifacts=<n> sows=<n>
```

### 5.3 `fleet/verify.sh` — one additive line, after the `orb-freeze-to-sow` stage (`:81`)

```sh
stage "f10-capstone"   required bash    "bash unavailable" bash tests/acceptance/f10-capstone.sh
```

---

## 6. The two tiers — and which one is the P0 exit trigger

This section exists because of `proxy-is-not-the-property`. `--agent stub` appends the literal
`// fleet stub change` to `main.rs` (`main.rs:3199-3207`). A suite that drives the full chain with
that agent proves **the chain is wired**. It does **not** prove **a module was built**. Both are
worth having; conflating them is the failure this section forbids.

### 6.1 Tier A — gated, deterministic, in `verify.sh`

Agent `stub`; `gh` shimmed; stub gateway for the LLM; throwaway git repos. Runs on every `verify`.
**Claim:** every hop H1–H7 is really connected, and every named failure path really refuses.
**Not a claim:** that a module was built. The driver stamps `tier=A agent=stub` into its own output
so no downstream reader can mistake one for the other.

### 6.2 Tier B — the actual P0 exit trigger, run once, evidence committed

Everything real: a **real gateway** (a real model turning a real typed/spoken description into a
brief — this is what "no hand-written spec" means), a **real builder agent** (`codex`), a **real
`gh`** against a real remote, a **real PR**.

**The Tier-B subject module is pinned here so the builder does not invent one:**

> **`fleet sow list` / `fleet sow show`** — a subcommand that lists the SOW records under
> `$FLEET_STATE/sows/` with each one's id, acceptance state, and origin (`--task` vs `--lld`).

Chosen because: it is **real** (F09's own contract records that these commands *"do not exist in
this tree"* — `F09-orb-fleet-handoff.md` §7.3, and that absence forced F09's script to assert
against raw JSON files); it is **small** (well inside the blueprint's ≤400 lines / ≤8 files
reference shape, `PHASES-TO-USABLE.md:85`); it was identified by a *prior* lane, not by F10, so
there is no circularity; and **F10's own machinery does not depend on it** — the driver reads the
record file directly, so a failed Tier B cannot cascade.

**The builder may not hand-write `fleet sow list`.** It must come out of the lane. If the builder
writes it by hand, F10's headline claim is false and the lane fails. (§11 restates this.)

Deliverable: `Light/docs/evidence/F10-p0-exit.md`, carrying — with real, pasted output —
the typed description, the turn count, the `freeze_id`/`sow_id`, the accept receipt, the
`artifact=` id, `fleet attest verify` output, the **real PR URL**, the diff stat, and both
measured durations.

### 6.3 The clause

**Tier A passing is not F10 done.** F10's done-definition (§10) requires Tier B's evidence file
with a real PR URL. Any report that cites Tier A's green as the P0 exit trigger is a
contract violation, and the verifier must fail the lane for it.

---

## 7. The acceptance suite — this IS the spec (T1/T2)

**Written by the lead before implementation. It is supposed to be red until the builder makes it
green.** `fleet/tests/acceptance/f10-capstone.sh` is **lead-owned and read-only to the builder
once written** — any diff touching it must be flagged. Every prior lane's tests
(`fleet/keel/fleet/tests/**`, `orb/backend/relay-py/tests/**`) and every fixture
(`fleet/contracts/fixtures/**`) are **read-only to this lane entirely**; the builder proves
non-modification with `git diff <merge-base> -- fleet/keel fleet/contracts orb/backend/relay-py`
being **empty** (§9).

Run: `bash fleet/tests/acceptance/f10-capstone.sh` (and via `bash fleet/verify.sh`).

| id | property | expected |
|---|---|---|
| **F10-T1** | **the whole chain, one process tree** | a typed module description → real relay turns → the freeze turn returns `freeze` non-null **and** `handoff.outcome == "sow_ready"` → real `fleet sow accept --id <sow_id>` exit `0` → driver → **one** `f10_lane:` line with `pr_url` non-empty. `f10_denominator: prs=1` asserted as **equality** |
| **F10-T2** | **the PR is real, four independent ways** — *a log line is not the property* | **(a)** the branch exists in the bare `origin` repo: `git -C <bare> rev-parse refs/heads/<branch>` exits 0; **(b)** the pushed commit's tree really contains the work: `git -C <bare> show <commit>:main.rs` contains `// fleet stub change` — **not merely that a branch exists**; **(c)** the `gh` shim's argv log records `pr create` with the parsed `--base` and `--head`; **(d)** `$FLEET_STATE/lifecycle/<safe_task_name>` reads exactly `Proposed` |
| **F10-T3** | **a real attestation, and it is *attached*** | `fleet attest verify <aid>` exits `0`; `$FLEET_STATE/attestations/<aid>.json` has **all 8** `REQUIRED` elements present (`lifecycle.rs:357-366`) **and** `predicate.elements.oracle_independence.distinct == true` — *not* the `{"status":"pending-adjudication"}` placeholder (`main.rs:1932`); **and** the PR body the shim captured via `--body-file` contains the artifact id and ≥3 of the 8 element names. *A PR with no attestation in it is a bare PR, which is not what the blueprint's exit names.* |
| **F10-T4** | **provenance chains all the way back — one chain, not two coincidences** | `sow.lld_provenance.freeze_id` == `$FLEET_STATE/freezes/<node_id>/1.json`'s `freeze.freeze_id` == the `handoff.freeze_id` the relay returned in T1; **and** identity is derived, not invented: re-run `fleet run` with the task text mutated by **one character** → refuses `SOW_NOT_ACCEPTED` (proving the id `enforce_accepted_sow` computes is `blake3` of *this exact* text, `sow.rs:25-27`) |
| **F10-T5** | **measured, with both clocks, and a do-nothing control** | `f10_measure:` prints `ms_speak_to_freeze` and `ms_freeze_to_pr`, both `> 0`, and `turns > 0`. **Control:** the driver run against an *unaccepted* SOW prints **no** duration field at all — never `0`, never a stale value (`clock-must-start-at-the-users-action`) |
| **F10-T6** | **"no hand-written spec" — census, not sample** | scan the driver and the acceptance script: **zero** occurrences of the compiled SOW grammar markers `Leaves:`, `Challenges:`, `Alternatives:`, `Edge cases:` (`sow.rs:346-361`) — the task text may reach `fleet run` **only** by being read out of the SOW record. Assert files-scanned `> 0` (`gate-census-not-sample`: measuring nothing is a failure) and assert the offending-file set **equals** the empty set |
| **F10-T7** | **empty module description** | POST a build turn with `text: ""` → the pinned status (assert whichever of `200`/`422` the merged relay actually returns, and pin it as a literal), the session **never** freezes, `handoff` is null, and `$FLEET_STATE/sows/` gains **zero** records |
| **F10-T8** | **a module that never reaches freeze** | drive turns with a vague/`NOT_READY` description (F05/F09's own construction) → never freezes; **zero** sows, **zero** artifacts, **zero** branches in origin. **And** the driver invoked with a non-existent sow id refuses `SOW_NOT_FOUND` |
| **F10-T9** | **an unaccepted SOW cannot produce a PR — the A15 gate holds** | freeze → SOW created → **skip** `fleet sow accept` → driver refuses `f10_lane_refused: code=SOW_NOT_ACCEPTED`, exits non-zero, prints the remedy, and origin has **zero** branches, `$FLEET_STATE/artifacts/` **zero** files. The driver never self-accepts |
| **F10-T10** | **a lane that fails mid-run reports a real failure, not a false PR** | point `--repo` at a real git repo with **no `main.rs`**, so the stub agent's `OpenOptions::open` genuinely fails (`main.rs:3201-3204`) → `fleet run` non-zero → `f10_lane_refused: code=RUN_FAILED` carrying fleet's own exit code → **zero** `f10_lane:` lines, **zero** branches, **zero** PRs |
| **F10-T11** | **a run that lands no work cannot produce a PR** | drive `--agent env-probe` (or a repo state where the append yields an identical diff) → the kernel's `NO_WORK_LANDED` / `EMPTY_DIFF` refusal is reached → no PR, and the driver's refusal code names it |
| **F10-T12** | **a tampered artifact cannot produce a PR** | after a green T1, overwrite one byte of `$FLEET_STATE/artifacts/<aid>`, re-drive only the pr-emit hop → `pr_emit_refused: code=ARTIFACT_DIGEST_MISMATCH` (`main.rs:5311-5316`), and origin still has exactly **one** branch |
| **F10-T13** | **an incomplete attestation cannot produce a PR** | delete one required element (`rollback`) from the attestation JSON → `pr_emit_refused: code=INCOMPLETE_ATTESTATION missing=rollback` (`main.rs:5346-5352`), no new PR |
| **F10-T14** | **idempotence — no double PR** | run the driver twice on the same accepted SOW → the second refuses (lifecycle is now `Proposed`, not `Accepted` → `NOT_ACCEPTED`, `main.rs:5283-5289`) and `f10_denominator: branches=1` asserted as **equality**, not `>= 1` |

**Fixtures** are built in a `mktemp -d` tempdir or in memory, never by editing
`fleet/contracts/fixtures/**`.

---

## 8. Mandatory mutation battery

Green tests prove nothing about a test's power. Each mutation is applied to the **real**
implementation (never to a fixture), shown to turn the **named** test red **with the predicted
failure**, then reverted with `git diff` proven empty. F05's lane found **three of eleven**
mutations initially passed for the wrong reason, and F09's builder found **two** table
corrections by measuring: **a green mutation on the first attempt is a finding, not a pass** —
write it up.

M1, M6, M7, M8 and M10 touch Rust that this lane otherwise may not modify (§9). That is
sanctioned **only** as a reverted mutation probe, and the revert must be proven byte-empty.

| # | mutation | must turn red |
|---|---|---|
| **M1** | in `app.py`, delete the `handoff.hand_off(brief)` call (`:1133`), leaving `freeze_proposal` built | **F10-T1** (`handoff.outcome` never `sow_ready`) **and F10-T4** (no provenance to chain) |
| **M2** | in the driver, replace the `fleet run` invocation with `true` and print `f10_lane:` with a fabricated 64-hex artifact id | **F10-T2(b)** (origin's commit has no marker), **F10-T3** (no attestation exists), **F10-T12**. *§4(b)'s "fake a stage" alternative, applied as an attack — the single most important mutation in this lane* |
| **M3** | in the driver, drop the `fleet pr emit` call and print a hardcoded `pr_url=https://github.test/…` | **F10-T2(a)** (no branch in origin) **and F10-T2(c)** (no `gh` argv log). *Proves the PR assertion is not a stdout read* |
| **M4** | in the driver, self-accept the SOW when it finds `accepted: null` | **F10-T9** |
| **M5** | in the driver, treat any `fleet run` exit code as success | **F10-T10 and F10-T11** |
| **M6** | in `main.rs`, drop `pr_emit_run`'s `persisted != "Accepted"` check (`:5283`) | **F10-T14** (a second PR becomes possible) |
| **M7** | in `main.rs`, drop `pr_emit_run`'s `bundle.missing_element()` check (`:5346`) | **F10-T13** |
| **M8** | in `main.rs`, remove the `blake3_hex(&diff) != artifact_id` check (`:5311`) | **F10-T12** |
| **M9** | in the driver, hardcode `ms_freeze_to_pr=0`, and separately print it on the refusal path too | **F10-T5** (both halves: the `> 0` assertion *and* the do-nothing control) |
| **M10** | in `main.rs`, make `enforce_accepted_sow` always return `Ok(())` (`:685-711`) | **F10-T9 must still fail** — at the driver's own gate — **and** a sub-case invoking `fleet run` **directly** with an unaccepted task must go red. *Pins that the A15 gate is keel's, not merely the driver's: two independent refusals, so removing either one is caught* |

---

## 9. Files owned (no overlap with any other lane)

| path | action |
|---|---|
| `fleet/bin/f10-lane-drive.sh` | **new** — the driver (§5.1) |
| `fleet/tests/acceptance/f10-capstone.sh` | **new** — lead-owned, read-only to builder once written |
| `fleet/verify.sh` | edit, **additive only** — one `stage` line after `:81` |
| `Light/docs/evidence/F10-p0-exit.md` | **new** — Tier B evidence (§6.2) |
| `Light/status/F10-P0-capstone-integration.status` | the builder's own status file |

### The hard boundary, as a checkable gate

**F10 writes zero lines of Rust and zero lines of Python.** F10's own FEATURES row says
"reuse (kernel)"; §3 shows nothing in the kernel needs to change. The done-definition requires:

```
git diff <merge-base> --stat -- fleet/keel orb/backend/relay-py fleet/contracts
# must be EMPTY
```

If the builder concludes that a Rust or Python change **is** required, that is a **finding to
escalate to the lead**, not a change to make. Say what you found and stop.

---

## 10. Done-definition

1. `bash fleet/tests/acceptance/f10-capstone.sh` — all 14 tests green, with the real
   `f10_denominator:` and `f10_measure:` lines pasted into the report.
2. `bash fleet/verify.sh` run and its **real** output pasted, reds included. The three
   pre-existing reds (`recur`, `semgrep`, `trivy` — missing `bin/*-gate.sh` in a fresh worktree)
   are known and out of scope; name them, do not fix them.
3. The orb relay-py suite run **standalone** — `cd orb/backend/relay-py && .venv/bin/pytest -q` —
   because `verify.sh`'s `pytest` stage runs `crew`'s suite via `uv`, **not** this one (§12.8).
   No regression against F09's 500 passed.
4. All **10** mutations applied to real code, each shown red on its named test with the predicted
   failure, each reverted with `git diff` proven empty. Any first-attempt-green mutation written
   up as a finding.
5. `git diff <merge-base> --stat -- fleet/keel orb/backend/relay-py fleet/contracts` **empty** (§9).
6. **`Light/docs/evidence/F10-p0-exit.md` exists and carries a real PR URL**, both measured
   durations, and the real diff stat — Tier B (§6.2). **Tier A green alone does not close F10**
   (§6.3).
7. `Light/status/F10-P0-capstone-integration.status` → `STATUS=verifying`; `bash Light/status/render.sh`.
8. A dated entry appended to `/Users/rachitsrivastava/youtube/Principal Engineering/FLEET-LEARNINGS.md`
   (the repo-root file — **never** a second copy under `Light/`).
9. A PR opened on `lane/F10-capstone`. **Stop there** (§13).

---

## 11. Out of scope — do not fold these in

- **F11 (thin status echo).** F10's driver prints to stdout. It must add **no** relay response
  field, **no** conversational text, and **no** ledger-sourced status surface. F11 owns that.
- **F12 (freeze ledger as a session object).** No `conversation_store` change.
- **Fixing F08's multi-line-title landmine** (§12.2). **Measure and disclose it; do not patch it.**
- **The `HumanApproval::recorded("run accepted")` internal token** (`lifecycle.rs:1144`). It is an
  in-crate token, not a claim of a second human review — the real human gate is H4, and the real
  final gate is a human merging the PR. Kernel unchanged; disclose in the report, change nothing.
- **Model routing into `run_with_evidence`**, and **semgrep/trivy into the lane** — both are P1
  (`PHASES-TO-USABLE.md:100-106`).
- **Any autonomous SOW acceptance**, and any removal or weakening of `enforce_accepted_sow` (§3.2).
- **Hand-writing `fleet sow list`.** It is Tier B's *subject*, produced *by* the lane. Writing it
  by hand falsifies F10's headline claim (§6.2).
- **`FLEET_SOW_BYPASS=1`** in any tier (§3.2).
- Editing `orb-freeze-to-sow.sh`, `p0.sh`, or any prior lane's test (§7).

---

## 12. Landmines (each one already cost someone a session)

**12.1 The compiled task text is multi-line, so the default PR branch name is invalid.**
`pr_emit_run`'s default head is `format!("fleet/{task}")` (`main.rs:5374`), and the compiled SOW
text is newline-joined (`sow.rs:346-362`). `git push -u origin "fleet/Task: …\nLeaves:…"` cannot
work. **The driver must always pass `--head` explicitly** (the flag exists, `main.rs:5236`).
Default: `fleet/f10-<sow_id[0:12]>`.

**12.2 The PR *title* has the same problem and there is no flag for it.**
`title = format!("fleet: {task}")` (`main.rs:5375`) is not overridable — `pr_emit_command` parses
`--task/--artifact/--repo/--base/--head` only (`main.rs:5231-5236`). Tier A's `gh` shim will
swallow a multi-line title silently. **A real `gh pr create` may not.** Tier B must *measure* this
and record the exact result. If real `gh` mangles or rejects it, that is a **real finding and a
genuine P0-exit blocker** — report it with the verbatim `gh` error and stop. **Do not work around
it, and do not patch F08.**

**12.3 `fleet sow accept` and `fleet run` must share one `$FLEET_STATE`.** The gate looks for the
accept receipt in *that* state's ledger (`main.rs:692-709`). A driver that makes its own tempdir
fails here, silently and confusingly.

**12.4 Backgrounding a process inside command substitution hangs the script.**
F09's own scar, in that script's header (`orb-freeze-to-sow.sh:13-17`): a function that both
backgrounds a job and is called as `x="$(f)"` makes the substitution wait on descriptors the
grandchild inherited — the relay answered `curl` in 30ms by hand while the script produced zero
output for 5+ minutes. **Start the relay and gateway directly in the script's own shell.**

**12.5 `set -u` and never `$?` after a pipe.** `verify.sh:20`, E1/S11. Capture status directly.

**12.6 Copy `p0.sh`'s git-context refusal verbatim.** `p0.sh:9-15` refuses to run when `GIT_DIR`,
`GIT_INDEX_FILE`, or `GIT_WORK_TREE` is set. This is not defensive theatre: that exact situation
once wrote a bogus commit into fleet-rs and **deleted 218 files from HEAD** (recovered via reflog).
F10 creates throwaway repos and pushes to them. **Copy the guard.**

**12.7 `--agent stub` requires a `main.rs` in the target repo** (`main.rs:3200`). Tier A's fixture
repo must have one (`p0.sh:38` shows the setup); F10-T10 deliberately omits it, which is *how* T10
produces a real mid-run failure rather than a simulated one.

**12.8 The orb relay-py suite is not in `verify.sh`.** Its `pytest` stage runs `crew`'s Python
suite via `uv` (`verify.sh:118`). "verify.sh's pytest stage passed" **never** means the relay
suite ran. Run it standalone (§10.3).

**12.9 `fleet sow list` / `fleet sow show` do not exist yet.** Do not write an assertion that calls
them — F09's script had to read raw JSON for exactly this reason. (They are Tier B's subject.)

**12.10 `CARGO_TARGET_DIR`.** Export the shared external cache before building, or the corpus's M2
detector trips on `keel/target` exceeding 15000 files (FLEET-LEARNINGS, B8). `FLEET_BIN` is derived
from it in both `verify.sh` and F09's script (`orb-freeze-to-sow.sh:23`).

**12.11 A peer may commit to your worktree.** Twice in this build a background agent has written
into a lane's tree (FLEET-LEARNINGS 2026-09-02, and `check-for-live-writers-before-ab`). Re-check
`git status` and `git log` before concluding anything about a diff you did not make.

---

## 13. Routing and merge policy

| role | model | why |
|---|---|---|
| **builder** | `mid-engineer` (Sonnet) | cross-language integration, real process orchestration, and debugging a 7-hop chain — logic and integration work, not mechanical scaffolding (A17) |
| **verifier** | `verifier` (Sonnet, **never** Haiku) | independent PASS/FAIL, re-driven from scratch in its own worktree |

**Sizing:** one focused session. The suite is 14 tests but the driver is ~120 lines of shell over
existing commands, and §3.4/§3.5 mean both halves of the harness already exist to copy. If Tier B
(§6.2) blocks on a real `gh`/quota problem, **split**: land Tier A + the driver, mark F10
`STATUS=blocked` with the exact blocker, and do **not** report the lane done.

**Merge:** **PR and stop.** A15/D4. F10 is the P0 exit trigger and it adds a driver that invokes
the lane kernel — a human reads it. Additionally, **Tier B's own PR (the `fleet sow list` module)
is a deliverable and must not be auto-merged**; it is the artifact the P0 claim rests on, and its
whole point is that a human reviews something they never specified in writing.

**Verifier's two highest-value probes**, given what this estate keeps getting wrong:

1. **Drive the runtime, don't read the suite.** Run the driver by hand against a fresh state and
   confirm a `pr_url` you can `git rev-parse` in the bare origin. Then re-run the *old* binary or
   a neutered driver and confirm the suite goes red — a suite that cannot fail measured nothing.
2. **Grep for callers.** Confirm `f10-lane-drive.sh` is invoked by `f10-capstone.sh` and that
   `f10-capstone.sh` is reached by `verify.sh` — the ADHD-Orb review's exact finding was an
   excellent contract layer with **no composition layer**, green gate notwithstanding.
