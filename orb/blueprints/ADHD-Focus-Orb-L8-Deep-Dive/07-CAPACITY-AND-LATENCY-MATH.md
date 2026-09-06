# 07 — Capacity & Latency Math

> **Purpose:** the numbers source of truth — the 20K demand model derived from scratch, the critical
> separation of **session-minutes vs speech-minutes vs LLM-calls**, peak concurrency by Little's law, the
> thin-backend sizing (why no queue/GPU/cluster), the honest latency audit (why there is ~no self-inflicted
> queueing at this rung), and the no-rewrite envelope to 200K. Every ₹ and ms elsewhere traces back here.

---

## 1. The 20K demand model (derived)

| Quantity | Derivation | Value |
|---|---|---|
| Registered users | given | 20,000 |
| DAU | 30 % (focus tools skew high-intent, low-retention — [09 §5](09-FAILURE-DR-AND-DEGRADATION.md)) | **6,000** |
| Sessions/day | ~1 focus session / DAU | **6,000** |
| Session length | one Pomodoro-style block | **25 min** |
| **User speech / session** | brain-dump + ~5 check-in replies + "done"s; VAD-gated (silent while working) | **~2.5 min** |
| Turns / session (total) | atomize + ~6 step transitions + ~5 check-ins + wrap | **~15** |
| **Real LLM calls / session** | ~65 % of turns are deterministic ([04 §4](04-MODEL-ROUTER-AND-THE-CREW.md)) | **~5** |
| Novel TTS chars / session | after fixed-phrase caching ([03 §4](03-VOICE-LATENCY-PIPELINE.md)) | **~500** |

**The three-way split is the whole point:** a 25-min session is **25 session-min** of noise bed (on-device,
₹0), but only **~2.5 speech-min** of STT and **~5 LLM calls** and **~500 live-TTS chars**. The expensive
things happen for a *fraction* of the session; the bed fills the rest for free. Costing the session as
"25 min of cloud voice" (the naive S2S model, [08 §3](08-COST-MODEL.md)) overstates the real load by ~10×.

## 2. Peak concurrency (Little's law)

Focus sessions cluster (post-coffee morning, after-lunch, evening). Take the peak hour = **20 %** of daily
sessions:

```
peak session starts  λ = 0.20 × 6,000 / 1 hr = 1,200/hr = 20/min = 0.33/s
concurrent sessions  L = λ × W  (Little's law) = 20/min × 25 min = 500
```

- **~500 concurrent sessions** at peak.
- **Total turn rate:** 500 × 15 / 25 min = 300/min = **~5 turns/s** (×3 correlated burst ⇒ ~15/s).
- **Real LLM-call rate:** 500 × 5 / 25 min = 100/min = **~1.7 calls/s** (×3 burst ⇒ ~5/s).
- **STT concurrency:** ~2.5 speech-min per 25-min session ⇒ ~10 % of sessions transcribing at once ≈
  **~50 concurrent STT streams** (on-device = per-phone; Sarvam/cloud only if used).

## 3. The thin backend (what it is, and why it's free at 20K)

The backend is **stateless orchestration + an LLM/TTS proxy + a cost meter + a durable mirror** — no queue,
no GPU, no cluster:

> **This changed with the audio-relay decision** ([03 §3](03-VOICE-LATENCY-PIPELINE.md)): the backend is no
> longer a stateless request proxy — it now terminates **WebSockets carrying live audio**. That moves it
> off a serverless free tier and onto a small always-on VM. It is still tiny; it is no longer free.

