# 08 — Cost Model

> **Purpose:** the authoritative ₹ build-up — cost per session line by line, why **voice is the dominant
> line** (the finding that moved the ceiling), the API-vs-self-host crossover with break-even math, the
> per-user/month result against the ≤ ₹120 ceiling, the free-tier consequence, the in-path cost-control loop that
> makes the ceiling structural, and the sensitivity check. All money reconciles here and to
> [README §6](README.md); if a number changes, it changes here first (R6/R7).

---

## 1. Cost per session (the build-up)

Unit = **one ~25-min focus session** (demand split in [07 §1](07-CAPACITY-AND-LATENCY-MATH.md): ~2.5 speech-min,
~500 novel TTS chars, ~5 LLM calls). Aug-2026 prices, $1 = ₹95 ([README §6](README.md)):

| Line | Basis | ₹/session |
|---|---|---|
| **Voice — STT (cloud, streaming)** | ~3 min **billed** (speech + socket margin, [03 §3](03-VOICE-LATENCY-PIPELINE.md)) × Sarvam ₹0.50/min (₹30/hr) · Deepgram ₹0.73/min | **~₹1.5–2.2** |
| **Voice — TTS (premium, live)** | ~500 novel chars × Fish $15/1M (₹1.43/1K); fixed phrases + backchannels **cached = ₹0** ([03 §2](03-VOICE-LATENCY-PIPELINE.md)) | **~₹0.7** |
| **LLM (brain)** | ~5 calls, ~6k in / 1k out Flash-Lite, cached prefix ([04 §4–5.1](04-MODEL-ROUTER-AND-THE-CREW.md)) + endpoint/speculation overhead | **~₹0.2** |
| Infra (relay VM, store, bandwidth) | ~₹3k/mo VM ÷ 180k sessions/mo ([07 §3](07-CAPACITY-AND-LATENCY-MATH.md)) | **~₹0.05** |
| **Default total** | | **~₹2.5–3.2/session** |
| *self-host TTS variant* | Fish Speech / Kokoro self-host (§3) | −₹0.65 |
| *self-host STT (at scale)* | breaks even ~20K, clear win ~50K+ (§3.1) | −₹1.0–1.7 |

**Voice is now ~85–90 % of marginal cost, and STT alone outweighs everything except TTS.** The LLM — the
thing everyone assumes dominates an "AI app" — is **~7 %**.

> **R5 — the number has moved against the story twice, and both times for the same reason.** ₹0.20/session
> (LLM only) → **₹0.9** (a non-robotic voice) → **~₹3** (on-device STT/TTS rejected outright as unusable
> multilingual, [03 §2](03-VOICE-LATENCY-PIPELINE.md)). Each step was a deliberate refusal to compromise the
> product's face, and each raised the ceiling: **₹15 → ₹40 → ₹120**. Stating the trajectory is the point —
> a cost model that only ever improves is a cost model that isn't being tested against real decisions.

### 1.1 The free-tier consequence (the thing this cost actually breaks)

At ~₹3/session, a free user doing 30 sessions/month costs **~₹90** — and no ad-supported or
conversion-funded model survives that. The paid economics are *fine* (§8: ~88 % margin); the **free tier is
what the quality decision breaks**. So the free tier must be **session-capped, not time-capped**:

| Tier | Sessions/mo | Cost to serve | Verdict |
|---|---|---|---|
| Free | **5** | **~₹15** | sustainable as acquisition |
| Paid | ~30 | ~₹90 | ~88 % margin at a ₹760 price point (§8) |

A session cap is also the *honest* free tier for this product — 5 real focus sessions is a genuine trial of
the loop, where a time cap would cut a user off mid-session, which is precisely the abandonment the product
exists to prevent.

## 2. Why voice dominates (and the two levers that keep it affordable)

