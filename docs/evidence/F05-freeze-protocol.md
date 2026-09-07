# F05 — Freeze protocol: propose → pushback → ❄ — evidence

Builder: mid-engineer (dispatched). Contract: `docs/lane-contracts/F05-freeze-protocol.md`.
Reproduce from a fresh clone of `lane/F05-freeze-protocol`; every command below is literal, run from
the worktree root unless noted.

## 1. Red-on-arrival (Done-definition item 2)

Per §7's rule ("prove red at the lane base, in a disposable `git worktree add --detach`, never
`git stash`"): the three F05 test files were copied into a disposable detached worktree at the
lane's base commit (`ce6e2e3`, the last commit before any F05 code existed), and run there with
`PYTHONPATH` pointed at that worktree's own `src/` (so the editable install could not smuggle in
this worktree's implementation).

```
$ git worktree add --detach /tmp/f05-red-check ce6e2e3
$ mkdir -p /tmp/f05-red-check/orb/backend/relay-py/tests
$ cp orb/backend/relay-py/tests/test_f05_*.py /tmp/f05-red-check/orb/backend/relay-py/tests/
$ cd /tmp/f05-red-check/orb/backend/relay-py
$ PYTHONPATH=/tmp/f05-red-check/orb/backend/relay-py/src <venv>/bin/python -m pytest -q \
    tests/test_f05_freeze_protocol.py tests/test_f05_readiness_gate.py tests/test_f05_convergence.py
```

Result:
- `test_f05_freeze_protocol.py`: **ERROR collecting** — `ImportError: cannot import name
  'freeze_protocol' from 'orb_relay.build'` (34 test cases never ran, correctly, because the module
  did not exist).
- `test_f05_readiness_gate.py`: **ERROR collecting** — `ImportError: cannot import name 'readiness'
  from 'orb_relay.build'` (7 test cases never ran).
- `test_f05_convergence.py`: **7 skipped** — `fleet/` is not part of the git-tracked tree (confirmed:
  `ls /tmp/f05-red-check/fleet/keel/target/debug/fleet` → no such file), so the module-level
  `pytestmark = pytest.mark.skipif(not _REAL_FLEET_BIN.exists(), ...)` fired honestly. A skip, not a
  false green — no test in this file was ever counted as passing at the base.

No test was "green at base." Worktree removed afterward (`git worktree remove --force
/tmp/f05-red-check`).

## 2. Baseline and final counts (Done-definition item 3)

```
$ cd orb/backend/relay-py && python3 -m venv .venv && .venv/bin/pip install --upgrade pip \
    && .venv/bin/pip install -e ".[dev]"
$ .venv/bin/pytest -q        # BEFORE any F05 file existed
444 passed, 5 warnings in 3.11s
```

```
$ .venv/bin/pytest -q        # AFTER the full F05 implementation + test suite
492 passed, 5 warnings in ~9s
```

