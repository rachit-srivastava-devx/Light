# Voice-UX human review — 15 minutes, binary answers

The machine half of the UX bar lives in `ux-gate/gate.py` (run it against a drive directory). This
file is the half a machine cannot judge: **does it feel right to a person.** Twelve observations, each
answerable yes/no in one listen. Do not skip the "what wrong looks like" column — it is there because
a reviewer who does not know the failure mode tends to mark everything pass.

Every item is a real product invariant, not taste. Where a number appears it comes from the
blueprint's own targets or the researched UX literature (sources in
`scratchpad/RESEARCH-simulated-user-and-voice-ux.md`).

## Before you start (2 min)

```bash
cd company/products/adhd-focus-orb
. scripts/load-env.sh                 # must be dot-sourced
npm run mobile:start                  # Metro, in its own terminal
bash scripts/dev.sh                   # relay + sidecars, in another terminal
```
Then launch the app on the simulator (device `ADHD Focus Orb E2E` exists) or a real device.
**Use headphones.** Several items are about audio texture and cannot be judged on laptop speakers.

Record as you go: `xcrun simctl io booted recordVideo /tmp/orb-review.mov` gives you video+audio to
re-check anything you are unsure about. Unsure = **fail**; a genuine pass is obvious.

## Known-current state (so you judge the product, not a known bug)

- Reply latency is **p50 ≈ 1.7 s, worst ≈ 3.4 s** — measured, real, and **6.8× over the 250 ms
  target**. Items H4/H5 are about how that *feels*, not whether it is fast.
- On the **simulator** the app shows "Mic error — check console" because a simulator has no audio
  input; the app still works via injected audio. On a **real device** the mic should work — if you
  see that message on hardware, that is a genuine failure of H11.
- `[breathing]`, `[warm]`, `[short pause]` in a reply are **intentional Fish Audio prosody tags**.
  If you *hear* those words spoken aloud, that is a real defect (and means the TTS provider changed).

---

## The twelve observations

| # | Observation | Pass looks like | What wrong looks like |
|---|---|---|---|
| **H1** | **App-open presence.** Open the app and say nothing for 20 s. | Something is audibly there within a moment; it never goes fully silent; the orb looks alive. | Dead silence. A static image. You wonder if it crashed. |
| **H2** | **Greeting.** The first thing it says. | Warm, short, human, uses your name naturally. | Robotic, generic, or a wall of text. Or it greets you twice. |
| **H3** | **States are distinguishable.** Watch/listen through one full turn. | You can tell *listening* vs *thinking* vs *speaking* vs *idle* without being told — visually AND audibly. | You cannot tell whether it heard you. You start repeating yourself. |
| **H4** | **The wait.** After you stop speaking, before it answers. | The gap feels like a person gathering a thought. You are not tempted to speak again. | It feels broken. You say "hello?" into the gap. |
| **H5** | **No unasked-for silence.** Any moment of nothing during a turn. | Ambience continues throughout; you never hear true silence you did not ask for. | An audible hole. The bed cuts out while it thinks. |
| **H6** | **Barge-in.** Talk over it mid-sentence. | It yields almost immediately and listens. Nothing stale plays afterwards. | It keeps talking over you, or finishes its old sentence after you stop. |
| **H7** | **Voice quality.** Listen to a full reply on headphones. | Sounds like a person: modulated, warm, varied. | Flat, buzzy, or obviously synthetic. **If it sounds robotic, the premium voice is not being used** — that is the product's canonical failure, not a nitpick. |
| **H8** | **Misrecognition repair.** Mumble something unintelligible. | It says it did not catch that, **in different words than its last reply**, and invites a retry. | Silence. Or it repeats its previous sentence verbatim. Or it guesses and acts on the wrong thing. |
| **H9** | **Distress handling.** Say "I'm completely overwhelmed and I can't start." | Acknowledges the feeling first; slows down; no push; offers something tiny or just presence. | Asks you for a "next step", sounds urgent/brisk, or is subtly disappointed in you. **Any shaming or hurrying register here is a critical failure.** |
| **H10** | **Not-a-task conversation.** Say something with no task in it — vent, ask its opinion, change the subject. | It just talks with you. Follows the topic change. Does not steer everything back to a task. | It tries to atomize your feelings, or asks for "the smallest possible step" when you were not asking for help. |
| **H11** | **Failure honesty.** Kill the network (airplane mode) mid-session. | Says plainly, **once**, that it cannot hear/speak right now; stays present; recovers when the network returns without a restart. | Silent death. Repeated error announcements. Developer text like "check console". A required force-quit. |
| **H12** | **Would you keep it open?** After 10 minutes of ordinary use. | You would leave it running while you work. | You want to close it. It nags, over-talks, or feels like a demo. |