The alternative — cloud **speech-to-speech** for the whole 25 min — is **₹47–260/session**
([README §6](README.md), sourced). A neural-TTS pipeline is 40–600× cheaper *and* keeps determinism
([04 §5](04-MODEL-ROUTER-AND-THE-CREW.md)). Two mechanisms cut it further:
- **Cache every fixed phrase** (fillers, wins, common check-ins) → synthesized once, ₹0 marginal, at
  premium quality. Only the ~500 novel step-chars are billed ([03 §4](03-VOICE-LATENCY-PIPELINE.md)).
- **Pre-synthesize the next step while the user works** — same billed chars, but the audio is cached
  locally so a network drop doesn't cost a re-synth.
Result: **live-TTS chars are ~500/session, not ~1,500** — a ~3× cut before a rupee is spent.

## 3. API vs self-host — the crossover (break-even math)

At 20K users, live-TTS volume = 500 chars × 6,000 sessions/day × 30 = **~90M chars/month**.

| Option | Monthly at 20K (90M chars) | Trait |
|---|---|---|
| **Fish Audio API** | 90M × $15/1M = **~$1,350 (₹1.28L)** | zero ops, #1 quality |
| **Sarvam Bulbul API** | 90M × ~$16.5/1M ≈ **~$1,485** | Indian voices, ₹-billed |
| **Self-host Fish Speech** | 1 GPU ~$1.5–4/hr × 730 ≈ **$1,100–2,900** + ops | #1 quality, multilingual, own the infra |
| **Self-host Kokoro** | tiny model, 90M × ~$0.70/1M-equiv ≈ **~$63** | English-centric, near-free |

- **Break-even:** Fish's own guidance puts self-host ahead of API at **~10M+ chars/month** — we cross that
  at **~2–3K users**, so self-host is *viable* early. But the API's zero-ops elasticity wins until you have
  GPU-ops capacity; **recommendation: start on Fish/Sarvam API, migrate to self-hosted Fish (or Kokoro for
  English-only) by ~50K users or when ops capacity exists** — the same "API's real product is elasticity"
  logic as the reference's LLM self-host call ([`../Support-Agent-L8-Deep-Dive/05-LLM-RUNTIME-AND-ROUTING.md`](../Support-Agent-L8-Deep-Dive/05-LLM-RUNTIME-AND-ROUTING.md) §7).
- **Self-host is also the price-risk hedge** (§7): it decouples cost from the API's per-char price, the one
  volatile line.

### 3.1 Self-hosting STT — the bigger lever, and why it does *not* pay yet

STT is now the largest single line, so it deserves the same math:

| | Monthly at 20K (~7,500 speech-hours) |
|---|---|
| **Sarvam API** (₹30/hr) | **~₹2.25 L/mo** (~$2,370) |
| **Deepgram API** ($0.0077/min) | ~₹2.6 L/mo |
| **Self-host** (Whisper/Parakeet-class, streaming) | peak ~**50 concurrent streams** ([07 §2](07-CAPACITY-AND-LATENCY-MATH.md)) ⇒ ~3–4 mid-tier GPUs ⇒ **~$2,200–2,900/mo cloud** + ops |

**Verdict: at 20K, self-hosted STT roughly *breaks even* and adds a GPU on-call rotation — so it does not
pay.** It becomes a clear win at **~50–100K users**, where API cost scales linearly and GPU utilization
finally saturates (and cheaper on dedicated hardware than cloud GPU rental). **Trigger to revisit: sustained
> ~25,000 speech-hours/month, or a residency mandate.** Until then, elasticity is worth more than the
margin — the same conclusion as TTS, reached independently.

## 4. Per active user / month, vs the ceiling

At ~30 sessions/active-user/month (paid tier; free tier is capped at 5, §1.1):

| Config | ₹/session | ₹/active-user/mo | vs ₹120 ceiling |
|---|---|---|---|
| **Default: Sarvam STT + Fish TTS (API)** | ~₹2.5 | **~₹75** | under |
| Deepgram STT + Fish TTS (global users) | ~₹3.2 | **~₹96** | under |
| Self-host TTS (≥ ~50K users) | ~₹1.9 | ~₹57 | 2× under |
| Self-host STT **and** TTS (≥ ~100K) | ~₹0.9 | ~₹27 | the long-run floor |

