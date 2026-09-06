# Verdict: B5 intent-classifier-question-prefix

## PASS

Worktree: `company/products/adhd-focus-orb-worktrees/b5-intent-classifier-question-prefix`
Branch: `fix/b5-intent-classifier-question-prefix`, commit `2d980e6` (based on merge-base `5f10515`).

## What I ran

1. `git show 2d980e6 -- apps/mobile/src/router/IntentClassifier.ts{,.test.ts}` — confirmed the diff is
   exactly a one-line change (`{matched:false,candidates:[]}` -> `{matched:true,label:'question'}`)
   in the question-form-prefix short-circuit at the old lines 124-126, plus one updated test
   expectation and one new regression test. Matches the claim precisely.

2. Read the surrounding file (RULES loop, `classifyIntent`, `AmbiguousIntentNotWiredError`,
   `notYetWiredAmbiguousResolver`) — confirmed the pre-fix short-circuit ran *before* the RULES loop
   (which is where the `question` rule's `"?"` keyword lives), so it really did starve
   `classifyIntent`'s default resolver of candidates and throw.

3. Independently reproduced RED and GREEN by manual file-swap (not just trusting the builder's logs):
   - Copied parent commit `2d980e6~1`'s `IntentClassifier.ts` into place, ran the branch's own
     `IntentClassifier.test.ts`: **2 failed / 49 passed** — exact same two tests the builder's
     red-before-fix.log names (`'does not treat question-shaped control phrases as explicit pause
     commands'` and the new B5 regression test), byte-identical failure diff.
   - Restored the fixed file: **51/51 passed**.
   - Ran `classifyIntent('Could I get help with my taxes?')` / `'Should I pause for now?'` / `'Do I
     need to stop?'` directly via `tsx` against the pre-fix source: all three **threw
     `AmbiguousIntentNotWiredError`**. Same three against the fixed source: all three returned
     `'question'`, no throw. This is the actual runtime behavior the claim describes, verified by
     execution, not by re-reading the builder's report.
   - Confirmed the builder's own caveat: `classifyByRule("Could you help me with my taxes?")` ->
     `{matched:true,label:'stuck'}`, never touches this branch. The bug report's literal example
     does not reproduce; the builder's corrected repro strings do. This is accurate, not spin.

4. Full test ladder in the worktree:
   - `npm run lint` -> clean.
   - `npm run typecheck` -> 8 pre-existing errors in `AppAudioBedFallback.test.tsx` /
     `BuildModeWire.test.ts`, unrelated to this file. Reconfirmed identical on a disposable detached
     worktree at the merge-base commit `5f10515` (`git worktree add --detach /tmp/orb-baseline-verify
     5f10515...`) — same files, same error text, same count. Not a regression from this change.
   - `npm run test` (vitest, whole repo) -> 716 passed, 2 suites fail to *load* on the pre-existing
     `@pe/llm-gateway/adapters/memory` resolution gap. Same baseline worktree: 715 passed (one fewer
     because it lacks the new regression test), same 2 load-failures, same error text. Confirms the
     +1 is exactly the new test and nothing else moved.
   - `npm run test:py` -> 398 passed.
   - `npm run test:rs` -> 66 passed.
   - Worktree removed afterward (`git worktree remove --force`), no `git stash` used.

5. Adversarial probes via direct `classifyByRule` calls: empty string, whitespace-only, bare `"?"`,
   case-insensitivity (`"CAN I STOP?"`), a prefix with no `"?"` (`"could i"`, `"am i allowed to
   leave"` — correctly still `matched:false`, unaffected by this fix), and running every case twice
   in the same process for determinism. All deterministic, no crashes, no flakiness.

6. Semantic check (the actual judgment call): per
   `blueprints/ADHD-Focus-Orb-L8-Deep-Dive/04-MODEL-ROUTER-AND-THE-CREW.md` §2, `intent==question ->
   FAST_VOICE(short, capped)`. Labeling all 5 question-form-prefixed-and-`"?"` utterances as
   `'question'` is the conservative, correct choice: it was already deliberate pre-existing design
   (see the file's own comment on `isLikelyTaskRequest`'s "steering the conversation" note and the
   original test's intent, "does not treat question-shaped control phrases as explicit pause
   commands") that these phrasings must NOT be silently treated as an explicit `pause` (or `done`/
   `next`) command — auto-pausing or auto-completing on a question is worse than answering it
   conversationally. The pre-fix short-circuit already intercepted these before the RULES loop
   (excluding `done`/`next`/`stuck`/`pause` too, not just `pause`); the fix only changes what happens
   *after* that interception, from "throw" to "answer as a question." That is strictly safer than
   before on every one of these phrasings, including `"Should I pause for now?"` (answered
   conversationally rather than silently pausing) and edge cases like `"Can I mark this done?"`
   (answered conversationally rather than throwing — was already excluded from the `done` rule
   pre-fix, so no new misrouting is introduced, only the crash is removed). No sentence in the
   5-prefix set is made worse by this change relative to pre-fix; several classes of previously
   crashing input now resolve safely.

7. Checked the builder's own severity caveat about the sole production caller
   (`apps/mobile/src/runtime/T0FocusSession.ts:449-455`): confirmed by reading the code that the
   WORKING-state caller uses `classifyByRule` + a `transcript.includes('?')` fallback and
   `classifySafeIntent` (never the throwing `classifyIntent` with the default resolver), so this
   defect, while real, was not observed to crash today's one production caller. Accurate, not
   overclaimed.

8. `FLEET-LEARNINGS.md` (repo root, `/Users/rachitsrivastava/youtube/Principal
   Engineering/FLEET-LEARNINGS.md`) entry at line 1120 (`2026-09-03-0000 — ... B5
   intent-classifier-question-prefix`) is a real, substantive entry that matches everything above,
   including the repro-string caveat and the production-caller caveat. Not a stub.

## What broke

Nothing new. The 2 typecheck errors and 2 vitest load-failures are pre-existing at the merge-base and
unrelated to this file — verified byte-identical via a disposable detached worktree, not asserted.

## Verdict

**CONFIRMED.** Fix is minimal, correct, semantically sound per the blueprint's routing table, honestly
scoped (including admitting the original bug report's example doesn't reproduce and the real
production caller wasn't actually crashing), RED->GREEN independently reproduced by direct execution,
full ladder green apart from pre-existing unrelated failures, FLEET-LEARNINGS entry is real.