---

## Scoring

Write the count, not an impression: **N of 12 passed.** Then, for every fail, one line: what you did,
what happened, what you expected. A fail with no reproduction is not actionable.

**H7, H9 and H11 are veto items.** Any one of them failing means the product is not shippable
regardless of the other nine, because each maps to a stated product invariant:
- **H7** — a degraded voice was the product's original silent-failure scar.
- **H9** — shame-adjacent registers are supposed to be *structurally unrepresentable*, not merely
  discouraged. `/v1/respond` had no server-side veto as of this writing (see the safety-guard work);
  a red-team run against the real model produced "maggot" and a shouted "LET'S GET THIS ENERGY UP".
- **H11** — presence must survive degradation; the bed is the last thing to stop.

## Where this fits

- Machine half: `python3 ux-gate/gate.py <run_dir>` — 8 automated checks with denominators.
- Simulated users: `e2e-human-simulator/` (9 journeys) and `evals/` (LangWatch Scenario).
- This file: the irreducibly human half. **None of the three substitutes for the others.** A gate that
  is green while H7 fails is the exact situation this product has already been burned by once.

---

## Opus-driven UI run — 2026-08-28 07:03–07:12 (evidence: `evidence/ui-drive-070313/`)

Driven on a **booted iOS 18.4 simulator** ("ADHD Focus Orb E2E") via `xcrun simctl` — stated
explicitly because the MCP simulator tool requires a device-access grant the owner was asleep to
give, so there was **no tap injection**: launch, render and log inspection only, no interaction.

### What is now PROVEN (not claimed)

| claim | evidence |
|---|---|
| The app builds, installs, launches and **renders** — no React Native red screen | `01-launch.png`, `** BUILD SUCCEEDED **` |
| The JS bundle actually evaluates on device | console: `ReactInstance: evaluateJavaScript() with JS bundle` |
| Audio session configures for voice | `category=PlayAndRecord mode=VoiceChat` |
| A UI copy change reaches the device | `02` → `03` before/after screenshots |
| **Fish TTS produces real speech** through the live sidecar | HTTP 200, 1.35 s, 57 chunks, 90,766 PCM16 samples, 95% nonzero, peak 18,588, RMS 2,531 |

### Defects found by looking at the screen

1. **`ios:build:check` passes while shipping an unlaunchable app.** `** BUILD SUCCEEDED **` and then
   `ios-launchable-check` FAILED: no embedded `main.jsbundle`. `npm run ios:bundle:local` writes
   `ios/build-js/main.jsbundle` and **no Xcode build phase consumes it** (`grep -c build-js
   project.pbxproj` → `0`). The script runs, reports OK, and its output is dead. A release/device
   install would render "No script URL provided". Only the Metro path (`--allow-packager`) is
   launchable today — which the check correctly labels "launchable here, NOT self-contained".
2. **No microphone is a hard dead end.** The whole session is gated on capture and there is **zero
   `TextInput` in `apps/mobile/src`**, so with the mic unavailable the user can do nothing at all.
   The relay logged **0 events** from the app across four launches, for exactly this reason.
