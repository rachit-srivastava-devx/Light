# F03 — Module-brief atomizer — evidence

Builder: mid-engineer. Worktree: `Light/.worktrees/F03-atomizer`, branch `lane/F03-atomizer`, lane
base `6fd66f3` (`git merge-base HEAD master`, confirmed clean tree at contract time). All commands
below were run with `cd orb/backend/relay-py` inside that exact worktree unless stated otherwise.
Every server started for the manual drive (§5) was started with an explicit background job, driven
with separate foreground `curl`s, and then explicitly killed by PID — never backgrounded-and-waited.

## 0. C1 verdict, restated

**install** (decode→validate→repair-once→fail-closed emitting `ModuleBrief`, the `ModuleBrief` type
+ validator, the detailed validator, `mode=build` on the wire, the build-mode branch in
`/v1/respond`, the append-only build-session store, Tier-0 evidence/register fold, both fixtures) +
one bounded **extract** (the reserve→call→settle/release envelope) + **zero build_new**. Confirmed
by the two probes in the lane contract §0.1 (grep for callers of `lld_decomposer.decompose` found
none in `src/`; `app.py` at the lane base imports only `atomize`, never `decompose`).

## 1. Files touched — real diff, not an estimate

```
$ git diff --numstat 6fd66f3...HEAD
147  29  orb/backend/relay-py/src/orb_relay/app.py
 64   0  orb/backend/relay-py/src/orb_relay/build/build_session.py
 34   2  orb/backend/relay-py/src/orb_relay/proxy/lld_decomposer.py
 18   1  orb/backend/relay-py/src/orb_relay/proxy/schemas.py
475   0  orb/backend/relay-py/tests/test_f03_module_brief_decompose.py
707   0  docs/lane-contracts/F03-module-brief-atomizer.md   (the lead's contract doc, committed alongside)
```

**Real net-new production line count: 231** (`(147-29) + (64-0) + (34-2) + (18-1)` across
`app.py`/`build_session.py`/`lld_decomposer.py`/`schemas.py`). Acceptance-test code is a separate
475 lines (`test_f03_module_brief_decompose.py`, all new). No file outside these four production
files plus the one new test file was touched — exactly the "five files, no file shared with any
other open lane" the contract's §3 preamble states.

### §3.5's mechanically-checked untouched list — prints nothing, run against the committed hash

```
$ git diff --numstat 6fd66f3...HEAD -- \
    orb/backend/relay-py/src/orb_relay/proxy/atomizer.py \
    orb/backend/relay-py/src/orb_relay/proxy/semantic_checks.py \
    orb/backend/relay-py/src/orb_relay/proxy/lld_schemas.py \
    orb/backend/relay-py/src/orb_relay/cognitive/belief.py \
    orb/backend/relay-py/src/orb_relay/store/build_session_store.py \
    orb/backend/relay-py/src/orb_relay/store/freeze_store.py \
    orb/backend/relay-py/src/orb_relay/eval/ \
    orb/domain/agents/focus-companion.v1.md orb/domain/agents/converse.v1.md \
    orb/domain/agents/teach.v1.md orb/domain/agents/build.v1.md \
    orb/apps/mobile/ orb/backend/relay-rs/ apps/macos/ fleet/ registry/
(no output — exit 0)
```

## 2. What changed, precisely

1. **`proxy/lld_decomposer.py`** (§0.2/§3.1 fix): `_parse_and_validate`'s failure path now calls a
   new `_rejection_detail_from_errors`, which runs `validate_module_brief_detailed` and renders
   `"<path>: <message>"` per error, joined with `"; "`, capped at a new named constant
   `MAX_REJECTION_DETAIL_CHARS = 400`. Falls back to the old constant string only if the detailed
   validator finds nothing to name (a genuine validator disagreement — not observed in practice,
   but handled rather than emitting an empty string). The success path is untouched
   (`validate_module_brief` still produces the typed object). `MAX_REPAIR_ATTEMPTS`,
   `DECOMPOSER_MAX_TOKENS`, `DecomposeKind`, `DEFAULT_MISSING_ON_DECOMPOSE_FAILURE`, the
   `clarify_request` fail-closed branch, `SYSTEM_PROMPT`, and `GatewayError`-propagates are all
   unchanged, as the contract requires.
