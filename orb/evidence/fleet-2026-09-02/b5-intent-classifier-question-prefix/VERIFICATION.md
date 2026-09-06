# B5 — IntentClassifier question-prefix ordering bug

## Task

`apps/mobile/src/router/IntentClassifier.ts`'s `classifyByRule`: a question-form-prefix check
(`"can I"`, `"could I"`, `"should I"`, `"do I"`, `"am I allowed to"` + `?`) short-circuited to
`{matched: false, candidates: []}` BEFORE the `question` rule (whose own keyword list includes the
literal `"?"`) ever got a chance to fire. Feeding empty candidates into `classifyIntent`'s default
resolver (`notYetWiredAmbiguousResolver`) throws `AmbiguousIntentNotWiredError` — a deterministic,
no-I/O function throwing on an ordinary question, violating `docs/BUILD-DIGEST.md` §4
("deterministic path must not throw on valid inputs") when the utterance should simply route
`question -> fast_voice` (§2).

## Note on the exact repro string in the brief

The brief's literal example, `"Could you help me with my taxes?"`, does **not** hit this code path:
`QUESTION_FORM_PREFIXES` is `["can i", "could i", "should i", "do i", "am i allowed to"]` — it does
not include `"could you"`. That exact string classifies as `stuck` (matches the `help` keyword) via
the normal `RULES` loop and does not throw. Verified directly (probe script, not committed):

```
classifyByRule("Could you help me with my taxes?") => {"matched":true,"label":"stuck"}
classifyIntent("Could you help me with my taxes?") => "stuck"   (no throw)
```

The underlying mechanism the defect-hunt report describes is real, though, for any utterance that
*does* start with one of the five listed prefixes and contains `?` — e.g. `"Could I get help with
my taxes?"`, `"Should I pause for now?"`, `"Do I need to stop?"`, `"Can I ask you something?"`. Every
one of those unconditionally produced `{matched: false, candidates: []}` and threw
`AmbiguousIntentNotWiredError` when routed through `classifyIntent` with the default resolver — the
same class of failure as the brief's headline claim, just not on the literal string quoted. Probed
and confirmed before writing the test.

Production impact is narrower than "the app throws mid-session": `T0FocusSession.ts`'s WORKING-state
handler (`apps/mobile/src/runtime/T0FocusSession.ts:449-455`) has its own separate
`transcript.includes('?')` fallback that routes any `?`-containing transcript to
`respondToConversation` regardless of what `classifyByRule` returns, and it never calls the throwing
`classifyIntent` function at all (`classifySafeIntent` at L671-676 calls `classifyByRule` and falls
back to `FALLBACK_INTENT` on no match — no throw). So today's single production caller is not
observed to crash. The defect is real at the unit level `classifyByRule`/`classifyIntent` are meant
to guarantee deterministically (BUILD-DIGEST §4's own promise, and the exact functions/behavior the
existing `IntentClassifier.test.ts` `describe('classifyIntent — ...')` block already tests for other
ambiguous inputs), and any future caller of `classifyIntent` directly (rather than duplicating the
`'?'` fallback) would inherit the throw. Fixed at the root (`classifyByRule`) rather than papering
over it in `T0FocusSession.ts`.

## The fix

`apps/mobile/src/router/IntentClassifier.ts`, `classifyByRule`, the question-form-prefix branch:
changed `return { matched: false, candidates: [] }` to `return { matched: true, label: 'question' }`.
A question-form-prefixed `?`-utterance is unambiguously a question; it should resolve to `question`
directly instead of producing an empty-candidate ambiguous result, even though it may also contain a
word that would otherwise fire an earlier rule ("can I **stop**?" contains the `pause` keyword;
"could I get **help**?" contains the `stuck` keyword) — the whole reason this branch exists is to
protect against exactly that false match, and it should protect by classifying correctly, not by
producing nothing.

One pre-existing test's expectation was updated to match the corrected (not just "not pause",
actually "question") behavior:

- `IntentClassifier.test.ts`, `'does not treat question-shaped control phrases as explicit pause
  commands'`: `classifyByRule('can I stop?')` now expected to equal `{matched: true, label:
  'question'}` (was `{matched: false, candidates: []}`). The test's own title/intent — "not pause" —
  still holds; the assertion is now precise about what it *does* become instead of only what it does
  not.

A new test was added directly below it (`'B5: a question-form-prefix utterance during an active
session routes to question, not an empty/ambiguous match'`) that pins the exact repro (`classifyIntent`
must not throw and must return `'question'` for `"Could I get help with my taxes?"` and `"Do I need
to stop?"`).