3. **Developer copy was the only thing on screen.** "Mic error — check console" — an instruction to
   a user who has no console, with no way forward. **FIXED** this run (`App.tsx:653`) to
   "I can't reach your microphone right now. It may be in use by another app.", verified rendered in
   `03-after-copy-fix.png`. The copy is honest now; the dead end (item 2) is still real.
4. **`focus-orb:fish-unavailable` ×3 on launch** — because the voice sidecar on `:8083` was simply
   not running. Not a code defect, but the app's response to it is: a red error badge and silence,
   with nothing in the UI telling the user the voice is unavailable. Note the design here is
   deliberately correct — `native_tts_fallback: false` means a lost premium voice is LOUD, not a
   silent downgrade.
5. **No affordance of any kind.** A static orb, a status line, and nothing else — no button, no
   prompt, no hint. A first-time user has no idea what to do or that they should speak.

### Corrections to my own earlier reads, recorded so they don't propagate

- I reported the orb rendering as a "deformed polygon" from `02`. **Wrong** — it was a mid-animation
  frame; `03` and `04` show a clean circle.
- I first verified the TTS payload by reading its bytes as PCM16 and declared "real audio". The
  response is **JSON** (`{"audio_chunks": [[int,…],…]}`), so that measured JSON text as samples and
  proved nothing. The numbers in the table above come from decoding the chunks properly. Measuring
  the wrong quantity truly is the cheapest way to be confidently wrong.

### Still not covered

- **No interaction was driven** (no tap permission) — so barge-in, ducking, the 250 ms budget, and
  every gesture path remain unverified on device. `ux-gate/gate.py` was NOT run: it needs a run
  directory from an interactive drive, and running it on this evidence would have it pass while
  measuring nothing.
- H7 (voice naturalness), H9 (distress), H11 (failure honesty) still need the owner's ears. The Fish
  audio above proves the pipe carries real speech; it says nothing about whether it sounds good.

### The spoken path, proven without a human — 2026-08-28 07:16

The owner reported **"speaking no reply"** three times while 594 TS tests, 250 Python tests and a
green backend gate all passed. Three stacked service faults, none of them code, none covered by any
suite:

1. **`relay-rs` was not running.** The app's own console said so: `focus-orb:relay-transport-error
   endpoint=ws://localhost:8091`. Audio went into a dead socket.
2. **The voice sidecar was not running** — the `focus-orb:fish-unavailable` ×3 badge. With it up,
   those errors go to 0.
3. **`scripts/dev.sh` never set `ORB_RELAY_PROVIDER`,** and `provider.rs:76` defaults it to `fake`.
   So the official one-command dev startup ran the realtime relay on canned transcripts: the orb
   connects, the mic captures, the status says "Listening", and relay-py dutifully answers the fake
   string *"the final transcript"*. **FIXED** in `scripts/dev.sh` — defaults to the real provider
   pointed at the voice sidecar, still overridable with `ORB_RELAY_PROVIDER=fake`.
   `scripts/dev.test.sh` exit 0.

**`e2e-human-simulator/synthetic_mic.mjs`** now guards this. It synthesises a real utterance with the
live Fish TTS and streams that PCM into relay-rs as raw binary frames exactly as `RelayClient.ts`
does, then does what the app does with the result: POST `/v1/respond`, send `speak`, and count the
audio that comes back.

```
speech in    161,470 bytes / 92 frames   (Fish TTS, peak 29,289 — silence is rejected explicitly)
transcript   "My kitchen is a total disaster, and I cannot get started on it."
orb reply    "[sigh] [gentle] Oh wow, a messy kitchen can feel like a lot.
              [short pause] How about we just focus on clearing off one counter?"
             source=model  degraded=false
audio back   277,384 bytes  (speech_starting -> 90 binary frames -> speech_complete)
EXIT 0
```

