# 09 — Failure, DR & Degradation

> **Purpose:** how the system fails *safely* — the doctrine that **presence outranks intelligence** (the
> noise bed is the last thing to stop, [02](02-ALWAYS-ON-AUDIO-ENGINE.md)), the ordered degrade ladder,
> what works offline, the per-plane failure catalog, the DR posture for the small durable state, and the
> honest admission that the biggest failure mode isn't technical — it's the novelty cliff. The one true
> outage in this product is **silence**.

---

## 1. The failure philosophy: presence > intelligence > richness

For most apps the safe failure is "slow but correct." For an ADHD companion it is **"quiet-but-present, and
still moving you"** — never "fast but absent." A silent orb is not a degraded orb; it is a *dead* one,
because silence is the exact stimulus-drop the product exists to prevent ([02 §1](02-ALWAYS-ON-AUDIO-ENGINE.md)).
So the ordering, encoded in config (a daylight decision, not a 2am improvisation):

> **Presence > intelligence > richness.** Keep the bed and keep the user moving, even if the orb gets
> "dumber." Shed cleverness and polish long before you shed company.

A dumb, present body double ("I'm here — keep going, tell me when you're done") beats a smart, absent one
every time for this user.

## 2. The degrade ladder (shed in reverse value; the bed is priority 0)

