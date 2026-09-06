# 02 — The Always-On Audio Engine (Invariant #1)

> **Purpose:** the one guarantee the whole product is built to protect — **the user is never disconnected,
> not for a second, from app-open to session-end**. Why the bed is *generated* rather than shipped; how it
> stays gapless **in pure React Native with no custom native module**; ducking, interruption recovery, and
> background warmth; the battery/CPU/memory numbers; and the exact suite that proves 0 ms of accidental
> silence. This is invariant #1: every other plane degrades before this one does
> ([09 §2](09-FAILURE-DR-AND-DEGRADATION.md)).

---

## 1. The invariant, as a contract

For an ADHD brain, a moment of unexplained silence is not a UX blemish — it is an **understimulation
event**: arousal drops below the threshold where the brain goes hunting for dopamine (the phone, a tab, a
thought), and the session dies. The contract:

> **While the app is foreground and a session is live, `bed_active == true` at all times**, except three
> **sanctioned** silences: (a) a bounded ≤ 300 ms interruption window (phone call / Siri), always followed
> by a cover; (b) the deliberate fade-out at `SESSION_DONE`; and (c) **a user-intended pause** — the app is
> backgrounded, audio is disconnected, or the user pauses (§5.1), where silence is *exactly what the user
> asked for*. **Any other silence is a defect (a "gap") and is merge-blocking.**
>
> **The distinction that matters:** silence the user did not ask for reads as *abandonment* and is the
> product dying. Silence the user **caused** reads as *respect* — and failing to deliver it is worse,
> because an app that keeps the mic open when you've left it is a trust violation (§8).

- **Gap budget: 0 ms.** Formally: **0 audio underruns per session**, bounding the worst inter-buffer gap
  below one buffer period (~5–10 ms) — inaudible, and we assert zero even of those.
- **The bed starts at app-open, not at session-start** ([12 §2](12-LLD-AND-SESSION-CONTRACT.md)) — the orb
  is alive the instant the user arrives, with no network and no model call.
- **The bed is the substrate, not a layer.** Voice and cues are **mixed on top** and **duck** it; they never
  stop or replace it (§4). Silence is used exactly once, as punctuation (§6).

## 2. Generated, not shipped — and how the loop stays seamless

A shipped `rain.wav` has two costs: **storage/download** (a 60 s 48 kHz stereo WAV ≈ 11 MB) and a
**perceptible loop period** the ear learns. We generate the buffer procedurally instead, at startup, in JS:

- **Brown noise (1/f², the default — warm, sustainable over 25 min where white fatigues):** a leaky
  integrator over white noise; each sample depends on the previous output (`y = clamp(y·0.998 + w·0.02)`).
- **Pink noise (1/f, the "thinking" color):** Paul Kellet's refined method — a 7-coefficient cascade
  (`b0..b6`) approximating a −3 dB/octave filter.
- Both are the algorithms the audio library's own noise-generation guide documents, so this is the
  supported path rather than a clever detour.

**The honest detail (corrects the naive "procedural ⇒ no loop point"):** a generated *buffer* is finite, so
looping it reintroduces a seam. Two fixes, both applied:
1. **A long buffer** — **30 s**, not the guide's 2 s starter, so any residual periodicity is far below
   conscious detection over a 25-min session. Cost: `30 × 48,000 × 4 B ≈ 5.8 MB` mono (stereo is
   decorrelated with a cheap all-pass, not a second buffer).
2. **A tail↔head crossfade** — the buffer's last ~250 ms is crossfaded into its first, so the wrap is
   continuous in both amplitude and (for a stochastic signal) spectrum. Noise is the *easiest* signal to
   loop invisibly precisely because it has no pitch or phrase to betray the wrap.

**Generation cost:** 1.44 M samples × ≤ ~10 FLOPs ≈ **< 15 MFLOP, once, at startup** (single-digit ms) —
then the audio thread only *reads* the buffer. Steady-state DSP cost is effectively zero; the engineering is
not the DSP, it's never letting the OS take the stream away without a designed recovery (§5).

## 3. Gapless in pure React Native — the structural argument, without a native module

**We write no custom native audio code.** The bed runs on
[`react-native-audio-api`](https://github.com/software-mansion/react-native-audio-api) (Software Mansion,
v0.13.x as of Aug 2026) — a **Web Audio API implementation for React Native** whose rendering happens on a
**C++ audio thread** the library owns. We declare an audio graph from JS/TS; we never sit in the render
loop.

```
AudioContext
 └── AudioBufferSourceNode(noiseBuffer, loop = true)   ← the bed, started once, never stopped
      └── GainNode  (bed gain; ducked/restored by SCHEDULED ramps)
           └── BiquadFilterNode (lowpass; cutoff swept = the brown↔pink "color morph")
                └── destination
 └── AudioBufferSourceNode(voice/cue chunks)  → GainNode → destination   ← mixed OVER the bed
```