2. **`build/build_session.py`** (§3.3, the one genuinely new logic): added
   `derive_brief_evidence(brief: ModuleBrief) -> list[Evidence]`, a field read over an
   already-validated brief mapping `interface`/`data_owned`/`deps`/`non_goals` (conditionally,
   non-empty) and `acceptance`/`registry`/`alternatives`/`guarantees` (always, schema-required) to
   `Coverage`/`Alternatives`/`Depth` evidence, all `EvidenceTier.TIER0` at the existing
   `_TURN_EVIDENCE_WEIGHT`/`_TURN_EVIDENCE_RELIABILITY` scale. `derive_turn_evidence` is untouched;
   the two coexist per §3.3 constraint 1. Never emits `Contradiction` (constraint 2). This module
   is not firewalled against importing `lld_schemas` the way `cognitive/belief.py` is —
   `tests/test_f04_belief_registers.py`'s A5 AST check targets `belief.py` specifically, confirmed
   by reading that test before relying on it.
3. **`proxy/schemas.py`** (additive, the human-merge trigger): new `DecomposeOutcome` model
   (`kind`, `brief: ModuleBrief | None`, `missing`, `rejections`), and
   `ConversationResponse.decompose: DecomposeOutcome | None = None`. No existing field's type,
   optionality, or presence changed.