**444 → 492: +48 new passing tests, zero regressions.** Breakdown: T1–T4/T6 = 34 (`test_f05_freeze_
protocol.py`, including one test added past the contract's own T1.8 — see §5), T5 = 7 (`test_f05_
readiness_gate.py`), T7 = 7 (`test_f05_convergence.py`, T7.1–T7.3 combined into one case, plus T7.9,
a case added past the contract's own T7.1–T7.8 — see §4 M9).

## 3. Manual end-to-end drive (§6), driven for real

Two real OS processes throughout: a disposable stateful stub gateway
(`http.server.ThreadingHTTPServer`, standing in for the model, matching §6's own instruction not to
reuse `tests/manual/stub_gateway.py`) and the real `orb_relay.app:app` under `uvicorn`, with the real
`FLEET_BIN` pointed at `fleet/keel/target/debug/fleet` (built via `cargo build --manifest-path
fleet/keel/Cargo.toml`). Driven by real `curl`, not `TestClient`.

**5a — warmup.** `mode=build` → `"opening": "Build mode. What are we making?"` (F04, unchanged).

**5b — residual_ambiguity FALLS turn over turn** (not a static number), scripted decompose replying
with `complete_module.json` and user text mentioning "database... for storage" so `data_owned`
(empty in that fixture) gets covered via the independent turn-evidence path:

```
TURN 1: residual_ambiguity=0.4683444517819128  move=ask    freeze=null
TURN 2: residual_ambiguity=0.08265217886566592 move=freeze freeze=NOT null
TURN 3: residual_ambiguity=0.0                 move=freeze freeze=NOT null
```

**5c — the freeze turn's real body** (`/v1/respond`, turn 2, fresh session `s-manual-2`):

```json
"freeze": {
  "proposed_by": "orb:dialogue",
  "node_id": "lld-v1-schema",
  "content_hash": "sha256:1b70c4b402ba13413865856e140403b93bc911cc73977a64d506fcb6a0bbc8b0...",
  "readiness": {"outcome": "ready", "exit_code": 0,
    "detail": "gate lld-ready: READY (...) -- 14/14 checks passed"},
  "clarify_turns_used": 1,
  "residual_ambiguity": 0.08265217886566592
}
```

Structural forgery check: a raw `grep -c stamped_by` over this exact response body returns **1** —
but that hit is inside a `guarantees[].claim` STRING (`"ModuleBrief has no content_hash/freeze_id/
depth_evidence/stamped_by/state/version field"`), i.e. prose describing the schema's own rule, not a
JSON key. A recursive KEY scan (matching `test_t7_2`'s actual assertion, and re-run standalone
against this exact captured body) confirms: **`stamped_by` is never a JSON key anywhere in the
body** (81 distinct keys total, none named `stamped_by`). Recorded here because the contract's own
§6 step 5c literal suggestion (`grep -c stamped_by == 0`) is a proxy that gives a false positive on
this real body — the precise property (no forged key) is what actually holds, and what T7.2
actually checks.

**5d — the negative drive, 14 real turns, decompose always garbage** (forces `clarify_request` every
time):

```
turns 1-11:  move=ask   freeze=null  HTTP 200
turns 12-14: move=split freeze=null  HTTP 200
```

Never freezes, never loops forever, every turn 200. (The ask→split boundary lands at turn 11→12,
one turn earlier than the pure-function unit test's turn 12 boundary — see §5's disclosed
wiring-level approximation.)

**5e — kill the fleet binary mid-session (the step the contract calls out as "the one to actually
perform").** A disposable copy of the keel binary, `chmod -x`'d between turn 1 and turn 2 of a
session that would otherwise freeze at turn 2 (same script as 5b/5c):

```
turn 1 (binary executable):     move=ask,   freeze=null
[chmod -x /tmp/f05-manual-fleet-copy]
turn 2 (binary NOT executable, the turn that would have frozen): HTTP 200
    freeze: None
    move: ask
    blocking: ['DEPTH']
turn 3: HTTP_STATUS=200 (confirmed explicitly)
```

`UNAVAILABLE` refuses — never assumes ready, never silently freezes on stale/missing tooling.

**5f — focus mode untouched, same live server:** `mode=focus` on the very server that had just
handled BUILD-mode freeze turns → `build`, `decompose`, `proposal`, `freeze` all `null`; every
pre-F05 field (`text`, `beats`, `degraded`, `spent_paise`, ...) present and unchanged.

## 4. T9 mutation adequacy — all 11, real output, reverted via `git checkout --` against a checkpoint
   commit (never `git stash`, per landmine L3)

| # | Mutation | Predicted victim(s) | Actual result |
|---|---|---|---|
| M1 | `EPSILON_GAIN` 0.05→0.03 | T2.2 | **Broke T2.2** exactly: `keep_asking` flips to `True` (`assert True is False`) at the freeze point. |
| M2 | `THETA_COV` 0.80→0.81 | T2.2 boundary | **Broke T2.2**: `coverage_ok` flips to `False` at the 0.8 boundary (`assert False is True`) — catches `>` vs `>=`. |
| M3 | `SLOT_WEIGHT[NON_GOALS]` 0.2→0.6 | T1.3, T2.3, T2.4 | **Broke all three** exactly as named. |
| M4 | `RHO_CONFIRM` 1.0→0.5 | T3.5 only (predicted) | **Broke T3.5, T3.6, AND T4.2** — wider than predicted. Finding: T3.6/T4.2 assert the exact escalated EIG value (not just "still an Ask"), which is a more faithful reading of the contract's own T3.6 spec ("Move.Ask **with the T3.5 candidate**"); a weaker T3.6 would have missed this. |
| M5 | `continue_dialogue` → `keep_asking(...)` | T3.6, T3.7, T4.4, T4.5 | First attempt: **only T3.6 broke** — `next_move` inlined the split/ask condition instead of calling `continue_dialogue`, so the mutation had nothing to propagate through. **Fixed the implementation** (routed `next_move` through `continue_dialogue` explicitly — logically identical, now actually dependent) and re-ran: **T3.6, T3.7, T4.4 all broke** as predicted; T4.5 (a totality/no-raise sweep) still does not, since a `keep_asking`-gated `continue_dialogue` is wrong but still well-typed. Disclosed, not hidden. |
| M6 | `SPLIT_TRIGGER_TURNS` 12→1000 | T4.4, T7.4 | **Broke both.** T4.4 directly (`assert 1000 == 12`). T7.4 via a 500: the defensive `assert MAX_CONVERSATION_TURNS >= 2 * SPLIT_TRIGGER_TURNS` added in `app.py` (see §5) fired loudly, which is itself the intended fail-closed behavior. |
| M7 | `REQUIRED_SLOTS` widened to all six | T2.5 | First attempt: **T2.5 stayed green** — its own state-construction loop iterated the SAME mutated `REQUIRED_SLOTS`, so the input and the expectation moved together. **Fixed the test** (hardcoded the five intended slots as `_THE_FIVE_REQUIRED_SLOTS`, added `test_t1_9` to pin the real constant against it) and re-ran: **T1.9 and T2.5 both broke** as intended. |
| M8 | `ReadinessVerdict.is_pass()` → `outcome is not NOT_READY` | T5.4 | **Broke T5.4** exactly (`UNAVAILABLE` would silently pass). |
| M9 | `depth_pass` hardcoded `True` in `app.py` | T7.4 (predicted) | **T7.4 did not break** — its scenario never reaches a covered-but-not-depth-passed state (coverage is never satisfied there either way). Real gap found: added **T7.9**, a schema-valid/keel-invalid (bad owner) brief driven 8 real turns; confirmed green on the real implementation, then **confirmed red under M9** (froze at turn 2 on a brief keel would reject). |
| M10 | `coverage_ok` drops the `value_s >= MIN_SLOT_VALUE` term | T6.1 | First attempt: **T6.1 stayed green** — it only drove the one slot under test via negative evidence, leaving the other four required slots uncovered for the ordinary reason, so `coverage_ok` was already `False` without the guard ever being exercised. **Fixed the test** (independently covered the other four slots with positive evidence first) and re-ran: **broke** as intended (`assert True is False`). |
| M11 | `FreezeProposal.proposed_by` → `"keel:lld-ready"` | T7.2 | **Broke** — even more strongly than a value mismatch: pydantic's own `Literal["orb:dialogue"]` type refuses the forged string at construction time (`ValidationError: Input should be 'orb:dialogue'`), turning the whole turn into a 500 before T7.2's specific assertion is even reached. |

Net: **11/11 mutations produced a real, observed failure signal.** Three (M5, M7, M10) required a
genuine fix to close a gap the mutation itself exposed (two test-isolation bugs, one implementation
refactor); one (M9) required a new test to close a coverage gap; one (M4) broke wider than predicted
for a defensible reason. Every fix/addition is committed as its own commit with the real before/after
output in the commit message. **491/492 tests pass after all reverts confirmed clean** (`git diff`
against the last commit shows no leftover mutation).

## 5. Disclosed judgment calls and known gaps

1. **`clarify_turns`'s wiring-level approximation.** `app.py` derives it from `_conversation_store`
   (a persisted, per-session log), NOT a new counter. That store **physically prunes** rows past
   `MAX_CONVERSATION_TURNS` (24 messages / 12 turns) on every append — discovered directly when T7.4
   went red past turn 12 (the counted value froze at 11 forever, which is the exact unbounded-loop
   shape this lane exists to rule out). Fixed: once the session hits the cap, report
   `SPLIT_TRIGGER_TURNS` itself — a safe, proven lower bound, guarded by an explicit assertion
   (`MAX_CONVERSATION_TURNS >= 2 * SPLIT_TRIGGER_TURNS`) that fails loudly if the two constants are
   ever changed independently (this is exactly what M6 exercised). **Disclosed cost:** a real session
   that reaches the cap on its 12th turn is reported as already at the split ceiling one turn early
   (turn 11→12 boundary instead of 12→13). `next_move`'s own pure-function contract (T4.4) is
   unaffected; only this app.py-level derivation carries the approximation. A future lane wanting
   exact precision past turn 12 needs a dedicated, non-pruning counter (out of this lane's scope —
   the registry table forbids a new evidence table, and `ConversationStore`/`BuildSessionStore` are
   both "install, read-only").
2. **`ambiguity_history` is always passed as `()` from `app.py`.** `escalation_armed`'s reason (i)
   (the §3.1 stall) is fully wired end-to-end and proven at the wire level (T7.5). Reason (ii) (three
   consecutive small ambiguity deltas) is proven only at the pure-function level (T3.8) — wiring it
   for real requires a persisted, per-turn history of `residual_ambiguity` snapshots, which (like
   item 1) would need new persisted state this lane's interface does not provide for. Flagged
   forward, not silently absorbed.
3. **`ConversationResponse.proposal` is always `None`.** The wire type (`ProposalTurn`) and its
   structural guards are shipped and tested (T6.3/T6.4), but nothing in this lane's `app.py`
   integration ever constructs one — `next_move` has no `Propose` variant, and §4.4's own algorithm
   steps never build a `ProposalTurn`. Reserved for whichever lane implements the propose/pushback
   dialogue mechanic explicitly.
4. Everything in the lane contract's own §8.2 (out of scope) and §9 (flagged forward — no keel
   producer of a stamped `Freeze` exists anywhere; F12 owns freeze persistence; the depth rubric
   stays keel's own).

## 6. `fleet/verify.sh` — real exit, pre-existing failures traced disjoint

```
$ bash fleet/verify.sh
...
-- 15 passed, 8 failed, 1 skipped (denominator: 24 stages) --
$ echo $?
6
```

`git diff --stat master` (the change this lane actually makes) touches exactly 8 files, all under
`orb/backend/relay-py/` or `docs/`:

```
 docs/lane-contracts/F05-freeze-protocol.md         | 1023 ++
 orb/backend/relay-py/src/orb_relay/app.py          |  183 +-
 .../src/orb_relay/build/freeze_protocol.py         |  339 ++
 .../relay-py/src/orb_relay/build/readiness.py      |  151 ++
 .../relay-py/src/orb_relay/proxy/schemas.py        |  100 +-
 orb/backend/relay-py/tests/test_f05_convergence.py |  467 ++
 .../relay-py/tests/test_f05_freeze_protocol.py     |  594 ++
 .../relay-py/tests/test_f05_readiness_gate.py      |  150 ++
```

T8's own untouched-file command (`git diff --name-only master -- cognitive/ store/ lld_schemas.py
conversation_guard.py atomizer.py lld_decomposer.py fleet/ orb/apps/ tests/test_f0[1-4]*.py`) prints
**nothing**. `orb/backend/relay-py/src/orb_relay/build/build_session.py` (F04's own file, "install,
read-only") also shows an empty diff, confirmed separately.

Every one of the 8 failing stages traces to a file this lane never touched:

| Stage | File : line | Disjoint from F05 because |
|---|---|---|
| fmt | `fleet/keel/fleet/src/lifecycle.rs:458,507,1275,1287` | Rust file under `fleet/keel/`; F05 touches zero files there. |
| clippy | `fleet/keel/fleet/src/main.rs:5269,5296,5305,5328,5391` (5 `manual_inspect` warnings) | Same. |
| unit tests | `fleet/tests/f08_pr_emit.rs:167` — `probe exited Some(3)`, "could not freeze artifact" | An F08 (PR-emit) integration test, unrelated lane; **F06's own verifier note in STATUS.md independently documents this exact test as "intermittent, hit only [under] coverage[,] not unit-tests"** — a pre-existing, previously-documented flake. |
| secrets | `gitleaks detect -s .` run from `fleet/` (scans that whole tree, not `orb/`) | F05's changed files are not under `fleet/` at all; the scan's own scope excludes them by construction. |
| lld-crosslang | `MIRROR FAILED: ts` (the `apps/mobile/src/build/lld-v1.ts` mirror vs. the Rust/Python mirrors) | `orb/apps/` shows an empty diff (T8, above); F05 never touches `lld_schemas.py` or the TS mirror. |
| semgrep | `registry-reference/registry/features/memory/memory_store.py:184` (dangerous-subprocess-tainted-env-args) | Not a path under `orb/backend/relay-py/`; untouched. |
| coverage | `fleet/src/repl.rs:1020` — `repl::tests::completed_child_waits_for_stream_eof_before_dropping_tail` panicked (`cargo llvm-cov` re-run surfaces a DIFFERENT flaky test than the plain unit-test pass, same class as the f08 flake above) | Rust file under `fleet/`, untouched. |
| corpus | `fleet/registry-reference/registry/features/ledger/ledger.sh:26` (`bash fleet/bin/corpus.sh`'s own scoring against `fleet/PRINCIPLES.md`) | Not under `orb/`; untouched. |

**Cross-confirmation:** F06's and F07's own verifier/builder notes (visible in `Light/STATUS.md`,
written independently, before this lane started) describe this *exact same set* of pre-existing
failures (fmt/clippy/f08-flake/secrets/semgrep/corpus) as already known and already traced disjoint
in their own runs — this is not a new discovery specific to F05.

**Not re-run a second time** (a full `fleet/verify.sh` pass took ~25 minutes on this machine,
dominated by a from-scratch `cargo llvm-cov` compile of the whole dependency tree); the single real
run's output is captured above and in `var/verify.log` at the time of this writing.