**Why this is still a structural guarantee and not "we optimized JS":** Web Audio is **declarative and
sample-scheduled**. `gain.setTargetAtTime(...)`, `linearRampToValueAtTime(...)`, and `source.start(when)`
hand the audio thread an *instruction with a timestamp*; the C++ thread executes it whether or not JS is
busy. **JS is never in the per-buffer render path**, so a 40–80 ms JS GC pause — the exact thing that broke
the first prototype (§12.1) — cannot produce a gap. The conclusion of the original native-module argument
survives; the native code we'd have hand-written does not.

**What this costs us (R21, honestly):** we now depend on a **pre-1.0 third-party library** owning the most
load-bearing guarantee in the product. That is a real risk, and it's a *dependency* risk, not an
architectural one. The fallback ladder, in order:
1. Pin the version, and gate upgrades on the full audio suite (§11) — a library bump is a **merge-gated
   change**, treated like a model-version bump ([05 §5](05-DETERMINISM-AND-LLM-CONSISTENCY.md)).
2. If a release regresses gaplessness: hold the pin, file upstream (it's actively maintained, OSS).
3. If the library is abandoned: `react-native-track-player` looping the same generated buffer (coarser
   control, still gapless playback) — a **degradation of expressiveness, not of the invariant**.
4. Last resort only: the thin native module we deliberately avoided. Named so the escape hatch is known, not
   so it's planned.

## 4. Ducking and mixing — lower, never pause

Voice and cues are additional nodes into the **same graph**, so they share one clock and cannot drift
against the bed. When the orb speaks:

1. The bed's `GainNode` is ramped from nominal (−18 dBFS) to a duck level (−30 dBFS) with
   `setTargetAtTime` over ~80–150 ms — **scheduled**, so the ramp is smooth even if the JS thread hiccups.
2. Voice plays through its own gain node, mixed over.
3. The bed ramps back. **The bed's source node is never stopped, paused, or recreated** — only its gain
   moves.

A cue (step-ready chime, the "win" swell) is the same: mixed over, never interrupting. The user hears the
orb speak *through* an unbroken line.

## 5. The interruption & route-change recovery matrix (the real engineering)

The only thing that can silence the bed is the OS reclaiming audio. These are **normal and designed-for**,
each recovering ≤ 300 ms behind a cover. The library surfaces the platform session/focus events; the
handling policy is ours:

Events split into two classes, and **misclassifying one as the other is the bug**: a *transient* interruption
(the user didn't leave — resume) versus a *user-intent* signal (the user stepped away — **pause and release
the mic**).

| Event | Class | Behavior | User feels |
|---|---|---|---|
| Phone call begins/ends | transient | on end/focus-gain: resume context, ramp bed up ≤ 300 ms + soft swell | "it waited, it's back" |
| Siri / assistant | transient | same | seamless |
| Audio stack reset / context death | transient (nuclear) | **rebuild the AudioContext + graph**; the one path that may exceed 300 ms → cover with a spoken "I'm back with you" | a designed return, never mystery silence |
| **App backgrounded / loses focus** | **user-intent** | **PAUSE**: stop the bed, **release the mic**, close the STT socket, show the paused indicator (§5.1) | "it stopped when I left — good" |
| **Bluetooth / headphone disconnect** | **user-intent** | **PAUSE** — do *not* silently reroute to the phone speaker | never blasts a private session into a room |
| Screen lock | **user-intent** | **PAUSE** | the session isn't secretly running in a pocket |
| Another app plays media | user-intent (implicit) | **PAUSE** + offer resume on return | the user chose other audio |

**The worst-case contract for the transient class:** if recovery *cannot* meet 300 ms, the system doesn't
pretend — it **speaks a cover line** so the return is a designed moment.

### 5.1 The pause contract (privacy is the feature)

An always-listening companion earns its place only if the user is *certain* it isn't listening when they
don't want it to be. So a pause is total and visible, never a mute:

1. **Bed stops** (a short ~250 ms fade, not an abrupt cut — an abrupt cut reads as a crash).
2. **Mic is released** at the OS level — not "ignored," *released*, so the platform's own recording
   indicator goes out. That indicator is the user's proof, and it must be honest.
3. **The STT socket closes** — no audio is in flight, and nothing is billed ([03 §3](03-VOICE-LATENCY-PIPELINE.md)).
4. **The UI states it plainly** — "paused — not listening" — and resuming is one deliberate tap.
5. **Nothing is captured while paused.** Not buffered, not "held for context." The gap in the session
   record is simply a gap.

**Resume** is explicit (the user returns and taps) or, on a transient interruption, automatic with a cover.
**The session does not silently restart itself when the app comes back to the foreground** — the user
decides when listening resumes.

**And the pause is not a dead end for the product:** backgrounding is one of the strongest *evidence*
signals we can actually observe — the user left, and for how long. It feeds the Attention register
(External Interruption) and, on return, the drift/re-anchor path
([13 §5](13-COGNITIVE-STATE-AND-POLICY.md)). We honor the privacy boundary **and** learn from the event;
those were never in conflict.

## 6. Silence as one punctuation mark

The only sanctioned silence is the **deliberate fade-out at `SESSION_DONE`** — a ~2 s ramp that *means*
"we're done, well done." Because the user only ever hears the bed stop at the end, a stop at any other time
is unambiguous evidence something broke — which is exactly what the watchdog (§11) asserts cannot happen.

## 7. The bed as an ambient status channel

Status is carried by the **color of the same continuous stream** rather than the orb narrating — quieter
than a talking avatar, and it never breaks the line. The frontend mirrors this visually from the
`orb.emotion` / `orb.bed_color` fields of the response envelope
([12 §6](12-LLD-AND-SESSION-CONTRACT.md)):

| Session state | Bed | How |
|---|---|---|
| Idle / working | **brown**, low, steady | the default line-open tone |
| Thinking (crew forming steps) | morph **toward pink** | sweep the lowpass cutoff up over ~400 ms |
| Step ready / win | brief **swell** + soft chime over the bed | scheduled gain bump + mixed cue |

This is the VR body-doubling study's "self-talk cue on idle/milestone" rendered as sound on one unbroken
stream — presence without chatter.

## 8. Foreground-only: the warmth lever, deliberately given up

An earlier design kept the bed playing under iOS background-audio mode / an Android foreground service so
the process stayed warm and an on-device timer could still check in with the screen off — a partial path to
"ping me" without a push backend. **That is rejected**, and the reasoning is worth stating because the
tradeoff is real:

| | Background-warmth (rejected) | **Foreground-only pause (chosen)** |
|---|---|---|
| Can nudge you after you leave | ✅ partially | ❌ — needs push, deferred ([README §9.1](README.md)) |
| User can be *sure* it isn't listening | ❌ ambiguous | **✅ mic released, OS indicator off** |
| Battery while away | continuous drain | **~0** |
| "Is this thing recording me?" | the question exists | **the question is answered** |

**Why trust wins:** for a companion that hears you all day, an ambiguous listening state is a
product-ending risk — one screenshot of a mic indicator while the app is backgrounded is the whole story.
The feature we lose (background nudging) was **already deferred to Phase 2** and belongs to push
notifications, which is the honest mechanism for it. We are not giving up a capability we had; we are
declining to fake one at the cost of trust.

**Battery, consequently:** the bed only runs foreground during an active session, so the draw is bounded to
session time — a 25-min session at **≤ ~4 %/hr** ≈ **1–1.7 %**, *(to be confirmed on-device with the
platform energy log — [TBM], R1)*. And the standing rule survives: the bed is always **user-initiated and
one-tap mutable**, with a visible live indicator. A sound the user didn't start is an uninstall.

## 9. Android's long tail (R5: honestly)

iOS is the reference target; Android is the harder one:
- **Audio-HAL latency varies** by OEM; the low-latency path isn't guaranteed on every device → a
  capability probe at launch with a documented higher-latency fallback (a larger buffer — more latency,
  same gaplessness).
- **Aggressive OEM battery killers** kill foreground services despite the notification → the warmth lever
  (§8) is weaker there; mitigation is a battery-optimization-exemption prompt, then honest foreground-only
  behavior.
- Named, bounded risks; full Android hardening is Phase 2 ([README §9.6](README.md)).

## 10. The numbers (the spec table)

| Quantity | Value | Basis |
|---|---|---|
| Sample rate | 48 kHz | platform default |
| Buffer (render quantum) | ~5–10 ms | Web Audio render block |
| Noise buffer length | **30 s**, crossfaded tail↔head | §2 (imperceptible loop period) |
| Noise buffer memory | **~5.8 MB** mono float32 | 30 × 48,000 × 4 B |
| Generation cost | < 15 MFLOP once at startup (single-digit ms) | §2 |
| Steady-state DSP | ~0 (buffer read + gain + one biquad) | §3 |
| Shipped audio asset | **0 MB** (generated) | vs ~11 MB for a 60 s stereo WAV |
| **Gap budget** | **0 ms (0 underruns/session)** | invariant #1 |
| Interruption recovery | ≤ 300 ms behind a cover | §5 |
| Battery (screen off) | ≤ ~4 %/hr *(TBM)* | §8 |
| Duck depth / ramp | −12 dB / 80–150 ms scheduled | §4 |

## 11. How to test (exactly)

1. **Continuous-audio watchdog** (debug always; sampled canary in prod): sample the context's playback
   position/underrun signal every render period during a live session and assert **no discontinuity**. Any
   break logs the timestamp + preceding state. **Gate: gap events/session-hour == 0 (merge-blocking).**
2. **Loudness-floor probe (the platform-independent gap test):** record the device's own output (loopback or
   an external capture rig) for a full session and assert **no window > 20 ms falls below the bed's noise
   floor** except the sanctioned final fade. This tests the *user-audible* invariant rather than a
   library-reported counter — it catches gaps the library doesn't know it caused.