| Concern | Sizing at 20K | Fits on |
|---|---|---|
| **Concurrent WebSockets** | **~500** (one per live session, [§2](#2-peak-concurrency-littles-law)) | **one small VM** — a 4-vCPU box holds thousands of mostly-idle sockets |
| **Concurrent audio streams** (actually flowing) | ~10 % of sessions speaking ⇒ **~50** | same VM: 50 × 24 kbps ≈ **1.2 Mbps** in, trivial |
| Audio bandwidth | Opus ~20 kbps × ~3 min ≈ **450 KB/session** ⇒ ~2.7 GB/day | negligible; $0 egress on Cloudflare-fronted paths |
| Control/API requests | ~10/session × 6,000 = **60,000/day** (~3.3 req/s peak) | the same VM, or Workers |
| Durable hot state | a few KB/session; hot set **< 500 MB** | **Supabase Postgres free tier** |
| Cold logs / telemetry | ~30 MB/day (~11 GB/yr) | object storage (R2), ~99 % cold |
| Cached premium audio | fixed phrases + backchannels ([03 §2](03-VOICE-LATENCY-PIPELINE.md)) | shipped in-app / CDN — not per-request |
| LLM / STT / TTS | external APIs | provider capacity, not ours |

**Sizing the relay honestly:** the VM does **no** transcoding and **no** inference — it terminates a socket,
forwards Opus frames to the provider, and runs the belief/policy arithmetic (~50 FLOPs/event,
[13 §4](13-COGNITIVE-STATE-AND-POLICY.md)). That is an I/O-bound workload; **one 4-vCPU VM (~₹3,000/mo)
carries the whole 20K rung** with room to spare, at **~₹0.017/session**
([08 §1](08-COST-MODEL.md)). Two for redundancy.

**What we gave up:** the "entire backend runs free" claim from the pre-relay design. Honest replacement:
**infra is ~₹3–6k/mo at 20K — ~0.6 % of the ~₹5.4L/mo provider bill**, so it is a rounding error against
STT/TTS, just not literally zero (R5). Still: no Kafka, no K8s, no GPU, no vector DB
([12 §3](12-LLD-AND-SESSION-CONTRACT.md)).

## 4. The honest latency audit (why there's ~no queueing here)

The [03](03-VOICE-LATENCY-PIPELINE.md) budget assumed no self-inflicted queueing. At this rung that
assumption is *true*, and here's the proof rather than the hope:

- **We are ~3 orders of magnitude below any provider ceiling.** Hosted models serve millions of TPM; our
  **~1.7 LLM calls/s** (5/s burst) is a rounding error against that. Utilization ρ at our own layer ≈ 0.
- **Kingman (the queueing tail) vanishes at ρ≈0:** queue wait ≈ `(ρ/(1−ρ)) · (·)` → with ρ≈0, **≈ 0 ms
  self-inflicted queueing.** The p99 tail in [03 §1](03-VOICE-LATENCY-PIPELINE.md) is therefore **external**
  (provider TTFT p99 ~1.2 s) + **physical** (network RTT, endpointing) — *not* our infrastructure.
- **The contrast with big-scale deep-dives is the lesson:** the Support-Agent folder hits provider
  token ceilings and needs an admission cascade at 5M MAU. **We do not — and building that machinery now
  would be premature-scaling.** The *trigger* to add admission smoothing is named: sustained concurrent
  session-starts driving > ~50 atomize-calls/s, i.e. **~10× past 200K**. Until then, don't build it.
- **The one real burst:** correlated 9:00 starts hit the backend at ~10–30 req/s for a few seconds — well
  inside the free tier / one VM. If it ever bites, the fix is a few seconds of client-side jitter on
  session start, not a queue.

**The honest headline:** at 20K, **nothing is throughput-bound.** The engineering problems are the three
this folder spends its depth on — **on-device audio gaplessness ([02](02-ALWAYS-ON-AUDIO-ENGINE.md)), the
tail of the voice round-trip ([03](03-VOICE-LATENCY-PIPELINE.md)), and LLM behavioral consistency
([05](05-DETERMINISM-AND-LLM-CONSISTENCY.md))** — correctness and latency, not scale.

## 5. The no-rewrite envelope (200K, ×10)

| Quantity | 20K | 200K | What changes |
|---|---|---|---|
| DAU / sessions/day | 6K | 60K | — |
| Peak concurrent sessions | 500 | 5,000 | — |
| Real LLM calls/s (peak) | ~1.7 | ~17 (burst ~50) | still trivial for a hosted model |
| Backend req/day | 60K | 600K | **Workers free → paid ($5/mo)**; Supabase free → Pro ($25/mo) |
| Live-TTS volume | small | 10× | **the self-host-Fish/Kokoro crossover fires** ([08 §3](08-COST-MODEL.md)) |
| On-device work (bed/voice/state) | per-phone | per-phone | **unchanged — scales for free** |
| Architecture / boundaries | — | — | **unchanged — 0 rewrites** |

**20K → 200K is a tier swap + one read replica + turning on self-hosted TTS** — never a re-architecture. The
guarantee holds because the load-bearing boundary (on-device does the always-on 25 min; the cloud does ~5
short bursts) is fixed on day one, and the backend is stateless behind one interface so it scales
horizontally by adding instances. **Design for 200K (get the boundary right, it's free); pay for 20K (free
tiers); climb only when a named trigger fires** ([README §4](README.md) target 6).

## 6. Cost (cross-reference)

The ₹ build-up on this demand model — voice-dominant marginal **~₹2.5–3.2/session**, **~₹75–96/active-user/month**,
the **₹120** ceiling, the STT/TTS self-host crossovers, and the session-capped free tier — is [08-COST-MODEL.md](08-COST-MODEL.md).
Every quantity it consumes is a row above.

## 7. Operator's scars

1. **Costed the session as 25 minutes of voice.** The first model priced STT+TTS at the full 25-min session
   length and concluded the product was unaffordable (~₹15/session). The fix was seeing that speech is
   ~2.5 min and live-TTS ~500 chars — the bed owns the other 22 minutes for free (§1). The naive minute-count
   was a 10× overstatement.
2. **Almost built an admission queue.** An early design copied the big-scale deep-dive's token-budget
   admission cascade — machinery for a provider ceiling we sit 1,000× below. Deleting it (and naming the
   trigger to revisit, §4) was the right call; premature scale-machinery is cost and complexity for a
   problem you don't have.
3. **The 9:00 correlated start.** Load felt fine on average but a synthetic "everyone starts at 9:00" test
   briefly spiked backend req/s; the lesson wasn't "add a queue" but "add client jitter on start" — the
   cheapest possible fix for a burst that's inside free-tier capacity anyway.
4. **Supabase free tier and the log flood.** Writing full session transcripts to Postgres would have blown
   the 500 MB free tier in weeks; splitting **hot state (Postgres) from cold logs (R2)** kept the hot set
   tiny and the free tier viable (§3).

## 8. Interview questions this file answers

- "Size this system — concurrency, throughput, backend." (§1–§3)
- "Why is the naive 'cost = minutes of voice' wrong?" (§1, §7.1 — the session/speech/LLM split)
- "What's your peak concurrency and how did you get it?" (§2 — Little's law)
- "Where's the queueing tail?" (§4 — there isn't one at ρ≈0; the tail is provider + network)
- "Don't you need admission control / a token budget?" (§4 — no, and here's the trigger that would change that)
- "Prove 20K→200K needs no rewrite." (§5 — the fixed boundary + stateless backend)