**Landing ~₹75–96/active-user/month at 20K on API providers**, under the **₹120** ceiling, with the
self-host path bending it back toward ~₹27 as volume grows. The ceiling is set by *business viability*
(§8), not by wishful engineering: at a ₹760/mo price point even ₹96 is ~87 % gross margin.

## 5. The free-tier runway (solo dev → first users)

Before any paid spend: **Gemini free tier** + **Sarvam ₹1,000 credits** + **Fish free tier** + **Supabase
free (500 MB)**. **But the runway is now much shorter than the pre-cloud-speech design claimed (R5):** with
STT and TTS both metered from the first session, free credits cover roughly **~100–300 early users**, not
1–2K. The relay VM (~₹3k/mo) is a fixed cost from day one.

**What is still genuinely free:** the noise bed, the state machine, the belief model and policy, and every
cached phrase — i.e. **presence costs nothing at any scale**, which is what keeps the *marginal* session
cheap. Reach-to-many now comes from the ~₹3/session unit cost and a session-capped free tier (§1.1), not
from a zero-cost stack.

## 6. Cost as a control loop (the ceiling is structural, not hoped)

Per [README §4](README.md) target 5 / INV5, the ceiling is enforced **in-path**, not reported after:
1. **Meter** — every session accrues a token/char tally (LLM tokens + live-TTS chars) into a per-session
   counter.
2. **Attribute** — tagged by user + session + line (voice/LLM), so the dominant line is always visible
   ([06 §8](06-EVALS-AND-TESTING.md) mix-drift alert).
3. **Cap (reservation):** a per-session **hard budget** (e.g. ₹4 — ~4× the mean, well under the monthly
   ceiling per session) — a runaway atomize-loop or a pathological TTS request **cannot exceed it**; it
   fails closed to the deterministic fallback ([05 §3](05-DETERMINISM-AND-LLM-CONSISTENCY.md)). Over-spend is
   *impossible*, not merely alerted.
4. **Alert** — mix-drift (paid-model share up, [04 §6](04-MODEL-ROUTER-AND-THE-CREW.md)) and ₹/session
   canary (+15 % auto-rollback, [06 §8](06-EVALS-AND-TESTING.md)) page before a trend becomes a bill.

## 7. Sensitivity (R4 — does the conclusion survive?)

| Shock | Effect | Survives? |
|---|---|---|
| **2× sessions** (power user, 60/mo) | ~₹150/user/mo | over the ceiling; ~5× under price (§8) — a **packaging** fix (fair-use cap), not an architecture risk |
| **FX $1 → ₹105** | +~10 % on the $-denominated lines (TTS/Deepgram); Sarvam is ₹-billed | yes — ~₹80/user/mo default |
| **Flash-Lite retires / LLM price 3×** | LLM ₹0.2 → ₹0.6/session | **negligible (+₹12/mo)** — the cost is *robust* to LLM price moves |
| **Speech minutes 2×** (a rambly user) | **STT ₹1.5 → ₹3.0/session** | **the most sensitive input** — mitigated by tight socket lifecycle ([03 §3](03-VOICE-LATENCY-PIPELINE.md)) and self-host at scale |
| **TTS API price 2×** | +~₹21/user/mo | mitigated by self-host (§3) |
| **Caching 50 % less effective** | live chars ~750 → TTS +₹0.35/session | still under ceiling |

The two genuinely sensitive inputs are now **billed speech minutes** and **TTS per-char price** — both
hedged by self-hosting, and the first also by socket discipline. The conclusion (**≤ ₹120, voice-dominant
at ~85–90 %, LLM-price-insensitive**) holds across every shock that matters. **The riskiest line is the one
a bug can move**: a socket left open turns ₹1.5 into ₹12 (scar §10.5), which is why it has a CI test rather
than a code review.

## 8. The business framing (R5 — cost-to-serve vs willingness-to-pay)

