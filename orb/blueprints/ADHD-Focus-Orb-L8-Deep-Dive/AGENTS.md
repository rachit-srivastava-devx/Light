# AGENTS.md — Focus Orb (ADHD) L8 Deep-Dive

> Governed by [`../AUTHORING-GUIDE.md`](../AUTHORING-GUIDE.md) (the law: R1–R21, the depth bar) and
> [`../AGENTS.md`](../AGENTS.md) (the estate map — this folder is an **L8 system/engineering deep-dive**,
> a sibling of [`../Support-Agent-L8-Deep-Dive/`](../Support-Agent-L8-Deep-Dive/README.md) and
> [`../Voice-Agent-OS/`](../Voice-Agent-OS/README.md)). This file adds only what's specific to this
> instance; it never overrides either.

**Mission (from `README.md` §1):** a plane-by-plane, number-by-number deep-dive of **Phase 1** of an
always-on voice companion for ADHD — a single on-screen orb that runs a **body-doubling focus session**
(one spoken task → one atomic step at a time, audibly present the whole time). The center of gravity is
**three things done at L8 depth**: the **always-on audio invariant** (never a millisecond of silence),
the **voice-latency hot path**, and **driving cheap LLMs deterministically so they behave the same every
run** — with as much deterministic logic and as little LLM dependence as the problem allows.

**Canonical facts:** [`README.md`](README.md) §6 (prices, FX, headline economics) and §7 (the 20K demand
model) are the **single source of truth**. [07-CAPACITY-AND-LATENCY-MATH.md](07-CAPACITY-AND-LATENCY-MATH.md)
owns the capacity/latency derivations; [08-COST-MODEL.md](08-COST-MODEL.md) owns the money. Grep the
folder when a number changes (R7).

**Genre-specific notes:**
- **The audio bed is invariant #1, not a feature.** "We play a background sound" with no gap budget, no
  interruption matrix, and no native-thread argument is a *defect* here, not a stylistic nit — same
  standing as "we have a gate/eval" in the Support-Agent folder. Every claim about presence gets a number
  and a failure story ([02](02-ALWAYS-ON-AUDIO-ENGINE.md)).
- **The control path is deterministic; the LLM only fills language.** Any file that lets an LLM decide a
  state transition, an authority, or a route contradicts §4 target 5 — flag it, don't write it. Structural
  determinism (a state machine, schema-locked output, pinned versions) beats "the prompt tells it to."
- **This is a 20K-user rung, not a 50M one.** Do **not** import the estate's FMCG/Support ₹100/user +
  petabyte + Kafka/K8s framing. The honest story here is *the opposite*: thin cloud, on-device compute,
  nothing throughput-bound. Reach and cost come from pushing work to the phone, not from scaling servers.
- **Pure React Native — no custom native module.** Gaplessness is guaranteed by a *declaratively scheduled*
  Web Audio graph (`react-native-audio-api`) rendered on a library-owned C++ audio thread: JS declares,
  the audio thread executes, JS is never in the render loop. Don't reintroduce native modules into a file;
  they exist only as the last rung of the fallback ladder ([02 §3](02-ALWAYS-ON-AUDIO-ENGINE.md)).
- **Empathy is a deterministic system, not a TTS flag.** The session *state* selects the emotional register
  ([12 §5](12-LLD-AND-SESSION-CONTRACT.md)); a model never picks tone, and shame-adjacent registers are
  excluded by an allowed-set. A file that treats voice warmth as a provider checkbox misses the product's
  top-priority surface.
- **Evals are voice-to-voice.** Any new quality claim must be gradeable on audio-in→audio-out
  ([06 §1.1](06-EVALS-AND-TESTING.md)); a text-only metric is not evidence here.
- [10-CROSS-EXAMINATION.md](10-CROSS-EXAMINATION.md) and [11-L8-INTERVIEW-DRILL.md](11-L8-INTERVIEW-DRILL.md)
  are the consolidated question banks — a new file extends them, it doesn't answer questions only locally.

**Before editing:** read [`README.md`](README.md) in full — it is the single source of truth for this
folder's numbers and reading order, not this file.