Neither test file is under `tests/large/acceptance/**` (that directory does not exist in this repo);
no acceptance-test boundary was touched.

## RED -> GREEN

RED (before the fix — confirmed by temporarily reverting only the one-line fix, keeping the new
test, per this session's established "revert-slice, run, restore" pattern for demonstrating RED
without a fresh-worktree diff):

```
$ npx vitest run apps/mobile/src/router/IntentClassifier.test.ts
 FAIL  ... > does not treat question-shaped control phrases as explicit pause commands
AssertionError: expected { matched: false, candidates: [] } to deeply equal { matched: true, label: 'question' }
 FAIL  ... > B5: a question-form-prefix utterance during an active session routes to question, not an empty/ambiguous match
AssertionError: expected { matched: false, candidates: [] } to deeply equal { matched: true, label: 'question' }
 Test Files  1 failed (1)
      Tests  2 failed | 49 passed (51)
```

Full output: `red-before-fix.log`.

GREEN (after restoring the fix):

```
$ npx vitest run apps/mobile/src/router/IntentClassifier.test.ts
 ✓ apps/mobile/src/router/IntentClassifier.test.ts (51 tests) 105ms
 Test Files  1 passed (1)
      Tests  51 passed (51)
```

Full output: `green-focused.log`.

## Wider verification

- `npx vitest run apps/mobile` (whole mobile app suite): **608/608 passed**, 46 test files.
  `green-mobile-suite.log`.
- `npx vitest run` (whole repo): **716/716 tests passed**; 2 test files fail to *load*
  (`backend/gateway-sidecar/__tests__/adapterTelemetry.test.ts`,
  `.../complete.test.ts`), both on `Cannot find module '@pe/llm-gateway/adapters/memory'`.
  `green-whole-repo.log`. Confirmed **pre-existing and unrelated to this change**: reproduced
  byte-for-byte identical on an unmodified base checkout via a disposable detached worktree
  (`git worktree add --detach /tmp/baseline-b5-check 5f10515752ea51d53f8a7b99c66591533e8fe5c2`,
  node_modules symlinked from the main checkout, then removed with `git worktree remove --force`
  afterward — never `git stash`, per this file's own hard boundary). Same root cause other
  2026-09-02 entries in `FLEET-LEARNINGS.md` already documented for `@pe/llm-gateway`.
- `npm run lint`: clean (`lint.log`).
- `npx tsc --noEmit -p apps/mobile`: fails with 8 pre-existing errors
  (`AppAudioBedFallback.test.tsx` / `BuildModeWire.test.ts`), confirmed byte-for-byte identical
  on the same disposable unmodified-base worktree, run twice (per the b4 entry's own gotcha about a
  possible stale-incremental-build false negative on the first run — reproduced identically both
  times here too).
- `npm run test:py`: **398 passed** (`test-py.log`).
- `npm run test:rs`: **66 passed** (`test-rs.log`).
- `npm run verify`: **exits 2**, halting at the `typecheck` step on the same pre-existing
  `apps/mobile` errors isolated above (`verify.log`). Does not reach `test`/`test:py`/`test:rs` as
  separate `verify` steps, but each was run and is green independently (see above).

## Files changed

- `apps/mobile/src/router/IntentClassifier.ts` — the fix (one `return` statement changed, plus an
  explanatory comment).
- `apps/mobile/src/router/IntentClassifier.test.ts` — one pre-existing assertion corrected to the
  fixed behavior, one new test added.

## Reproduce from a fresh clone

```bash
cd company/products/adhd-focus-orb-worktrees/b5-intent-classifier-question-prefix
# worktree has no node_modules/.venv of its own (gitignored) — symlink the sibling checkout's,
# mind the relative depth (this worktree sits one level deeper than the main checkout):
ln -s ../../adhd-focus-orb/node_modules node_modules
mkdir -p backend/relay-py
ln -s ../../../../adhd-focus-orb/backend/relay-py/.venv backend/relay-py/.venv

npx vitest run apps/mobile/src/router/IntentClassifier.test.ts   # 51/51
npx vitest run apps/mobile                                       # 608/608
npx vitest run                                                    # 716/716, 2 pre-existing load failures
npm run lint                                                      # clean
npm run test:py                                                   # 398 passed
npm run test:rs                                                   # 66 passed
npm run verify                                                    # exits 2 at typecheck (pre-existing)
```

## Commit

Branch `fix/b5-intent-classifier-question-prefix`, committed in this worktree only.
