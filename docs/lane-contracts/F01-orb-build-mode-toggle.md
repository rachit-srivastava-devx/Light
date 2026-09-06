# F01 — Orb build-mode toggle (lane contract)

| | |
|---|---|
| **Feature** | F01 — Orb build-mode toggle |
| **Repo / lane** | `orb` · branch `lane/F01-orb-build-mode` · base `master` @ `e421a59` |
| **Lead (author of this contract)** | lead-architect (F01) |
| **Builder** | *(unassigned — this file is the brief)* |
| **Verifier** | *(independent; must re-derive, never trust the builder's report)* |
| **C1 registry verdict** | **install (the whole mode plane) + build-new (two small things that genuinely do not exist)** — see §1 |
| **Depends on** | S0 baseline gate only. Blocking precondition in §7. |

---

## 0. Restatement — what is actually being asked, in this codebase's own terms

The orb today can only ever *launch* into focus mode. `createT0FocusSession()`
(`orb/apps/mobile/src/runtime/T0FocusSession.ts:109`) picks one of four rotating launch greetings and
its `start()` at line 346 hands `makeEnvelope(...)` the string literal `'focus'` as the envelope mode.
That literal is the only reason build mode is unreachable — because everything *downstream* of it is
already built and already green: `OrbMode` at `orb/apps/mobile/src/router/contracts.ts:33` is already
`'converse' | 'focus' | 'teach' | 'build'`; `ConversationPort`
(`orb/apps/mobile/src/runtime/ConversationPort.ts:92`) already forwards `mode` onto the POST body and
already omits it rather than sending `null`; relay-py's `ResponseMode`
(`orb/backend/relay-py/src/orb_relay/proxy/schemas.py:103`) already has `BUILD = "build"`; and
`orb/apps/mobile/src/build/BuildModeWire.test.ts` already proves `mode:'build'` reaches the wire. A
whole parallel build FSM (`orb/apps/mobile/src/build/{contracts,fsm-core,BuildStateMachine}.ts`) is
sitting there too, file-disjoint from the session FSM by design.

So F01 is not "add build mode." F01 is: **give the already-installed build plane a front door.** Two
things do not exist anywhere in this tree, and I confirmed both by grep — there is no production
caller of `mode: 'build'` anywhere in `orb/apps/mobile/src` (the only hit is that one test file), and
there is no build-mode greeting copy at all. Those two things, and nothing else, are this lane:
(a) a **launch-time mode selector** at the composition root (`orb/apps/mobile/src/App.tsx:131-153`,
where `createT0FocusSession` is constructed), and (b) a **build-specific opening line** that the
existing, unmodified session runtime speaks instead of the focus greeting, carrying `mode:'build'` on
the real envelope. Everything else — the freeze protocol, the depth registers, the module-brief
atomizer, turn-level build routing — is a later feature and is explicitly out of scope here (§8).

---

## 1. C1 / L2 registry decision, said out loud

`Light/registry/{features,services,modules}/` are all **empty** — nothing is installable from the
Company-OS registry. The real registry for this lane is the copied orb itself, and against it the
verdict splits:

| Thing | Verdict | Evidence |
|---|---|---|
| `OrbMode` includes `'build'` | **install — 0 lines** | `orb/apps/mobile/src/router/contracts.ts:33` |
| `ModedResponseEnvelope` superset pattern | **install — 0 lines** | `orb/apps/mobile/src/router/contracts.ts:53-55` |
| `mode` reaches the HTTP body | **install — 0 lines** | `ConversationPort.ts:90-93`; proven by `build/BuildModeWire.test.ts:15` |
| Relay accepts `"build"` | **install — 0 lines** | `schemas.py:103` (`BUILD = "build"`) |
| Build FSM states/events/table | **install — 0 lines, and untouched by F01** | `orb/apps/mobile/src/build/contracts.ts`, `BuildStateMachine.ts` |
| Session FSM, router, step gate, presence, voice loop | **install — 0 lines** | reused verbatim |
| **Launch-time mode selector** | **build-new (~15 lines)** | grep: zero production `mode: 'build'` call sites exist |
| **Build-mode greeting** | **build-new (~6 lines)** | grep: no build greeting copy exists |

FEATURES.md row F01 pre-labels this "install (reuse FSM/gateway)". That is right about the FSM and the
gateway and I am keeping it, but it is not the whole truth and the builder must not read it as "wire
up something that exists": the selector and the greeting are genuinely new code. Net new production
surface for this entire feature should land **under ~40 lines**. If the builder's diff to production
files is substantially larger than that, that is a signal something out of scope crept in — say so
rather than shipping it.

---

## 2. Killed alternatives

### 2.1 KILLED — a separate top-level build app/screen (`BuildOrbApp.tsx`) instead of a mode on the existing screen

The obvious "clean separation" move: a second root component with its own runtime, so focus code and
build code never meet. **Rejected on measured cost and on where the product is going.** `App.tsx` is
1000+ lines and is not a screen — it is the composition root for presence-bed boot, the relay
WebSocket effect, mic capture and VAD, barge-in, ducking, speech dispatch, the typed-input fallback,
and app-lifecycle recovery (`App.tsx:9-40` imports alone show the surface). A second root either
duplicates all of that (two places to fix every voice bug) or extracts it first — and extracting the
composition root is a multi-session refactor, not the smallest first-visible thing this lane is
supposed to be. It also fights the destination: F10's capstone is *one* conversation that goes
spoken → frozen → built, and F37 renders *the real orb conversation* in a pane. A forked screen means
a forked conversation, which is the thing P0 is trying to prove you don't need.

### 2.2 KILLED — a new `SessionState` FSM state (e.g. adding `BUILD_INTAKE` to the session union)

**Rejected — this exact alternative was already tried, killed, and documented in this repo, and the
replacement is already merged.** `orb/docs/SPEED-OF-THOUGHT-P0-CONTRACT.md:360-362` records killed
alternative (b) — one table with a `mode` discriminant on every guard — and the reason: every guard
would take a mode it ignores, and an illegal focus↔build edge degrades from a compile error into a
runtime guard failure. The already-landed answer is the file-disjoint parallel table in
`build/contracts.ts`, whose `BuildState` shares **no literal** with `SessionState` precisely so a type
error prevents cross-dispatch. Adding a build literal to `session/contracts.ts` now would also break
the exhaustive switches that `Router.ts`, `Policy.ts` and `ProsodyDirector.ts` compile against.

There is a second, independent reason to keep hands off: `session/StateMachine.ts` carries a known,
unresolved inconsistency (one file names 9 states including `CLARIFY`; the formal transition table has
8 rows and no `CLARIFY` row; a policy veto elsewhere references `Session ∈ {INTAKE, CLARIFY}`). **The
builder must not "fix" that as part of F01.** It is a real open question that deserves its own change
and its own review. Touching it here would make this lane's diff unreviewable and would put a
correctness question inside a cosmetic feature.

### 2.3 KILLED — voice command ("let's build something") as the mode selector for F01

Genuinely the nicest UX, and it is clearly where this goes — the build FSM already reserves an
`enter_build` event (`build/contracts.ts:22`) for exactly this. **Rejected for F01 on two grounds.**
First, scope collision: recognizing a spoken build intent means editing `router/IntentClassifier.ts`
and `router/Router.ts`, which is *the* focus-mode decision path this lane has to prove untouched — the
acceptance criterion and the entry mechanism would be in direct conflict. Second, it does not satisfy
the acceptance as written: "**Launching** in build-mode produces a build-mode-specific greeting" is a
launch-time property, and a mid-session voice switch cannot produce a launch greeting at all. Mid-session
entry via `enter_build` is a later feature and should be briefed as one.

### 2.4 KILLED — appending the build greeting to the existing `GREETINGS` array

The one-line version of this feature, and it is a **silent cross-mode data leak**. `T0FocusSession.ts:115`
selects with `greetingIndex % GREETINGS.length`, and `greetingIndex` comes from `Date.now()` at
`App.tsx:130`. Append two build greetings to that four-element array and roughly a third of ordinary
*focus* launches start speaking build copy — with no test failure anywhere, because no existing test
pins the array's length. `cachedPhrase()` at line 788 also does `GREETINGS.find(g => g.text === text)`,
so a build entry with a `phrase_id` would additionally claim a cached-audio hit for a recording that
does not exist. **The build greeting must live in its own constant.** This is asserted, not merely
requested — see test A6.

### 2.5 KILLED (briefly) — a module-level mutable or React context carrying the mode

Rejected because `T0FocusSession.ts` is contractually pure — no wall clock, no randomness, no ambient
state (`AGENTS.md` determinism invariant; the file's own header at lines 85-87 explains that even the
greeting index is injected rather than read). A module-level mode would be shared by every session
constructed in one process, and the existing test files construct 20+ sessions each. Mode is
constructor input, full stop.

---

## 3. The interface — exactly what changes

Three production files (one of them new) plus one new test file. Nothing else.

### 3.1 `orb/apps/mobile/src/runtime/T0FocusSession.ts`

**(a) New exported constant** — its own array, never merged into `GREETINGS`:

```ts
/**
 * Build-mode launch opening. Deliberately a separate constant from GREETINGS: that array is indexed
 * by `greetingIndex % GREETINGS.length` for focus launches, so appending here would leak build copy
 * into ordinary focus sessions (see lane contract F01 §2.4).
 *
 * No `phrase_id`: no recording of this line exists, so it resolves to `{ mode: 'novel' }` through
 * `cachedPhrase()` and costs a real TTS call. Claiming a cached phrase_id for an unrecorded line
 * would be a fabricated cache hit (CLAUDE.md anti-slop rules).
 */
export const BUILD_GREETING = 'Build mode. What are we making?';
```

The exact string is fixed by this contract; do not reword it. Rotation across several build greetings
is deliberately deferred (§8).

**(b) New optional 5th parameter** on `createT0FocusSession`, and only that:

```ts
import type { LaunchMode } from './LaunchMode';   // see §3.2b

export interface T0SessionOptions {
  /** Named `sessionMode`, not `mode`: this file already has two unrelated `mode` fields — the
   *  envelope's OrbMode and SpeechAudio's 'cached'|'novel' discriminant (see the warning at
   *  T0FocusSession.ts:719-721). A third bare `mode` here would be actively confusing. */
  readonly sessionMode?: LaunchMode;
}

export function createT0FocusSession(
  input: T0FocusSessionInput,
  atomizer: AtomizerPort = createStaticAtomizerPort(),
  greetingIndex = 0,
  conversation: ConversationPort = createStaticConversationPort(),
  options: T0SessionOptions = {},          // <-- the only signature change
): T0FocusSessionRuntime
```

`sessionMode` defaults to `'focus'`. Every one of the ~20 existing call sites passes 4 or fewer
positional arguments, so **zero existing call sites change** — that is the point of making it the
last parameter with a default, and it is what makes "the focus suite is unchanged" mechanically
provable rather than a claim.

**(c) `start()` at line 343-347** is the *only* behavioural edit. Currently:

```ts
async start() {
  seq += 1;
  state = dispatch(state, 'tap');
  return makeEnvelope(input, seq, state, gate, greeting.text, 'reuse', 0, 0, interruptionKind, 'focus');
},
```

It must become the same call with the launch mode and its matching opening substituted — i.e. the
`'focus'` literal becomes the resolved `sessionMode`, and `greeting.text` becomes `BUILD_GREETING`
when that mode is `'build'`. `dispatch(state, 'tap')` stays exactly as it is: build mode enters the
**same** session FSM state as focus mode (`IDLE_PRESENT → INTAKE`). Asserted by test A5.

**Every other `makeEnvelope(...)` call site in this file keeps its current hardcoded mode literal.**
There are 23 call sites — lines 168, 183, 191, 193, 197, 199, 206, 231, 264, 281, 291, 307, 331,
**346**, 365, 384, 397, 410, 447, 477, 505, 523, 612 (plus the definition at 701; the mention at 260
is a comment). Only line 346 changes; the other 22 stay exactly as they are. Turn-level build routing
— what a *reply* inside a build session should be tagged — is F03+, not F01, and a builder who threads
`sessionMode` into the other 22 call sites has silently shipped a different feature.

### 3.2 `orb/apps/mobile/src/App.tsx`

**(a) New prop** on `FocusOrbAppProps` (currently lines 100-108):

```ts
readonly launchMode?: LaunchMode;
```

**(b) Pass the mode through** at the `createT0FocusSession` construction (App.tsx:131-153) as the new
5th argument: `{ sessionMode: resolveLaunchMode(launchMode, process.env.ORB_LAUNCH_MODE) }`, and add
`launchMode` to that `useMemo`'s dependency array. Nothing else in `App.tsx` changes — no new UI, no
new effect, no change to the `autoStart` gate at 682-693.

### 3.2b `orb/apps/mobile/src/runtime/LaunchMode.ts` — new leaf file

The resolver does **not** live in `App.tsx`. `App.tsx` pulls in `react-native`, the presence bed, the
relay socket and the mic port, so importing it from a plain node-environment test drags in the whole
118-line `vi.mock` preamble that `AppSurface.test.tsx` needs. A7 must be cheap and mock-free, so the
resolver goes in its own leaf module that imports nothing but a type:

```ts
import type { OrbMode } from '../router/contracts';

/** Which conversation a session opens in. Turn-level modes ('converse'/'teach') are decided per
 *  turn by the router and are not launchable, so the launch domain is narrower than OrbMode. */
export type LaunchMode = Extract<OrbMode, 'focus' | 'build'>;

/**
 * Launch mode: explicit prop wins, then ORB_LAUNCH_MODE, then focus. An unrecognised env value
 * degrades to focus and warns — it must never crash the app, and it must never be forwarded to the
 * relay, where an unknown mode is a typed 422 (schemas.py:90-96).
 */
export function resolveLaunchMode(prop?: LaunchMode, env?: string): LaunchMode {
  if (prop) return prop;
  const raw = env?.trim().toLowerCase();
  if (raw === 'build' || raw === 'focus') return raw;
  if (raw) console.warn('focus-orb:unknown-launch-mode', { value: raw, falling_back_to: 'focus' });
  return 'focus';
}
```

`T0FocusSession.ts` imports `LaunchMode` from `./LaunchMode` (sibling, no cycle — the leaf imports
only `router/contracts`, which `T0FocusSession.ts:28` already depends on). `App.tsx` imports both
`LaunchMode` and `resolveLaunchMode` from `./runtime/LaunchMode`. Placing it under `runtime/` rather
than `build/` keeps it out of the Speed-of-Thought build plane, which F01 does not touch (§3.4), and
keeps `tooling/boundary-lint.mjs`'s app→runtime→router direction intact.

Exporting `resolveLaunchMode` is **only** so its unenumerated-input behaviour is directly testable.
Note carefully: testing it in isolation **does not satisfy this contract on its own** — see A3 and
the warning in §4.

### 3.3 `orb/apps/mobile/src/build/BuildModeLaunch.test.ts` — new file, written by the builder against §4

Test files under `orb/apps/mobile/src/build/` are un-ignored by an explicit negation
(`orb/.gitignore:7-8`, and `Light/.gitignore:18-20`) that exists because a bare `build/` rule
previously swallowed real source in this repo. Verify with `git status` (or
`git check-ignore -v <path>`) that the new file is actually tracked before committing. Do **not**
reach for `git add -f`, and do **not** edit any `.gitignore` — the negation is already there.

### 3.4 What does NOT change — the untouched list, mechanically checkable

These paths must have **zero** changed lines. This is not a style preference; it is the acceptance
criterion's other half, and the verifier checks it with a command, not by reading a report.

| Path | Why it must not move |
|---|---|
| `orb/apps/mobile/src/session/StateMachine.ts` | the 9-state session FSM — reused verbatim; carries the unresolved `CLARIFY` inconsistency (§2.2) |
| `orb/apps/mobile/src/session/contracts.ts` | `SessionState`/`SessionEvent` unions feed exhaustive switches |
| `orb/apps/mobile/src/runtime/ConversationPort.ts` | already forwards `mode` and already omits an absent one |
| `orb/apps/mobile/src/router/contracts.ts` | `OrbMode` already contains `'build'` |
| `orb/apps/mobile/src/router/Router.ts`, `router/IntentClassifier.ts` | the focus decision path (§2.3) |
| `orb/apps/mobile/src/lld/ResponseEnvelope.ts` | frozen surface — extended by superset only, never edited |
| `orb/apps/mobile/src/build/**` *(except the one new test file)* | already-landed U2/U4 work |
| `orb/backend/relay-py/**` | `ResponseMode.BUILD` already exists |
| `orb/apps/mobile/src/**/*.test.ts`, `*.test.tsx` *(all pre-existing)* | the regression evidence — see §6 |
| any `.gitignore` | §3.3 |

Proof command (must print nothing):

```bash
cd "<worktree>" && git diff --numstat e421a59...HEAD -- \
  'orb/apps/mobile/src/session/' 'orb/apps/mobile/src/router/' \
  'orb/apps/mobile/src/lld/ResponseEnvelope.ts' 'orb/apps/mobile/src/runtime/ConversationPort.ts' \
  'orb/backend/relay-py/' '*.gitignore' \
  'orb/apps/mobile/src/build/contracts.ts' 'orb/apps/mobile/src/build/fsm-core.ts' \
  'orb/apps/mobile/src/build/BuildStateMachine.ts' 'orb/apps/mobile/src/build/BuildEnvelope.ts' \
  'orb/apps/mobile/src/build/BuildModeWire.test.ts'
```

---

## 4. Acceptance test — `orb/apps/mobile/src/build/BuildModeLaunch.test.ts`

Real vitest, matching this repo's style (`vitest run` from `orb/`, node environment, `include:
['apps/mobile/**/*.test.ts', ...]` per `orb/vitest.config.ts:32-51`).

**The builder may not weaken, skip, rename, or delete any case below.** If a case cannot be made to
pass, the builder reports that as a blocked finding with the real error — deleting it is a contract
breach and the verifier must reject the lane on sight.

```ts
import { describe, expect, it } from 'vitest';
import { createStaticAtomizerPort } from '../runtime/AtomizerPort';
import { resolveLaunchMode } from '../runtime/LaunchMode';
import { BUILD_GREETING, createT0FocusSession } from '../runtime/T0FocusSession';

const INPUT = { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'ignored seed task' };
const step = () => createStaticAtomizerPort({ step_text: 'Open the tax portal', est_min: 1, done_signal: 'it is open' });

describe('F01 — build-mode launch', () => {
  // --- A1: the mode is real on the real envelope, not just a local variable -------------------
  it('A1 launching in build mode tags the launch envelope mode "build"', async () => {
    const runtime = createT0FocusSession(INPUT, step(), 0, undefined, { sessionMode: 'build' });
    expect((await runtime.start()).mode).toBe('build');
  });

  // --- A2: distinguishable, and distinguishable from the greeting THIS launch would otherwise
  //         have produced (a fixed-string compare could pass by luck; this cannot) -------------
  it('A2 the build opening differs from the focus opening at every greeting index', async () => {
    for (const index of [0, 1, 2, 3, 4, 17]) {
      const focus = await createT0FocusSession(INPUT, step(), index).start();
      const build = await createT0FocusSession(INPUT, step(), index, undefined, { sessionMode: 'build' }).start();
      expect(build.speech?.text).toBe(BUILD_GREETING);
      expect(build.speech?.text).not.toBe(focus.speech?.text);
      expect(focus.mode).toBe('focus');   // the focus launch is untouched by the build launch
    }
  });

  // --- A2b: the opening is not a fabricated cache hit -----------------------------------------
  it('A2b the build opening resolves to novel audio, not a phrase_id with no recording', async () => {
    const envelope = await createT0FocusSession(INPUT, step(), 0, undefined, { sessionMode: 'build' }).start();
    expect(envelope.speech?.audio).toEqual({ mode: 'novel' });
  });

  // --- A4: the default is preserved — omitting the option changes nothing ---------------------
  it('A4 omitting sessionMode still launches focus with the existing greeting', async () => {
    const envelope = await createT0FocusSession(INPUT, step(), 0).start();
    expect(envelope.mode).toBe('focus');
    expect(envelope.speech?.text).toBe("I'm here. Tell me the task.");
  });

  // --- A5: same FSM, reused. Build mode must NOT introduce a session state or a new cost. ------
  it('A5 build launch enters the same session state as focus and costs nothing extra', async () => {
    const focus = await createT0FocusSession(INPUT, step(), 0).start();
    const build = await createT0FocusSession(INPUT, step(), 0, undefined, { sessionMode: 'build' }).start();
    expect(build.session.state).toBe(focus.session.state);
    expect(build.meta.source).toBe(focus.meta.source);
    expect(build.meta.cost_paise).toBe(0);
  });

  // --- A6: §2.4's killed alternative is asserted, not merely asked for ------------------------
  it('A6 the build opening was not appended to the focus GREETINGS rotation', async () => {
    const rotation = await Promise.all(
      [0, 1, 2, 3, 4, 5, 6, 7].map((i) => createT0FocusSession(INPUT, step(), i).start()),
    );
    for (const envelope of rotation) {
      expect(envelope.speech?.text).not.toBe(BUILD_GREETING);
      expect(envelope.mode).toBe('focus');
    }
    // exactly four distinct focus greetings, i.e. the array's length did not change
    expect(new Set(rotation.map((e) => e.speech?.text)).size).toBe(4);
  });

  // --- A7: unenumerated launch-mode input, explicitly handled ---------------------------------
  it('A7 an unrecognised ORB_LAUNCH_MODE degrades to focus rather than reaching the relay', () => {
    // imported from the leaf module (§3.2b), NOT from App.tsx — App drags in react-native
    expect(resolveLaunchMode(undefined, 'build')).toBe('build');
    expect(resolveLaunchMode(undefined, ' BUILD ')).toBe('build');
    expect(resolveLaunchMode(undefined, 'teach')).toBe('focus');   // turn-level, not launchable
    expect(resolveLaunchMode(undefined, 'wat')).toBe('focus');
    expect(resolveLaunchMode(undefined, undefined)).toBe('focus');
    expect(resolveLaunchMode('build', 'focus')).toBe('build');     // explicit prop wins
  });
});
```

### A3 — the composition-root probe (**mandatory, and the one that actually matters**)

A1–A7 all pass against a runtime the *test* constructs. That is exactly the failure this repo has
already paid for twice: a `mode` that was computed everywhere and transmitted nowhere, fully
unit-tested and unreachable from the real app. **A1–A7 are proxies. A3 is the property.**

Add to the same file (or a sibling `BuildModeLaunch.app.test.tsx` if the mock preamble makes that
cleaner) a test that mounts the **real** `FocusOrbApp` with **no `runtime` prop**, so the composition
root builds its own session, and asserts the build opening reaches the rendered surface:

```
mount <FocusOrbApp launchMode="build" createAudioContext={fake} speaker={fake} />   // autoStart left at its default true
await the start() effect to flush
expect a rendered node with testID 'focus-orb-reply-text' whose text is BUILD_GREETING
and the same mount with launchMode="focus" must render one of the four focus greetings instead
```

Harness notes, so this is buildable and not a guess:
- Copy the mock preamble shape from `orb/apps/mobile/src/AppSurface.test.tsx` (lines 1-118: the
  hoisted `vi.mock` calls for `NativeMicPort` and friends, `FakeAudioContext`, the recording speaker).
  **Copy it; do not edit `AppSurface.test.tsx` itself** — it is on the untouched list.
- `AppSurface.test.tsx:242-243` is the proof this mount shape works with no `runtime` prop.
- `autoStart` must be left at its default `true` — `App.tsx:682-693` gates the `actions.start()`
  effect on it, and `AppSurface.test.tsx` passes `autoStart={false}`, which would make this test
  assert nothing.
- `focus-orb-reply-text` renders only once an envelope carries reply text (`App.tsx:958`), so flush
  the effect before asserting; `AppSurface.test.tsx:467` shows the `findAllByProps` pattern.

**Explicitly:** testing `resolveLaunchMode` in isolation (A7) is *not* a substitute for A3. A lane
that ships A1–A7 green and A3 absent, skipped, or asserting only on the pure resolver has not built
this feature and must be failed.

---

## 5. Done-definition

A lane is done when **all** of these hold, each with the literal output pasted into the evidence file:

1. `orb/apps/mobile/src/build/BuildModeLaunch.test.ts` exists, contains A1–A7 **and A3**, is tracked
   by git (`git check-ignore -v` finds no rule), and is green.
2. `cd orb && npm test` — full suite green. Paste the literal final summary line
   (`Test Files N passed | ...` / `Tests N passed`). A red line is reported as red; see §7.
3. `cd orb && npm run typecheck` — clean (this is what makes an illegal fourth launch mode a compile
   error rather than a runtime surprise).
4. `cd orb && npm run lint` — clean (`node tooling/boundary-lint.mjs`).
5. The §3.4 `git diff --numstat` proof command prints **nothing**.
6. Production diff is confined to exactly three files — `runtime/T0FocusSession.ts`, `App.tsx`, and
   the new `runtime/LaunchMode.ts` — and is under ~40 net new lines. `git diff --stat e421a59...HEAD`
   pasted in full. Any fourth production file in that diff needs an explicit written reason.
7. Evidence file written to `docs/evidence/F01-orb-build-mode-toggle.md` with the raw output of 2-6,
   failures included. No "it works" without it.
8. Status file updated per `Light/status/CONVENTION.md`
   (`Light/status/F01-Orb-build-mode-toggle.status` → `STATUS=verifying`), then
   `bash Light/status/render.sh`.
9. **Ship to a PR and stop.** Do not self-merge (author ≠ integrator).

---

## 6. Which existing tests prove "focus mode is unchanged and still green"

Named, so this is checkable rather than assumed. `orb/apps/mobile/src/runtime/T0FocusSession.test.ts`
is the primary witness — it holds 35 cases, and two of them pin exactly what F01 could break:

- **`T0FocusSession.test.ts:723-731`** — `it('B4: mode is explicit on every envelope and matches the
  pipeline that produced the turn (focus path)')`, whose first line is literally
  `expect((await runtime.start()).mode).toBe('focus')`. This is the single most load-bearing existing
  assertion for this lane: it is the regression guard on the exact line (`T0FocusSession.ts:346`)
  F01 edits. It must stay green **and its source must be byte-identical**.
- **`T0FocusSession.test.ts:62-65`** — `expect(screenModelFromEnvelope(await runtime.start()))
  .toMatchObject({ stateLabel: 'INTAKE', speechText: "I'm here. Tell me the task." })`. This pins
  *both* halves of the line F01 edits: the default greeting (`GREETINGS[0]`, catching an accidental
  reordering or extension of the array — §2.4) **and** the session state `start()` leaves the FSM in.
  It is the pre-existing counterpart to this contract's A5, and it means a builder who changes what
  `dispatch(state,'tap')` does will be caught by a test they are forbidden to edit.

Supporting witnesses, all of which must stay green and unedited:

| File | Cases | What it protects here |
|---|---|---|
| `orb/apps/mobile/src/runtime/T0FocusSession.test.ts` | 35 | the whole focus runtime; B4 mode assertions at 723-731, 733-750, 762, 804 |
| `orb/apps/mobile/src/runtime/ConversationPort.test.ts` | 4 | the port and its mode transmission stay untouched |
| `orb/apps/mobile/src/build/BuildModeWire.test.ts` | 2 | `mode:'build'` on the wire; absent mode omitted, not null |
| `orb/apps/mobile/src/AppSurface.test.tsx` | 6 | the composition root still mounts (this is what A3's harness is modelled on) |
| `orb/apps/mobile/src/AppVoiceWiring.test.tsx` | 6 | the real-App mic/voice path survives a new prop on `FocusOrbAppProps` |
| `orb/apps/mobile/src/AppController.test.ts` | 4 | `createFocusOrbActions` / `runtime.start()` orchestration (`AppController.ts:149`) |

"Unchanged" means **byte-identical**, and is proved by the §3.4 command showing zero changed lines
under those test paths — not by the suite merely being green. A builder who edits a test to make it
pass has inverted the contract; T1/T2 says the test is the spec.

---

## 7. Blocking precondition — S0 baseline (read this before writing any code)

**`node_modules` is absent from both `Light/orb/` and this worktree's `orb/`.** A research pass
confirmed `npm test` currently fails with `sh: vitest: command not found`. The orb suite has therefore
**never been run in `Light/` at all**, and FEATURES.md's S0 row ("confirm each copied repo's own
pre-existing verify gate still passes unchanged in its new location") is genuinely unestablished for
this repo. So:

1. **Before editing anything**, run `cd orb && npm install` (or `npm ci` if a lockfile permits) in
   *this worktree*, then `npm test`, and record the result as the **baseline** in the evidence file.
   Install into the worktree; do **not** symlink or copy a `node_modules` from a sibling directory —
   `file:` dependencies resolve from the real path and a symlinked tree makes the app's own
   `node_modules` invisible.
2. If the baseline is **red**, that is an S0 finding, not an F01 failure. Record the exact failing
   test names, set `STATUS=blocked` with a one-line `NOTE`, and report up. Do **not** fix unrelated
   red tests inside this lane, and do **not** build on top of an unknown baseline — without it,
   "focus-mode's suite is still green" is unfalsifiable.
3. Only tests that were green at baseline count as regressions if they go red after the change.

Landmines already paid for, do not rediscover: **never `git stash` here** (`refs/stash` is one shared
stack across every worktree of this repo — use `git worktree add --detach` for a throwaway baseline);
**never background a slow command and wait for it** (a subagent gets no notification for its own
background children — run in the foreground or with an explicit `timeout N`, e.g.
`timeout 900 npm install`); and re-read §3.3 on the `build/` gitignore negation.

---

## 8. Assumptions, and what this lane deliberately does not do

**Assumptions**
- "Launching in build-mode" is a **launch-time** selection. A mid-session focus↔build switch is not
  in scope (§2.3), even though the build FSM already reserves `enter_build` for it.
- A prop plus an env var is an acceptable P0 selector. There is no product decision yet on a visible
  toggle, and `App.tsx`'s header states the component renders no visible buttons — adding permanent
  chrome to the orb is above this lane's pay grade. `ORB_LAUNCH_MODE` follows the existing
  `ORB_RELAY_WS_URL` / `ORB_STT_PROVIDER` precedent.
- The build opening is spoken **locally and deterministically**, exactly like the focus greeting:
  `start()` makes no atomizer and no gateway call today, and F01 does not add one. Build mode
  therefore costs ₹0 at launch and needs no backend change to demonstrate.
- The exact greeting wording (`'Build mode. What are we making?'`) is a lead call, not a researched
  one. It is pinned so the builder cannot drift; if a human wants different copy, that is a one-line
  change to a named constant plus the A2 assertion.

**Explicitly NOT in this lane** — each is its own FEATURES.md row, and shipping any of it here makes
the diff unreviewable:
- The freeze protocol / propose / pushback / FREEZE_CONFIRM loop (**F05**).
- Depth / completeness / belief registers (**F04**) and the `lld-ready` gate (**F06**) — note
  `build/LldReadyGate.ts` already exists and is *not* to be wired up here.
- The module-brief atomizer (**F03**) — build mode in F01 does **not** change what an utterance
  decomposes into.
- The `lld.v1` schema (**F02**), the fleet handoff (**F09**), status echo (**F11**).
- Turn-level build routing: what a *reply* inside a build session is tagged. Only `start()` changes;
  the other 22 `makeEnvelope` call sites keep their current literals (§3.1c).
- Any change to the build FSM in `orb/apps/mobile/src/build/` — it is installed, and F01 does not
  dispatch into it. Wiring `buildDispatch` into the runtime is a later feature.
- Resolving the `CLARIFY` 9-states-vs-8-rows inconsistency in the session FSM (§2.2). Real, open,
  and someone else's change.
- Rotating build greetings, build-specific prosody, a build system-prompt on the relay side.

---

## 9. Handoff

Builder starts by setting `Light/status/F01-Orb-build-mode-toggle.status` to `STATUS=building` with
their own `AGENT=` and a fresh `SINCE=`, then runs `bash Light/status/render.sh`. Verifier re-derives
§5 from scratch in a clean checkout using only what is written here, and treats the builder's report
as unproven until each command in §5 has been re-run by hand.