4. **`app.py`** (additive, inside F04's existing `if body.mode is ResponseMode.BUILD:` block):
   - New module-level `_reserve_run_settle(meter, reservation, line, awaitable)` — the reserve→
     call→settle-on-success/release-on-any-exception envelope, extracted from what was three
     duplicated copies of this shape. It does **not** call `reserve_remaining()` itself, so each
     call site's own admission-refusal (402) `dev_log`/`HTTPException` code is untouched, only the
     call+settle/release body moved into one place. `/v1/atomize` and `/v1/respond`'s existing
     guarded-completion call site were both refactored to call it — see §3 below for the proof
     this changed no observable behavior at either site.
   - The BUILD branch now additionally: derives `goal_text` (the session's accumulated **user**
     turns, joined) by re-deriving through the same `_conversation_messages` helper the
     conversational call already uses — not a second, hand-rolled accumulation — over
     `_conversation_store.recent(...)`, called **after** this turn's own append, so it correctly
     includes the current turn without any carry-over ambiguity; reserves budget for a `decompose`
     call through the same envelope; on `GatewayError` specifically, degrades to a null decompose
     result (`dev_log` at warn) without failing the turn (§3.2's disclosed judgment call — the
     spoken reply has already landed); on `ReservationExceededError` (either the pre-call
     admission check or the post-call settle safety net), propagates a 402 for the whole turn,
     same as every other admission-refusal path in this file (§3.2 does not carve out an
     exception for this, only for `GatewayError`); on `kind=="brief"`, appends
     `derive_brief_evidence(brief)`'s entries to the build store **before** `build_turn_state(...)`
     is rendered; populates the new `decompose` response field.

## 3. §6.9 — red-on-arrival proof, real output

Copying `tests/test_f03_module_brief_decompose.py` into a disposable **detached worktree** at the
lane base and installing an **independent** venv there (an editable-install `.pth` file hardcodes
its source path — symlinking the builder's own `.venv` silently ran the modified worktree's code
under the base worktree's test tree; caught and fixed before trusting this result):

```
$ git worktree add --detach /tmp/f03-base-check 6fd66f3
$ cd /tmp/f03-base-check/orb/backend/relay-py && python3 -m venv .venv && .venv/bin/pip install -q -e ".[dev]"
$ PYTHONDONTWRITEBYTECODE=1 .venv/bin/pytest -q tests/test_f03_module_brief_decompose.py
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t1_composition_build_turn_produces_a_real_module_brief
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t2_fail_closed_garbage_decode_never_yields_a_brief
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t3_repair_prompt_names_the_actual_violation
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t4_repair_is_bounded_at_exactly_one
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t5b_build_mode_calls_decompose_and_never_atomize
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t6_mode_stays_explicit_never_inferred
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t7a_a_validated_brief_moves_coverage_registers_strictly
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t7b_a_clarify_request_launders_no_coverage
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t8a_decompose_admission_refused_with_zero_gateway_calls_when_exhausted
FAILED tests/test_f03_module_brief_decompose.py::test_f03_t8b_gateway_error_on_decompose_degrades_without_leaking_the_reservation
10 failed, 4 passed, 5 warnings in 0.45s
```

**10 RED**, exactly T1, T2, T3, T4, T5b, T6, T7a, T7b, T8a, T8b — matching the contract's §6.9
table. **4 GREEN at base**, and each is legitimate, not a miscategorized red:

- **T5a, T5c** — the contract's own designated locks (focus path already correct; both use
  `monkeypatch.setattr(..., raising=False)` so spying on a not-yet-imported `decompose` name
  doesn't error at collection time).
- **T7c** (`Contradiction` register unchanged by either outcome) and **T8c** (record the spend
  delta) are not claimed RED anywhere in §6.9's table — and correctly so: at the lane base nothing
  anywhere emits `Contradiction` evidence (T7c's property already holds, vacuously), and a
  BUILD-mode turn costs the *same* as a FOCUS-mode turn when nothing calls `decompose` yet (T8c's
  `delta_paise == 0`, `0 < 25%`, both still true). Disclosed rather than presented as red.

## 4. Green — real output, this worktree

```
$ PYTHONDONTWRITEBYTECODE=1 .venv/bin/pytest -q tests/test_f03_module_brief_decompose.py -s
.............T8C_SPENT_WITH_DECOMPOSE_PAISE=14 T8C_SPENT_WITHOUT_DECOMPOSE_PAISE=7 T8C_DELTA_PAISE=7 T8C_DELTA_PCT_OF_RESERVATION=1.75
.
14 passed, 5 warnings in 0.37s
```

**T8c's recorded numbers**: with an overridden `Rates(1000, 1000)` (round paise/1k-token math) and
the scripted `UsageDelta(llm_tokens_in=3, llm_tokens_out=4)` per call, one build turn with a
successful decompose spends **14 paise** (conversational + one decompose attempt, 7 paise each);
the same turn with no decompose at all (a mirrored FOCUS turn) spends **7 paise**. Delta = **7
paise = 1.75% of the ~₹4 (400 paise) session reservation** — well under §2.4's 25% revive
threshold. **§2.4's revive trigger: did NOT fire.** (Caveat, disclosed: this is scripted-gateway
usage, a mechanism demonstration, not a live-provider cost measurement — real token counts for a
real decompose prompt/response will differ.)

Full pre-existing suite + this new file, same worktree:

```
$ PYTHONDONTWRITEBYTECODE=1 .venv/bin/pytest -q
FAILED tests/test_f04_build_mode_wire.py::test_a1_2_system_prompt_byte_matches_build_v1_md_at_the_gateway_boundary
FAILED tests/test_f04_build_mode_wire.py::test_a7_1_build_mode_permits_a_clarify_question_that_converse_would_veto
FAILED tests/test_f04_build_mode_wire.py::test_a7_2_build_mode_still_vetoes_shame_adjacent_output
3 failed, 441 passed, 5 warnings in 2.66s
```

441 = 430 (lane-base baseline, itself re-confirmed at the start of this lane) − 3 (below) + 14
(this file). **Zero regressions anywhere else in the 430-test pre-existing suite.** See §6 for the
3 failures — a disclosed, analyzed, escalated conflict, not silently absorbed.

## 5. §7 manual E2E drive — real live processes, raw curl, no RN, no Swift

`tests/manual/stub_gateway.py` (F04-owned) always returns one fixed non-JSON string regardless of
prompt — correct for F04's own A1 (which tests *which prompt was sent*), insufficient here (I need
to see a *real validated brief come back*). Rather than edit that shared file, I wrote a disposable,
scripted variant in my scratchpad (`scripted_stub_gateway.py`, not committed, not shared) that
serves an ordered list of replies over repeated `/v1/complete` calls — same wire shape, additive
capability only.

```
$ python3 -m venv/pip already set up; started:
.venv/bin/python scripted_stub_gateway.py --port 8383 --record /tmp/f03-e2e-gateway.jsonl \
  --replies-file f03-e2e-replies.json &     # pid 71248
ORB_GATEWAY_URL=http://127.0.0.1:8383 ORB_CONTEXT_DB_PATH=/tmp/f03-e2e-relay-context.db \
  .venv/bin/python -m uvicorn orb_relay.app:app --host 127.0.0.1 --port 8766 &   # pid 71251
$ curl -s http://127.0.0.1:8383/healthz && curl -s http://127.0.0.1:8766/healthz
{"status": "ok"}{"status":"ok"}
```

**Step 3 — build turn 1, scripted gateway replies with the conversational text then
`complete_module.json`:**

```
$ curl -s -X POST http://127.0.0.1:8766/v1/respond -H "Content-Type: application/json" \
  -d '{"tenant_id":"t-e2e","user_id":"u-e2e","session_id":"s-e2e-clean-1","text":"I want to build a rate limiter for the gateway","mode":"build"}'
text: Sounds good, let us shape this together.
decompose.kind: brief
decompose.brief.node_id: lld-v1-schema
decompose.brief.interface: [{'name': 'validateModuleBrief', ...}, {'name': 'canonicalJson', ...}, {'name': 'contentHash', ...}]
decompose.brief.acceptance: {'given': 'a ModuleBrief fixture on disk', 'when': 'validateModuleBrief and validate_module_brief and validate_module_brief (rust) all parse it', ...}
```

`node_id`, `interface`, and `acceptance` are byte-identical to `fleet/contracts/fixtures/lld/complete_module.json`
on disk — confirmed by eye, over a real HTTP response from a real `uvicorn` process.

**Step 4 — same session, second turn, scripted gateway now replies with garbage twice:**

```
$ curl -s -X POST http://127.0.0.1:8766/v1/respond -H "Content-Type: application/json" \
  -d '{"tenant_id":"t-e2e","user_id":"u-e2e","session_id":"s-e2e-clean-1","text":"it should use a sliding window","mode":"build"}'
http 200 body text: Got it, tell me a bit more.
decompose.kind: clarify_request
decompose.brief: None
decompose.missing: ['interface', 'data_owned', 'acceptance', 'registry_verdict']
```

HTTP 200 (not 500), the spoken reply still landed, and the fail-closed branch produced no brief.

**Step 5 — `/v1/atomize` on the SAME live process, pre-F03 body shape (focus mode, driven not assumed):**

```
$ curl -s -X POST http://127.0.0.1:8766/v1/atomize -H "Content-Type: application/json" \
  -d '{"session_id":"s-e2e-clean-atomize","tenant_id":"t-e2e","user_id":"u-e2e","task":"file my taxes"}'
{
    "tenant_id": "t-e2e", "session_id": "s-e2e-clean-atomize",
    "output": {"steps": [{"step_text": "Open the tax portal", "est_min": 1, "done_signal": "portal on screen"}], "steps_total": 1},
    "source": "model", "rejections": [], "spent_paise": 1, "remaining_paise": 399
}
```

Byte-identical shape to every pre-F03 `/v1/atomize` response — no `decompose` key, no `build` key.
Both servers killed by PID afterward (`kill 71248 71251`); confirmed `lsof -i :8383 -i :8766` prints
nothing.

**Gotcha hit and fixed during this drive**: my first attempt reused a stale `stub_gateway.py`
process from an earlier port-clash (my scripted stub failed to bind — `OSError: [Errno 48] Address
already in use` — because I'd tried to `kill %1` in a *new* Bash tool call, and job-control specs
like `%1` do not persist across separate shell invocations the way they would across commands typed
into one interactive terminal). The relay silently kept talking to the old, unscripted stub, which
produced a real but misleading response. Killed the stale PID explicitly, restarted clean, and
re-drove the whole sequence on fresh ports with a fresh `ORB_CONTEXT_DB_PATH` before trusting any of
the output above.

## 6. KNOWN, ANALYZED, UNRESOLVED CONFLICT — escalating, not silently absorbing

`tests/test_f04_build_mode_wire.py` is F04-owned and frozen ("the builder may not edit this file...
escalate"). Three of its cases now fail:

```
FAILED test_a1_2_system_prompt_byte_matches_build_v1_md_at_the_gateway_boundary
FAILED test_a7_1_build_mode_permits_a_clarify_question_that_converse_would_veto
FAILED test_a7_2_build_mode_still_vetoes_shame_adjacent_output
```

**Root cause, proven, not guessed:**

- `test_a1_2` asserts `real_relay.last_gateway_request()["system"]` (the LAST recorded gateway
  call) equals `build.v1.md`'s content. Since a build turn now makes a **second** real gateway call
  (the decompose attempt, which F03's own contract §2.4 mandates unconditionally), the "last"
  recorded request is now the decomposer's system prompt, not the conversational one. Confirmed via
  the real pytest diff: `sent_system` == `"You turn one build goal into a single
  ModuleBrief...` instead of the expected `build.v1.md` text.
- `test_a7_1`/`test_a7_2` each script a **finite** `ScriptedGateway` with exactly enough replies for
  the conversational leg (plus, for A7.2, its own internal guard repair). Once decompose *also*
  calls the same gateway, the scripted queue is exhausted and `ScriptedGateway.complete()` raises
  `IndexError: pop from empty list` (confirmed in the real traceback), which is an unhandled
  exception in `A7.1`'s case and changes the assertable call count in both.

**Why this cannot be fixed inside this lane's boundary, proven rather than assumed:** the lane
contract's own §2.4 ("KILLED — decompose only on the first build turn, gated on session state")
states plainly: *"Attempting a decompose every build turn and getting a clarify request early is
the designed loop running, not a failure to be optimized away."* This is presented as
non-negotiable, with an explicit revive trigger (§6.8's measured spend exceeding 25% — which T8c
above shows has NOT fired). There is no heuristic available to this lane (turn count, text length,
mode) that would make decompose skip **exactly** these three tests' turns (each sends full,
ordinary sentences — "I want a rate limiter", "what should I use for storage", a red-team distress
line — indistinguishable in shape from this lane's own T1's scenario, which *requires* decompose to
fire on a fresh, single-turn build session). Concretely, `test_a7_1`'s own assertion
`build_gateway.calls == 1` cannot be satisfied by any graceful internal handling of decompose's
failure, because `ScriptedGateway.calls` increments **before** its `.pop(0)` raises — the call
itself is the problem, not what happens after it.

**What I did NOT do:** edit `test_f04_build_mode_wire.py` (forbidden by its own front matter), or
weaken/gate F03's decompose call to dodge these three tests (would silently reverse §2.4's own
explicit, reasoned decision on no new evidence, for the sole purpose of making an unrelated file's
tests pass).

**What this needs:** a lead-architect decision on `test_f04_build_mode_wire.py` — likely updating
A1.2 to inspect the request whose `system` prompt matches `build.v1.md` rather than the literal
last one, and giving A7.1/A7.2 enough scripted replies to also cover the now-mandatory decompose
call (or asserting `>=1` conversational call rather than an exact gateway-wide count). This is a
one-paragraph, mechanical fix once someone with authority over that frozen file makes the call — it
is not a further F03 design question.

## 7. §6.10 mutation table — all 8 rows, real failure text, reverted, re-confirmed green

Run via checkpoint-commit-then-`git checkout --`-per-file (never `git stash`, per
`FLEET-LEARNINGS.md`): commit `06c5df9` on `lane/F03-atomizer` is the clean baseline every mutation
below was applied to and reverted from; `git diff --stat` confirmed empty after every single revert
before moving to the next row.

| # | Mutation | Real result |
|---|---|---|
| M1 | `if body.mode is ResponseMode.BUILD:` → `if True:` | **T6 FAILED**: `AssertionError: gateway called more times than the test scripted` (the FOCUS-mode leg of T6 now also calls decompose) |
| M2 | `_parse_and_validate`'s failure path reverted to the old constant string | **T3 FAILED**: `assert 'alternatives' in "...Your previous answer was rejected: module brief failed lld.v1 schema validation..."` — false |
| M3 | `MAX_REPAIR_ATTEMPTS = 2` | **T4 FAILED**: `AssertionError: gateway called more times than the test scripted` (3rd decompose attempt, only 2 scripted) |
| M4 | fail-closed branch returns `kind="brief"` with a hand-built `ModuleBrief.model_construct(node_id="best-effort-guess")` | **T2 FAILED**: `AttributeError: 'ModuleBrief' object has no attribute 'interface'` inside `derive_brief_evidence` — the guessed brief is incomplete and blows up downstream; either way T2 does not pass |
| M5 | `derive_brief_evidence` called unconditionally, including on `clarify_request` | **T7b FAILED** (the single most important row): `assert 5 == 1` — the clarify-turn's log picked up 4 laundered evidence entries it must never earn |
| M6 | `decompose(...)` awaited before `meter.reserve_remaining()` | **T8a FAILED**: `assert 2 == 1` — `gateway.calls` — the exhausted-budget scenario still called the gateway once for decompose despite what should have been a 402 with zero calls |
| M7 | BUILD branch also calls `atomize(body.text, ...)` and discards the result | **T5b FAILED** (`AssertionError: atomize must never be called from a BUILD-mode /v1/respond turn`), **T5a still PASSED** — confirms T5a genuinely cannot see this dual-write regression and T5b is the one that catches it, exactly as the contract predicts |
| M8 | `derive_brief_evidence`'s `if brief.interface:` guard removed (unconditional `Coverage[interface]`) | **All 14 tests still PASSED** — confirmed structurally unreachable: `validate_module_brief` empirically rejects any brief with an empty `interface` list (`Field(min_length=1)`), so no object that ever reaches `derive_brief_evidence` through the real pipeline can have one. Recorded honestly as a **kills (structural)** result per the contract's own explicit allowance, not skipped. |

## 8. Judgment calls, disclosed

1. **§3.2's GatewayError-degrade vs. §3.2's own-admission-refusal-402**: a `GatewayError` from the
   decompose call degrades the turn's build data (HTTP 200, `decompose: null`); a
   `ReservationExceededError` from decompose's own admission propagates as HTTP 402 for the whole
   turn, discarding an already-produced (already-settled, already-stored) conversational reply from
   the caller's perspective. The contract's §3.2 only names `GatewayError` for the graceful
   degrade; I read that as deliberate (an admission refusal is a budget-policy decision, not a
   transient upstream failure) and did not extend the same leniency to budget exhaustion. If this
   reading is wrong, it is a one-line change (add `ReservationExceededError` to the degrade
   instead of re-raising) — flagged rather than silently assumed correct.
2. **`_reserve_run_settle` releases on `except BaseException`, not a named exception list**: broader
   than the original triplicated code (which only released on `GatewayError` at one site and
   `GatewayError`/`Wait*Exceeded` at another). This is a strict generalization — any exception from
   the guarded call now releases the hold instead of leaking it — verified not to change any
   existing test's observable behavior (§4's 441-passed number), and arguably closes a
   pre-existing gap (a third, unlisted exception type from `atomize()`/`complete_guarded_conversation()`
   would previously have leaked the reservation).