It fails closed on each leg separately — no frames, a fake-provider marker, no transcript, no reply,
or **a reply with zero audio bytes** ("a reply the user cannot hear is not a reply"). The
fake-marker check exists because a stub transcript is a failure dressed as a pass, and that is
precisely what let three "no reply" reports coexist with a green suite.

That reply is also the corrected conversation behaviour running in the live voice loop, not a unit
test: it acknowledges before proposing, and the step is about the mess the user raised ("one
counter") rather than a substituted calmer activity.

**Honest limit:** this is a TTS -> STT round trip. It proves the transport, STT provider, relay,
model and audio-return legs are connected and carrying real content. It does **not** prove a human
voice transcribes well — real speech has accent, noise and disfluency that synthetic speech lacks.
H7 (naturalness) still needs the owner's ears.

### Multi-turn conversation, driven — 2026-08-28 07:35  (`npm run e2e:conversation`)

The owner's report was about everything BETWEEN turns: *"it does not have context of what I spoke
previously... not able to stop the conversation in between, divert or have a conversation. right now
its just one by one messaging not a conversation completely."*

Root cause was one function. `classifyIntakeUtterance` defaulted to `'task'`, `"let's"` was a
task-request prefix, and no rule recognised conversation control — so **every steering utterance
loaded the step-atomizer prompt**. Measured before → after:

| utterance | before | after |
|---|---|---|
| `let's create anything` | task | **converse** |
| `can we just chat` | task | **converse** + `chat_invitation` |
| `actually let's talk about something else` | task | **converse** + `divert` |
| `wait stop` / `hold on` | task | **converse** + `stop` |
| `no not that` | task | **converse** + `reject` |
| `help me clean the kitchen` | converse | **task** (was inverted) |

Task-side regression set: **9/9** still route to task. Non-Latin intake deliberately keeps the prior
path — every marker list here is English, so routing Devanagari to the new default would be a
different guess, not a better one. Stated as a known gap rather than hidden.

Also fixed: `'What is the smallest part to start?'` was a **hardcoded string in the CLARIFY state**
(`T0FocusSession.ts:611`), not model output — which is why it repeated verbatim and why the egress
guard could never see it. A guard on the model is not a guard on the product.

`e2e-human-simulator/conversation_drive.mjs` — 6 real spoken turns on ONE session, all checks passed:

```
T1 "I've been thinking about learning to cook properly lately."
   -> "Oh, that's wonderful! Learning to cook is such a rewarding skill."
T3 "Actually, can we talk about something else instead?"
   -> "Absolutely. What's on your mind instead?"
T5 "Wait! Stop!"
   -> "Okay, I'm stopping. What's up?"
T6 "Sorry, what were we saying about cooking earlier?"
   -> "We were just talking about how you've been wanting to learn to cook and that your
       kitchen is a bit of a disaster right now."
```

C1 context across 5 turns · C2 topic change · C2 stop · C3 no repeats · C4 no load handed back ·
C5 all 6 audible. **INFO: end-to-end p50 2662ms, max 3089ms** — STT alone is ~1.7s, so the 250ms
budget is nowhere near met on this path. Reported, not asserted.

`stop` now also **cuts the audio** (`App.tsx` transcript handler → `handleClosing('user_pause')` +
`bargeIn()`), before the ~800ms model round trip. Only `stop` interrupts: cutting playback on "no,
not that" would make ordinary disagreement feel like an error.

### Speaking rate — the "can't make that out clearly" report

Fish prosody had every emotion ABOVE 1.0 speed (default `warm` 1.04). Measured **3.4 words/sec**
against a natural 2.2–3.0; now **2.9 words/sec** (`MAX_SPEAKING_RATE` invariant test covers all
seven emotions plus an unknown one). Two other explanations were measured and falsified first: a
16-vs-24 kHz mismatch (refuted — real energy at 9–10 kHz, impossible below an 8 kHz Nyquist) and
Fish speaking the `[sigh]` tags aloud (refuted — +8%, consistent with an inserted pause).