| Order shed | Component | Trigger | Fallback | User feels |
|---|---|---|---|---|
| 1 (first) | Telemetry / eval sampling | any load | drop samples | nothing |
| 2 | Speculative generation ([03 §6](03-VOICE-LATENCY-PIPELINE.md)) | cost/latency pressure | wait for the endpoint | ~200 ms slower turns |
| 3 | Premium TTS for *novel* text | TTS provider down/slow | **cached premium phrases only** — the orb speaks a smaller vocabulary, still in its own warm voice | fewer new sentences, same companion |
| 4 | Crew LLM (novel atomization) | LLM outage / budget cap | **memory reuse → templated → deterministic starter step** | simpler steps, still present |
| 5 | Conversational replies | LLM outage | deterministic acknowledgments + backchannels only | fewer words, still doubling |
| 6 | Cloud STT (hearing) | STT outage / no network | **nothing to fall back to** — announce once, hold presence ([§3](#3-offline-mode-what-survives-no-network)) | "it can't hear me, but it's still here" |
| — (**never**) | **Noise bed · state machine · cached step audio · the pause contract** | — | — | **presence is unbroken; privacy is never traded for uptime** |

The bed is **priority 0**: it runs on-device, needs no network, and is the last thing standing
([02 §3](02-ALWAYS-ON-AUDIO-ENGINE.md)). Under a *total* cloud outage the orb becomes a **deterministic
body double** — bed on, a cached/templated first step, non-judgmental check-ins on the timer, "done"
advancing through a generic step set — dumb but fully present. That is the designed floor, not a crash.

## 3. Offline mode (what survives no network)

| Capability | Offline? | How |
|---|---|---|
| Noise bed | ✅ | procedural, on-device ([02](02-ALWAYS-ON-AUDIO-ENGINE.md)) |
| Session state machine + policy | ✅ | on-device arithmetic ([05 §2](05-DETERMINISM-AND-LLM-CONSISTENCY.md), [13 §4](13-COGNITIVE-STATE-AND-POLICY.md)) |
| Cached phrases, backchannels, pre-synth'd next steps | ✅ | local audio cache ([03 §2](03-VOICE-LATENCY-PIPELINE.md)) |
| Advancing through steps already in hand (tap/gesture) | ✅ | deterministic; no speech needed |
| **Hearing the user (STT)** | ❌ | cloud-only by decision ([03 §2](03-VOICE-LATENCY-PIPELINE.md)) — **no on-device fallback** |
| **Saying anything new (TTS)** | ❌ | cloud-only; only *cached* audio can play |
| **Atomizing a new task** | ❌ | needs the crew |

> **This is the honest cost of the voice-quality decision (R5).** The earlier design claimed a graceful
> offline mode via on-device STT/TTS. Those were rejected as unusably bad multilingual
> ([03 §2](03-VOICE-LATENCY-PIPELINE.md)), so **there is no degraded-voice fallback** — the orb goes
> *deaf and mute* without a network, and we do not pretend otherwise.

**What survives is still the thing that matters most: presence.** A session under way keeps its bed, its
cached step audio, and its state machine — so the user is *accompanied* and can keep moving through steps
already in hand via tap. What stops is *conversation*. The orb says so plainly, once, in cached audio
("I've lost the network — I'm still here, but I can't hear you until it's back"), then holds presence
rather than failing silently. **Honest deafness beats a robotic voice mangling Hinglish**, which was the
alternative on offer.

## 4. Per-plane failure catalog

| Plane | Failure | Caught by | User sees |
|---|---|---|---|
| Presence ([02](02-ALWAYS-ON-AUDIO-ENGINE.md)) | audio underrun / OS reclaim | continuous-audio watchdog (0-underrun assert) + interruption matrix | ≤ 300 ms cover, then bed back — never bare silence |
| Voice ([03](03-VOICE-LATENCY-PIPELINE.md)) | STT/TTS slow or down | per-hop timeout + on-device fallback | plainer voice / slightly slower turn |
| Session ([05](05-DETERMINISM-AND-LLM-CONSISTENCY.md)) | bad model output | schema validate → repair-once → fail-closed (INV2) | a safe deterministic step, never a broken one |
| Cognitive ([04](04-MODEL-ROUTER-AND-THE-CREW.md)) | LLM provider outage | cross-provider failover → deterministic fallback | simpler steps, still present |
| Backend ([07 §3](07-CAPACITY-AND-LATENCY-MATH.md)) | API/store down mid-session | on-device state is the in-session truth; write-behind | session continues; durable write catches up |

**The unifying rule:** every plane fails *toward presence*. No failure path is allowed to end in silence
during a live session — that's the one invariant the whole catalog protects.

## 5. The real failure mode: the novelty cliff (R5 — honestly)

The biggest risk to this product **is not technical.** Focus/ADHD tools have weak proven retention (Doppel:
2 ratings at launch, [README §9](README.md)); ADHD users are novelty-seekers who love a tool for a week and
ghost it. **None of the engineering in this folder fixes that** — a gapless bed and a 99.4 % atomizer keep
the *session* good, but they don't guarantee the user comes back on day 20. This is named as the weakest
link, and it's *why* Phase 2 is proactive presence + interrupt-timing ([README §9.1/§9.4](README.md)):
the habit-formation loop, not the session loop, is the retention bet — and it is deliberately deferred, not
solved. Claiming otherwise would be dishonest.

## 6. DR & data

- **Durable state is small** ([07 §3](07-CAPACITY-AND-LATENCY-MATH.md)): current todos + recent session
  records (a few KB/session). Hot set < 500 MB; cold logs on object storage.
- **RPO/RTO:** managed Postgres (Supabase) point-in-time backup → **RPO ≤ 24 h** (a lost recent session is
  low-stakes — the *value* is the live session, not the archive), **RTO minutes** (stateless backend
  redeploys; on-device state carried the live session anyway).
- **Mid-session backend loss is survivable by design:** the on-device state machine is the source of truth
  during a session ([01 §5](01-SYSTEM-OVERVIEW-AND-PLANES.md)); the durable write is write-behind, so a
  backend blip never interrupts the user — it just reconciles when the backend returns.

## 7. The 2am operator

- **The one alarm that matters:** the **audio-gap watchdog** (underruns/session-hr > 0) and
  **think-time-silence > 0** ([02 §11](02-ALWAYS-ON-AUDIO-ENGINE.md), [03 §7](03-VOICE-LATENCY-PIPELINE.md)) —
  because those are the only failures the user experiences as the product dying. Everything else degrades
  quietly per §2.
- **The degrade ladder is config, not code** — priorities set in daylight ("presence > intelligence >
  richness"), so a 2am pager doesn't require a judgment call; the system already knows what to shed.
- **Runbook per failure** ([§4](#4-per-plane-failure-catalog)): each row's fallback is the runbook. The
  default failure mode is the *safe* one (fail toward presence), so "do nothing and let it degrade" is
  usually correct.

## 8. Operator's scars

1. **"Fast and wrong" nearly shipped.** An early degrade path, under LLM latency, skipped the filler and
   answered faster with a worse step — optimizing latency into the exact failure (a bad step, fast) the
   product can't afford. The ladder now sheds *cleverness and richness*, never presence or correctness (§1).
2. **The offline dead-end.** v0 simply errored when a new task couldn't be atomized offline — leaving the
   user staring at a silent orb (the worst outcome). It now degrades to a deterministic starter step + the
   bed (§3); a dumb step beats a dead orb.
3. **Telemetry took down the session.** A synchronous eval-sample write on the session path added latency
   and, once, failed and blocked a turn. Telemetry is now fire-and-forget and **first to shed** (§2) — the
   measurement system must never be able to break the thing it measures.
4. **Mistook cost-viability for product-viability.** The cost model ([08 §8](08-COST-MODEL.md)) looked so
   healthy it was tempting to call the product de-risked. Retention (§5) is the real risk, and no amount of
   unit-economics headroom touches it — naming that kept the roadmap honest.

## 9. Interview questions this file answers

- "How does this degrade under load or a provider outage?" (§2 — the ladder; bed is priority 0)
- "What's the *safe* failure mode for an ADHD companion specifically?" (§1 — present-but-dumb, never absent)
- "What works with no network?" (§3 — most of a live session; new atomization degrades, doesn't die)
- "Backend goes down mid-session — what does the user experience?" (§4, §6 — nothing; on-device is the in-session truth)
- "What's your biggest risk?" (§5 — the novelty cliff; honestly, not a technical one)
- "What's the one alarm you'd wake up for?" (§7 — the audio-gap / think-time-silence watchdog)