3. **`MAX_REJECTION_DETAIL_CHARS = 400`**: the contract left this value to the builder's judgment. It is not calibrated
   against a real model's context budget beyond staying well under `DECOMPOSER_MAX_TOKENS` (768,
   itself an output-token cap, not directly comparable to the input characters this constant
   bounds) — a round, conservative number, disclosed rather than derived.
4. **T8c is scripted-usage, not live-provider cost**: the recorded 14/7/7-paise/1.75% numbers use
   the test harness's fixed `UsageDelta(llm_tokens_in=3, llm_tokens_out=4)` per call, not a real
   decomposer prompt against a real model. It demonstrates the mechanism (the added spend is
   measured and attributable) and that §2.4's revive trigger has not fired on the numbers available
   here — it is not a claim about real production cost.

## 9. Reproduce from a fresh clone

```bash
cd "Light/.worktrees/F03-atomizer/orb/backend/relay-py"
python3 -m venv .venv
.venv/bin/pip install -q -e ".[dev]"
PYTHONDONTWRITEBYTECODE=1 .venv/bin/pytest -q                                    # 441 passed, 3 failed (see §6)
PYTHONDONTWRITEBYTECODE=1 .venv/bin/pytest -q tests/test_f03_module_brief_decompose.py -s   # 14 passed
git diff --numstat 6fd66f3...HEAD -- orb/backend/relay-py/src/orb_relay/proxy/atomizer.py orb/backend/relay-py/src/orb_relay/proxy/semantic_checks.py   # prints nothing
```