3. **Interruption matrix as an automated suite** — one test per row of §5 (simulated call, Siri, route
   toggle, BT connect/disconnect, background/foreground, context death, Android focus loss/gain): assert
   **recovery ≤ 300 ms and the bed resumes**. The matrix is the gate; a new device quirk adds a row.
4. **Gapless soak:** 8 h on real devices — assert 0 gap events, and via FFT of the recorded output a
   **stationary spectrum with no audible periodicity** at the 30 s loop period (proves the crossfade works).
5. **Library-upgrade gate:** the full audio suite runs on any `react-native-audio-api` version bump; a
   regression holds the pin (§3).
6. **Battery test:** platform energy log across a 25-min screen-off session vs the ≤ ~4 %/hr budget — the
   [TBM] that makes §8 real.
7. **Perceptual gap-sensitivity panel:** inject controlled gaps (20/50/100/150/200 ms) for an
   ADHD-representative panel and record the noticed-gap threshold. If users reliably notice ≥ ~120 ms, the
   sub-buffer budget is earned; if nobody notices below 200 ms, we've learned the real tolerance and say so
   (R5).

## 12. Operator's scars

1. **The GC stutter.** An early prototype ran per-buffer mixing logic *from JS*; a routine ~60 ms GC pause
   produced an audible break. The fix was not "optimize JS" — it was moving to a **declaratively scheduled**
   graph where JS never sits in the render loop (§3). Same conclusion as writing a native module, without
   writing one.