Cost-to-serve is **not the constraint** here; retention is ([09 §5](09-FAILURE-DR-AND-DEGRADATION.md)). A
comparable app (Doppel) prices at **$7.99/mo ≈ ₹760/mo**. Against a ~₹75–96/user/mo cost-to-serve,
that's a **~87–90 % gross margin**; a ₹300/mo tier is still ~70 %. The unit economics have
enormous headroom — the money question that actually matters is whether an ADHD user *keeps* the habit
past the novelty cliff, not whether a session is affordable. Building the cost model *proves* the product
is viable at any plausible price; the risk lives in [09](09-FAILURE-DR-AND-DEGRADATION.md), not here.

## 9. Proof-summary table

| Claim | Number | Where proven |
|---|---|---|
| Voice is the dominant cost line | **~85–90 %** of marginal (STT ~55 %, TTS ~25 %) | §1 |
| The LLM is a minor line | **~7 %** (~₹0.2/session) | §1 |
| Marginal cost/session | **~₹2.5–3.2** (API) · ~₹0.9 (fully self-hosted) | §1, §3, §3.1 |
| Per active user/month | **~₹75–96** at 30 sessions | §4 |
| Under the ceiling | **≤ ₹120** | §4 |
| Free tier is sustainable | **only if session-capped at 5/mo (~₹15)** | §1.1 |
| Cost is LLM-price-insensitive | 3× LLM price ⇒ +₹0.4/session | §7 |
| Self-host STT does **not** pay yet | break-even at 20K; win at ~50–100K | §3.1 |
| vs a viable price point | **~88 % gross margin @ ₹760/mo** | §8 |
| Speech-to-speech avoided | ₹47–260/session ⇒ **15–85×** | §2 |

## 10. Operator's scars

1. **"The LLM is the only cost."** The first model priced the LLM (₹0.15) and declared ₹0.20/session — then
   the voice-quality decision made TTS the ₹0.7 dominant line and the honest number tripled. Costing the
   *cheap* thing and forgetting the *expensive* one (voice) is the classic miss; the ceiling moved and we
   said so (R5).
2. **Priced STT+TTS for 25 minutes.** Same root cause as [07 §7.1](07-CAPACITY-AND-LATENCY-MATH.md) — costing
   the full session length instead of ~2.5 speech-min + ~500 novel chars overstated voice ~3–10×.
3. **The uncapped atomize loop.** A pathological input drove repeated re-atomize calls; without INV5 a single
   session could have run ₹40 of API spend. The per-session reservation (§6) makes that structurally
   impossible now.
4. **Forgot the cache in the estimate.** An early estimate billed *every* spoken char live, including the
   fixed fillers/wins — ~3× the real bill. Cached fixed phrases are ₹0 marginal (§2); the estimate has to
   model the cache or it lies high.
5. **The socket we left open.** Holding the STT stream for the whole session billed ~25 min instead of ~3 —
   **~8×**, turning ₹1.5 into ₹12/session. It survived staging because staging sessions were 90 seconds
   long. Socket lifecycle is now a **CI assertion** on billed-seconds ≈ speech-seconds
   ([03 §9.5](03-VOICE-LATENCY-PIPELINE.md)), because the cost lines a *bug* can move are more dangerous
   than the ones a *price change* can.
6. **Modelled a free tier that couldn't exist.** The plan assumed a generous free tier right up until STT
   became a paid line — at which point a free user cost ~₹90/mo and the funnel was upside-down. The
   session-capped free tier (§1.1) came from redoing the arithmetic *after* the quality decision, not before.
   Product packaging is downstream of the cost model, and it moves when the cost model moves.

## 11. Interview questions this file answers

- "What does a session cost, line by line?" (§1)
- "What's your dominant cost and why?" (§1, §2 — voice, not the LLM)
- "Self-host or API for the voice — show me the break-even." (§3)
- "Is your cost sensitive to model-price moves?" (§7 — no on LLM, yes on TTS, hedged by self-host)
- "How is the cost ceiling actually enforced?" (§6 — meter→attribute→cap-as-reservation→alert)
- "Can you afford this at your price point?" (§8 — ~96 % margin; retention is the real constraint, not cost)