2. **The 2-second loop.** Following the library guide's 2 s starter buffer verbatim produced an audible
   periodicity within minutes on headphones — testers described it as *more* distracting than helpful. Fixed
   by the 30 s crossfaded buffer (§2). A tutorial default is not a production parameter.
3. **The uncovered return.** An early build resumed the bed instantly after a phone call — correct on
   latency, wrong on experience: abrupt silence→noise on hang-up was jarring. Recovery now always rides a
   cover (§5); the *return* is designed, not merely fast.
4. **The silent battery complaint.** A tester left a session backgrounded for an hour and blamed the app for
   drain — because nothing said the bed was still on. Now: always user-initiated, one-tap mute, visible
   live-indicator (§8). Presence you didn't ask for is a bug.
5. **Trusting a library counter.** An early gap regression didn't move the library's own underrun signal at
   all — the gap was upstream of what it measures. That's why the **recorded-output loudness-floor probe**
   (§11.2) exists: the invariant is defined by what the *user hears*, so the test must listen.
6. **Rerouted a private session to the room.** On a Bluetooth disconnect the graph "helpfully" fell back to
   the phone speaker — playing someone's focus session out loud in an open office. Disconnect is now a
   **pause**, never a reroute (§5). The polite-looking default was the harmful one.
7. **Kept the mic warm through a backgrounding.** An early build kept the session live when the app lost
   focus, "so resume would be instant." A tester saw the OS mic indicator on while using another app and
   uninstalled immediately — and was right to. Backgrounding now releases the mic at the OS level (§5.1);
   convenience never outranks a visible listening indicator.

## 13. Interview questions this file answers

- "Your core is 'never silent' — how do you *guarantee* it?" (§1, §3 — declarative scheduling, 0 gap events)
- "You're on React Native with no native module — how is real-time audio possible?" (§3 — Web Audio's scheduled graph; JS is never in the render loop)
- "What if that library regresses or dies?" (§3 — the pinned-version gate and the 4-step fallback ladder)
- "Why generate the noise instead of shipping a file — and doesn't a loop have a seam?" (§2 — 30 s buffer + tail↔head crossfade, honestly)
- "A phone call comes in mid-session. Walk me through it." (§5 — the recovery matrix)
- "How do you keep it alive backgrounded, and what does it cost?" (§8)
- "How would you *test* 'never silent'?" (§11 — the watchdog, and the recorded-output probe that catches what the library can't)
