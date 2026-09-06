# Voice-UX guidelines — evidence and the runnable gate

> A document alone is worth almost nothing here. Every rule below that can be checked from an
> artefact this repo already produces is wired to `scripts/voice-ux-gate.mjs`, which fails loudly.
> Everything else is named as a manual checklist, most of which **already exists** in this repo
> (`docs/UX-HUMAN-REVIEW.md`, `ux-gate/gate.py`) and is cross-referenced rather than duplicated.

**Tag convention** (same as `adhd-conversation-design/SKILL.md`, house style for this repo):
`[strong]` replicated/large-corpus or official normative source · `[moderate]` a single credible
primary source (vendor docs, one peer-reviewed study) not independently cross-verified elsewhere ·
`[thin]` widely repeated, popular, but I could not trace it to a primary source, or the primary
source turned out not to say it. **A claim with no traceable source is dropped, not softened into
`[thin]`** — two specific numbers were dropped below for exactly this reason; see §8.

Every external number here was fetched from the cited URL in this session (2026-08-27/28), not
recalled from training data. Where a vendor's own docs had no hard number, that is stated plainly
rather than filled in from a comparison blog.

---

## 0. What already exists in this repo — read this before the gate

This is the second voice-UX research pass in this repo, not the first, and none of it should be
re-derived:

| Existing artefact | What it already covers |
|---|---|
| `ux-gate/gate.py` | Executable gate over **one e2e drive's `trace.jsonl`**: every turn answered, first-audio latency on the **user's clock**, no verbatim-repeat replies, one-breath length, screenshots differ, failure surfaced once, presence-cover-vs-real-content. Cites: 250ms owner target, LiveKit's turn-detector curve, Stivers et al. 2009, Alexa's one-breath test — the same four sources this document independently re-verified in §§2–3. |
| `audio-gate/` | Gate over **rendered audio** (mic/speaker capture, not logs): presence/content latency gap, continuity. Its own README still states plainly that *"actual rendered TTS yield at the speaker within 100 ms is still not measured"* — that half of barge-in timing remains NOT-MECHANISABLE below (§7) rather than reinvented. What changed on 2026-08-28: the **relay-side** half is now measured (`npm run e2e:bargein`; G5 below), so §7 no longer covers the whole property, only the last leg of it. |
| `docs/UX-HUMAN-REVIEW.md` | The 12-item human listening checklist (H1–H12) — barge-in (H6), misrecognition repair (H8), distress handling (H9), failure honesty (H11), etc. This **is** the not-mechanisable bucket for most of what Step 1 researched below; §7 maps each researched topic onto it instead of writing a second checklist. |
| `evals/load_direction/README.md` + `conversation_guard.py`'s `shifts_mental_load` | The confirmation/load-direction control this product actually ships, already labelled honestly: `mitigates`, never `kills (structural)` — 8 known escape phrasings, unbounded phrasing space. Confirmation strategy (§6) points here rather than building a second, weaker version. |
| `scripts/provider-report.mjs` | The NDJSON reader convention this gate's reader function copies: glob `dev-logs/*.ndjson`, parse per line inside try/catch, `--hours`/`--json` flags, money in integer paise, exit non-zero on an empty read. |

`scripts/voice-ux-gate.mjs`'s job is what's left: **census-scale** checks across the full
`dev-logs/` corpus (tens of thousands of turns across weeks) rather than one drive's few turns —
things ux-gate/audio-gate structurally cannot see because they only ever look at one run.

---

## 1. Barge-in / interruption handling

| Claim | Tag | Source |
|---|---|---|
| Barge-in requires three chained steps under one budget: detect the interruption, stop/duck the agent's audio, and cancel whatever generation (LLM/TTS, possibly a tool call) was in flight — treated by practitioners as one policy layer, not a VAD threshold alone. | `[moderate]` | [Hamming AI — Voice Agent Interruption Handling runbook](https://hamming.ai/resources/voice-agent-interruption-handling-runbook) (practitioner guidance, "10M+ min protected / 10K+ agents" is their own stated evidence base, not a controlled study) |
| A real interruption must be distinguished from a **backchannel** ("mm-hmm", "right") before the agent yields — the same acoustic event (user sound while agent is speaking) needs different handling depending on which it is. | `[moderate]` | Hamming AI (above); converges with this repo's own `adhd-conversation-design/SKILL.md` Rule 19 (backchannels "must never be counted as an answer") |
| Barge-in sensitivity is not uniform through a call — production systems apply different policies at different stages (greeting vs. data collection vs. confirmation/legal). | `[moderate]` | Hamming AI (above) |
| Gemini Live API ships "Barge-in" as a named feature: "Users can interrupt the model at any time." | `[moderate]` | [Google — Gemini Live API overview](https://ai.google.dev/gemini-api/docs/live) (official, but no numeric budget published on this page) |
| This product's own barge-in yield budget: `barge_in_yield_ms: 100`, `barge_in_qualification_ms: 200` — self-declared, not externally sourced. | n/a (in-repo) | `dev-logs/relay-rs.ndjson`, `event: "latency.budgets"`; matches `AGENTS.md` "barge-in yield ≤100ms" |

**A specific number was dropped, not softened.** A first-pass web search returned "barge-in must
complete in <150ms total, 30–50ms per component; a guard adds 200ms but cuts false-barge-in 60–80%,"
attributed to the Hamming runbook above. Fetching that exact page directly showed **none of those
figures are on it** — its only numeric latency reference is Amazon Nova Sonic's endpointing
sensitivity (1.5/1.75/2.0s), unrelated to barge-in. That number is excluded entirely, not
downgraded to `[thin]`, because I could not trace it to the source the search summary attributed it
to, and no other source corroborated it. **No vendor in this research published an external,
independently-checkable "barge-in completes in Xms" benchmark** the way LiveKit publishes an
endpointing curve (§2) — this product's 100ms is a self-imposed engineering target, not a
researched external bar, and should be described that way, not as "industry standard."

## 2. Endpointing and turn-taking

| Claim | Tag | Source |
|---|---|---|
| Across 10 typologically diverse languages, response gaps in question-answer sequences are unimodally distributed with a **mode of 0–200ms per language** (overall cross-language mode ≈0ms) and **means of 7–468ms**; all languages show avoidance of overlap and minimization of silence. | `[strong]` | Stivers et al., **"Universals and cultural variation in turn-taking in conversation," PNAS 106(26):10587–10592 (2009)**, DOI [10.1073/pnas.0903616106](https://www.pnas.org/doi/10.1073/pnas.0903616106) — large cross-linguistic corpus, peer-reviewed, independently cited (below) |
| "The modal gap between turns is only around 200ms, and it works with equal efficiency without visual contact." | `[strong]` (corroborating citation) | Levinson & Torreira (2016), cited via search of the Stivers finding — independent paper restating the same ~200ms figure |
| This ~200ms figure has been challenged as **less mechanistically informative than assumed** for predicting real turn timing — inter-speaker gaps alone carry limited information about how turns are actually planned. | `[thin→contested]`, noted not verified in full | "Overrated gaps: Inter-speaker gaps provide limited information about the timing of turns in conversation," ScienceDirect — title/abstract only, not fetched in full; included so the 200ms number is not read as more settled than it is |
| LiveKit's open, reproducible turn-detector benchmark (own dataset, 14 languages): at a **5% false-cutoff rate**, best latency is **543ms**; at **10%**, **295ms**. Full comparison table below. | `[strong]` | [LiveKit — `eot-bench` README](https://github.com/livekit/eot-bench) (open benchmark + open dataset, primary, directly fetched) |
| A separate LiveKit multilingual model update (`v0.4.1-intl` vs `v0.3.0-intl`) cut false-positive interruptions **39.23%** at fixed true-positive rate, with no latency increase. | `[moderate]` | [LiveKit blog — "Improved end-of-turn model cuts Voice AI interruptions 39%"](https://livekit.com/blog/improved-end-of-turn-model-cuts-voice-ai-interruptions-39/) (single vendor's own before/after, not independently reproduced) |
| This product's own ADR measured, on 3 real speech fixtures × 13 thresholds (n=39 gate runs): false-cutoff rate is 100% at a 60ms post-endpoint-close threshold, 67% at 100–200ms, **0% at 250ms and up (flat through 1200ms)**. Explicitly caveated by its own author as n=3, "33%-step resolution," not a substitute for a real N=200 gate. | `[thin]` (n=3, self-caveated) but real, in-repo | `docs/adr/0013-vad-and-endpointing-architecture.md` |

**LiveKit eot-bench comparison table** (English operating points, fetched directly from the README):

| Model | False cutoffs @ 300ms budget | False cutoffs @ 600ms budget | Latency @ 5% cutoff | Latency @ 10% cutoff |
|---|---|---|---|---|
| LiveKit Turn Detector v1 | **9.9%** | **4.5%** | **543ms** | **295ms** |
| Deepgram Flux | 12.9% | 9.9% | 1151ms | 548ms |
| ultraVAD | 27.7% | 11.9% | 899ms | 663ms |
| LiveKit Turn Detector v1-mini | 27.8% | 12.1% | 1070ms | 698ms |
| Pipecat SmartTurn v3.2 | 35.2% | 14.8% | 1051ms | 739ms |
| AssemblyAI | 49.4% | 14.6% | 1049ms | 713ms |
| Soniox | – | 5.5% | 647ms | 512ms |

**FINDING — this product's own text-domain endpointer is currently unreachable in production**,
already discovered and documented by the concurrent VAD track, cited here because it directly
determines what "how long to wait" means for this app today: `apps/mobile/src/App.tsx:355`
hardcodes `pauseMs = 0` when calling `voiceLoop.handleTranscript`. Because
`decideEndpoint`'s `pause_ms < eagerPauseMs` (0 < 60) is always true, `SemanticEndpointer.ts`'s
entire text-domain completion path — both tiers — never fires; the only real endpointing authority
today is `VADGate.ts`'s audio-level hangover (`VAD_POST_ENDPOINT_CLOSE_MS = 800ms`). Full analysis:
`docs/adr/0013-vad-and-endpointing-architecture.md` §"Track J's subscription contract". Not fixed
here — `apps/mobile/src/` is out of this change's scope, and the ADR already owns the finding; it is
cross-referenced because it is load-bearing for anyone reading "endpointing" rules against this repo.

## 3. Latency budgets

| Claim | Tag | Source |
|---|---|---|
| Twilio's own published component targets (target / upper limit): STT 350/500ms · LLM time-to-first-token 375/750ms · TTS time-to-first-byte 100/250ms · platform turn gap 885/1100ms · **mouth-to-ear turn gap 1115/1400ms**. Explicitly labelled "starting benchmarks, not best-in-class." | `[moderate]` | [Twilio — "Core Latency in AI Voice Agents"](https://www.twilio.com/en-us/blog/developers/best-practices/guide-core-latency-ai-voice-agents) (official vendor blog) |
| Twilio's own managed ConversationRelay measures **<500ms p50, <725ms p95** for its half of the call only (excludes the customer's model/tool-call server). | `[moderate]` | same, official but explicitly partial-scope |
| Deepgram's own TTS latency model: **~600ms baseline + ~40ms per 100 characters**; a worked example shows 339ms SSL handshake + 277ms time-to-first-byte = 616ms, 745ms total. Deepgram states these are illustrative, not guarantees. | `[moderate]` | [Deepgram docs — Text-to-Speech Latency](https://developers.deepgram.com/docs/text-to-speech-latency) (official docs, directly fetched) |
| Cartesia states "sub-90ms latency" for Sonic TTS on its own product page (single self-reported figure; no open benchmark methodology disclosed on that page, unlike LiveKit's). | `[moderate]` | [Cartesia — Sonic](https://www.cartesia.ai/sonic) (official page, directly fetched) |
| **OpenAI and Google publish no hard latency SLA/target numbers in their own official docs.** Both were fetched directly (`developers.openai.com/api/docs/guides/realtime`, `ai.google.dev/gemini-api/docs/live`) and neither states a time-to-first-audio or turn-latency figure — OpenAI's only timing guidance is qualitative ("lower delay settings produce earlier partial text… test with your real audio conditions"); Google's Live API page names "low-latency" as a property without a number. | n/a — **absence stated explicitly** | see above, both fetched directly |
| Numbers like "GPT-4o Realtime median 300–600ms," "190ms end-to-end," "Gemini 320ms p50/780ms p95" appear across several comparison/marketing blogs (`inworld.ai`, `autointerviewai.com`, `kunalganglani.com`, `cekura.ai`, and similar). **None trace to a primary vendor source** — excluded from this document rather than reported as vendor claims. A real, primary, if informal, data point on production variance: a Google-hosted developer forum thread reports users seeing Live API latency **spike to 7–15 seconds** under some conditions. | `[thin]` / excluded | [Google AI Developers Forum — "Live API latency spikes"](https://discuss.ai.google.dev/t/live-api-latency-spikes/106814) (real but anecdotal, not independently reproduced here) |
| Human "natural conversation" reference gap: reuse Stivers et al. 2009 (§2) — modal 0–200ms — as the perceptual anchor for "this doesn't feel like a computer," rather than a separately sourced number. | `[strong]` (same citation as §2) | as above |
| This product's own configured/measured numbers — see §8 (Step 4 run), not researched externals. | n/a (in-repo) | `dev-logs/relay-rs.ndjson` `latency.budgets`/`latency.content_first_chunk_server`; `e2e-human-simulator/runs/**` |

## 4. Dead air

This repo's own `adhd-conversation-design/SKILL.md` §4 (Rules 15–19) is the primary source here and
is not re-derived: delay aversion (Sonuga-Barke) `[strong]`, Nielsen's visibility-of-system-status
`[strong]`, "treat exceeding a silence budget as a **failing state**, not a degraded one" (Rule 17) —
this is the exact rule §7/G1 below mechanises. New material from this pass:

| Claim | Tag | Source |
|---|---|---|
| Filler/cover speech ("let me check on that") is a real, commonly-used mitigation for perceived wait, used the instant a tool call starts, without waiting for the result — but it risks overlapping the user's own speech and raising interruption rates if not tuned. | `[moderate]` | converges across [Pipecat optimization guide](https://futureagi.com/blog/how-to-optimize-pipecat-latency-2026/), [Deepgram voice agent architecture guide](https://deepgram.com/learn/voice-agent-architecture-stt-llm-tts-pipeline-design) — practitioner consensus, no controlled study found |
| This product already runs exactly this pattern under its own name: `presence.cover_required` (relay-rs), self-labelled `measurement_scope: "control_frame_not_rendered_audio"` — i.e. it honestly reports that it measures the control signal, not confirmed audible cover. | n/a (in-repo) | `dev-logs/relay-rs.ndjson` |

## 5. ASR error recovery

| Claim | Tag | Source |
|---|---|---|
| Commercial dialog systems (Nuance) use a **three-tier confidence model**: below a low threshold, reject/reprompt; between low and high, elicit an explicit confirmation turn; above the high threshold, accept without confirming. | `[moderate]` | [Nuance Mix docs — Understanding confidence evaluation](https://docs.nuance.com/mix/integration/speech-suite/understand-confidence/) (official vendor docs; long-established commercial practice, not an academic/controlled study) |
| Never repeat the previous reply verbatim on a repair turn — already this repo's own precedent (`ux-gate/gate.py` U3, `docs/UX-HUMAN-REVIEW.md` H8: "in different words than its last reply"). | n/a (in-repo, reused) | as cited |

**FINDING — ASR confidence is not just unmeasured in telemetry, it is structurally absent from the
wire protocol**, even though the provider supplies it:
- `backend/voice-provider-sidecar/src/stt/deepgram.ts:9` — the `DeepgramResponse` TS interface types
  only `alternatives?: Array<{ transcript?: string }>`; Deepgram's real `/v1/listen` response also
  carries a per-alternative `confidence` field, which this interface does not declare or extract.
- `backend/relay-rs/src/protocol.rs:49-54` — the `ServerFrame::Transcript` wire variant carries only
  `tenant_id, session_id, text, is_final`. There is no field to carry a confidence score even if the
  sidecar extracted one.
- `backend/relay-rs/src/main.rs:94-115` (`log_stt_transcripts`) — the `stt.transcript` dev-log event
  mirrors the same fields; confirmed empirically, **0 occurrences of `"confidence"` anywhere in
  `dev-logs/*.ndjson`** (grep census, all files).
- Consequence: the Nuance-style tiered recovery this section researched cannot be built today
  without first threading a confidence value through both files above. Not fixed here (backend
  files, out of this change's scope) — recorded as a finding, not attempted.

## 6. Confirmation strategy

| Claim | Tag | Source |
|---|---|---|
| Same three-tier Nuance model as §5 governs when to confirm at all. | `[moderate]` | as §5 |
| Users prefer **explicit** confirmation for high-"purposiveness" (consequential) tasks and prefer system efficiency / **implicit** confirmation for low-purposiveness tasks; over-confirming low-stakes turns reads as friction. | `[moderate]` | Springer, *International Journal of Social Robotics*, "Exploring Confirmation Strategies for Voice Interaction in Multi-Tasking Scenario" (2025), DOI [10.1007/s12369-025-01302-w](https://link.springer.com/article/10.1007/s12369-025-01302-w) — **paywalled; abstract/search-index level only, full text not read.** Single study, not independently replicated. Cited at reduced confidence for exactly that reason, not presented as settled. |

**This product's confirmation-strategy control already exists and is already honestly labelled** —
not re-derived here. `evals/load_direction/README.md` + `backend/relay-py/src/orb_relay/proxy/conversation_guard.py`'s
`shifts_mental_load` is precisely "never make the user confirm/decide what the agent could just
propose" (this repo's sharper, ADHD-specific version of "don't over-confirm"), and its own README
states the honest limit: **8 known escape phrasings found, unbounded phrasing space, `mitigates`
never `kills (structural)`.** A second, independent gate asserting the same property would either
duplicate that work or (worse) imply a precision the original authors explicitly disclaim. §7 points
to it rather than rebuilding it.

## 7. Accessibility (WCAG and adjacent)

| Claim | Tag | Source |
|---|---|---|
| **SC 1.4.2 Audio Control (Level A, normative):** "If any audio on a web page plays automatically for more than 3 seconds, either a mechanism is available to pause or stop the audio, or a mechanism is available to control audio volume independently from the overall system volume level." | `[strong]` | [W3C WAI — Understanding SC 1.4.2](https://www.w3.org/WAI/WCAG21/Understanding/audio-control.html) (official, normative) |
| **SC 2.2.1 Timing Adjustable (Level A, normative):** for any time limit set by the content, the user must be able to turn it off, extend it ≥10×, or get a 20-second warning with a repeatable extend action (exceptions for real-time/essential activities). **Does not itself mention voice or spoken response deadlines** — the application to an endpointing/silence-wait timeout below is this document's inference, not WCAG's own text. | `[strong]` (criterion itself); analogy to voice is mine, not sourced | [W3C WAI — Understanding SC 2.2.1](https://www.w3.org/WAI/WCAG21/Understanding/timing-adjustable.html) |
| W3C's cognitive-accessibility voice guidance: pauses between phrases "to allow processing time"; extra time should be a **user setting**, for both speech rate and input time; **users of AAC/speech-to-speech devices may need significantly more time before the system times out**; users need explicit feedback that their input was received. | `[strong]` (official W3C Working Group Note) | [W3C — Cognitive Accessibility Research Modules: Voice Systems and Conversational Interfaces](https://www.w3.org/TR/coga-voice/) |
| W3C APA's "voice agent user requirements" page: error recovery should be graceful (no low-level error text spoken), users need to understand system state during processing, input flexibility. | `[thin]` — explicitly a draft wiki page, self-described as exploratory, not a finished Recommendation | [W3C APA — Voice agent user requirements](https://www.w3.org/WAI/APA/wiki/Voice_agent_user_requirements) |

**Applied to this product, not mechanised (needs a manual check, listed in §8's not-mechanisable
bucket):** the always-on presence bed (INV1) plays automatically indefinitely, well past the
3-second SC 1.4.2 threshold. The product's pause contract exists by design
(`AGENTS.md`: "the explicit pause/session-end contract"), which is the kind of mechanism SC 1.4.2
asks for in spirit — but whether that pause is **independent of system volume** and **discoverable**
(not just theoretically present in the state machine) is a UI/audio question this gate cannot answer
from `dev-logs/`. Listed as a manual item, not claimed either way.

---

## 8. Mechanisable vs. not-mechanisable — the actual split

**7 rules mechanised** in `scripts/voice-ux-gate.mjs` (5 gated, pass/fail, drive the exit code; 2 are
census/observational — printed with full denominators but not gated, because no defensible
pass/fail threshold exists for a *rate*, only for the underlying *event*; see rationale per rule).
**Everything else — the majority of what was researched** — needs a human ear/eye and is not
reinvented: it maps onto the existing `docs/UX-HUMAN-REVIEW.md` (H1–H12) and `ux-gate/gate.py`'s
`auto=False` items, both already in this repo.

### (a) MECHANISABLE — wired into `scripts/voice-ux-gate.mjs`

| # | Rule | Research basis | Gated? | Reads |
|---|---|---|---|---|
| G1 | Zero dead-air / silence-budget violations | `adhd-conversation-design` Rule 17 (§4) — a wait-budget breach is a **failing state** | Yes, threshold 0 | `conversation.wait_failed` |
| G2 | No consecutive verbatim-identical **generative** spoken replies, per session, gap-bounded to ≤30min and restricted to `source ∈ {model, model_repaired}` | Alexa/Nuance repair convention, this repo's own H8/U3 precedent (§5) | Yes, threshold 0 | `conversation.response.response_text`, grouped by `session_id` |
| G3 | Spoken replies pass the one-breath test (≤240 chars) | Amazon Alexa's official one-breath test (§3/`ux-gate/gate.py`'s existing constant, reused verbatim for consistency) | Yes, threshold 240 chars | `conversation.response.response_text` |
| G4 | Every accepted request resolves to a terminal outcome (no silent black hole) | H11 "failure honesty" / COGA "graceful recovery" (§7) — census version of `ux-gate/gate.py`'s U1 | Yes, threshold ≥99% accounted-for | `conversation.request` vs. `{response, reservation_exceeded, gateway_error, wait_failed}` counts |
| G5 | The relay yields the floor on barge-in within its own budget | Demonstrates the "measured nothing must fail" rule explicitly (see below) — and, since 2026-08-28, the sharper rule that a metric which *cannot* fail is worse than no metric | Yes | `latency.barge_in_yield.yield_latency_ms` vs. `latency.budgets.barge_in_yield_ms` |
| O1 | Degraded-response rate | AGENTS.md quality framing; no external "acceptable %" source exists | No — observational | `conversation.response.degraded` |
| O2 | Reservation-exceeded-with-no-reply-text rate | H11/COGA "graceful failure" (§7); confirmed at code level these map to HTTP 402 with no envelope (`backend/relay-py/src/orb_relay/app.py:423,442,613,679`) | No — observational | `conversation.reservation_exceeded` vs. `conversation.response` |

**Why G-rules are pass/fail and O-rules are not:** G1–G5 each have a threshold traceable to either
a cited rule ("this must never happen") or the product's own self-declared configuration value —
never a percentage I invented. O1/O2 are real, denominator-transparent, full-corpus measurements,
but no source in §§1–7 states what degraded-rate or reservation-exceeded-rate is *acceptable* — only
that the *event itself* (dead air, a black-hole turn) must not occur. Inventing a threshold
("≤5% degraded is fine") to force a PASS/FAIL would be exactly the kind of proxy dressed as a
verdict this repo's own memory calls out — the honest move is to report the number, denominator and
all, and say plainly that no external bar exists.

G5 deliberately narrows what it claims: it checks **how fast the relay yields the floor** —
retires the speaking generation, suppresses its remaining audio, and acknowledges — never "the
user's audio actually stopped." That stronger claim is `audio-gate/`'s job, and its own README
still says it isn't wired yet (§0). Conflating the two would be a proxy standing in for the
property.

**G5 was itself a proxy standing in for the property until 2026-08-28, and the corpus proves it.**
It graded `frame.processed.latency_ms` for barge-in frames: the time the relay's read loop spent
handling the frame *once it had read it*. The pre-fix `serve_connection` could not read a
`barge_in` frame until the entire utterance had been synthesised and flushed — so that number was
~0.1ms precisely *because* barge-in was broken, and G5 reported **PASS at p95 0.103ms against a
100ms budget over 68 samples** for a relay independently measured at p50 2176ms / p95 4046ms.
A true measurement of the wrong quantity. G5 now grades `latency.barge_in_yield.yield_latency_ms`,
emitted only when the relay has actually retired the generation, and deliberately has **no
fallback** to the old field: no rows, no verdict.

**G2 was built, run, found to be measuring the wrong thing, and fixed twice before this document
was finalized — recorded here because the failure mode is itself instructive.** First run: grouping
`conversation.response` by raw `session_id` and comparing adjacent replies gave a 68% "repeat rate"
(2933/4291). That number is not real: `session_id` values like `"s1"` are reused as a lazy pytest
fixture default across **hundreds** of unrelated test invocations — one such session carried 126
response rows with gaps up to 866,169 seconds (~10 days) between "consecutive" turns. A time-gap
bound (≤30 minutes — generous enough for a real check-in-later gap, tight enough to exclude
cross-run collisions) was added. The rate barely moved (2823/4165) — a full pytest run exercises
hundreds of unrelated cases inside one 30-minute window. Checking *what* was repeating: 93% of the
gap-bounded repeats carried `source: "model"` with text like *"A short reply here."* / *"Memory
adapter response."* — literal fixture strings from pytest's mocked gateway clients (e.g.
`ScriptGateway` in `backend/relay-py/tests/test_load_shift_gets_one_repair.py`), not a real
provider repeating itself. The check now also excludes explicitly-deterministic/templated sources
(`safety_fallback`, `wait_companion`, `format_fallback`, `deterministic_control` — repeating a
fixed canned line by design is not the H8/U3 defect) and restricts to `source ∈ {model,
model_repaired}`. Even after both fixes, the measured rate on the full corpus is still ~72%
(2767/3864 at last run) — **this is reported honestly as still-likely-dominated by mocked-gateway
fixture contamination within `source: "model"`, not resolved**, because separating that fully would
require joining to `gateway_client.completion.is_fake_adapter` by session/time proximity, which is
not built. See finding #6 in §9. The lesson generalizes past this one check: **`source: "model"` in
this corpus means "the code path a model call would take was exercised," not "a real provider
answered"** — the same caveat `scripts/provider-report.mjs` already handles for `gateway_client.completion`
via `is_fake_adapter`, but `conversation.response` carries no equivalent field.

### (b) NOT MECHANISABLE — the manual checklist (already exists; extended only where a gap was found)

No new checklist was written. `docs/UX-HUMAN-REVIEW.md`'s H1–H12 already cover barge-in feel (H6),
misrecognition repair (H8), distress handling (H9), not-a-task conversation (H10), failure honesty
(H11). Mapping this pass's research onto it:

| Researched topic | Covered by | Why it stays manual |
|---|---|---|
| Barge-in **yield timing at the speaker** (does audio actually stop ≤100ms *in the room*) | `docs/UX-HUMAN-REVIEW.md` H6; `audio-gate/`'s stated gap | **Scope narrowed 2026-08-28.** The relay-side leg is no longer in this bucket: `npm run e2e:bargein` drives the real relay over a real socket with real Fish TTS and measures it (n=10, p50 1.2ms / p95 1.9ms vs the 100ms budget, 100% of the utterance's bytes suppressed vs an uninterrupted control), and G5 grades the relay's own `latency.barge_in_yield` rows (n=30, p95 1.9ms). What remains not-mechanisable is the last leg: client playback buffer and speaker output. `audio-gate/README.md` still says outright that "actual rendered TTS yield at the speaker within 100 ms is still not measured", and the e2e harness's barge-in journey trace still tags every screenshot `"ui_confirmed": false` (`e2e-human-simulator/runs/orchestrated/20260827T220527Z/trace.jsonl`) — a human still has to listen. A relay that has stopped sending is necessary for the property, not sufficient for it. |
| Does the wait *feel* like a person thinking (vs. broken) | H4 | Subjective, audio texture |
| Voice quality / warmth | H7 | Subjective, audio texture |
| Distress handling register | H9 | Requires judging *tone*, not just that a safety control fired (`conversation.safety_veto` proves the control activated, not that the resulting reply sounded right) |
| Confirmation strategy "feels natural, not naggy" | Not on H1–H12 explicitly — **gap identified, not filled**: recommend adding an H13 to `docs/UX-HUMAN-REVIEW.md` in a future change (not made here; that file is not one of this change's three owned files) | The mechanised half is `shifts_mental_load` (§6); whether an *unshifted* reply still over-confirms in a way that annoys is a tone judgement |
| Bed pause discoverability / independent-of-system-volume (WCAG 1.4.2 in spirit) | Not on H1–H12 — **gap identified, not filled**, same reasoning as above | Requires operating the real UI |
| Endpointing "feels natural" (as opposed to the false-cutoff rate, which is mechanised in ADR 0013's own n=3 run) | H4, H8 | Whether an 800ms hangover *feels* like a thoughtful pause or a stall is a perception judgement no log field encodes |

Two real gaps were found in the existing 12-item list (confirmation-tone, bed-pause-discoverability)
and are named above rather than silently left out — `docs/UX-HUMAN-REVIEW.md` is not one of this
change's three owned files, so it was not edited; this is a finding for whoever next touches that
file, not a change made here.

---

## 9. Other findings (file:line), not fixed here

1. **`dev-logs/` is polluted by adversarial-fuzz-generated files with unsanitized names.**
   `backend/relay-py/src/orb_relay/app.py:803-813` — the dev-only `/dev/log` ingest route accepts a
   client-supplied `service` and `event` string and forwards them verbatim to `dev_log()`.
   `backend/relay-py/src/orb_relay/observability/devlog.py:50` builds the on-disk filename from
   `_safe_service_name(service)`; `devlog.py:57-59`'s sanitizer keeps anything `str.isalnum()`
   accepts, which is Unicode-aware (accented Latin, etc.), not ASCII-only — so adversarial input
   (almost certainly from `adversarial/api-fuzz` or `adversarial/redteam`, both of which exist in
   this tree) survives filtering and lands as real files. Confirmed by hex-dumping
   `dev-logs/K.ndjson`, `dev-logs/CMëÑ.ndjson`, `dev-logs/RG.ndjson`, etc. — valid JSON, `service`
   full of `\uXXXX`-escaped garbage, filename = whatever survived the alnum filter. Roughly two
   dozen such files sit in the same directory this gate (and `provider-report.mjs`) glob over.
   Harmless to correctness today (garbage `event` values don't collide with real event names), but
   real hygiene/observability debt.
2. **`dev-logs/` has no provenance discriminator.** The same `dev_log()` call
   (`devlog.py:38`) is used by manual dev sessions, the full pytest suite, `adversarial/` fuzzing,
   and `e2e-human-simulator/`, with nothing distinguishing them beyond guessable naming conventions
   in `session_id`/`tenant_id` (e.g. `"wait-tenant"`, `"h2-bounded-redteam"`, `"teach-exhausted"`).
   Concretely: `dev-logs/relay-py.ndjson` shows `cost.reservation_exceeded` firing with
   `cost_paise: 13405076370` (₹134M) on one row — obviously an adversarial/edge-case test value, not
   real spend. **This gate's own O1/O2 numbers in §8 inherit this limitation** — a spike can mean
   "a redteam suite ran," not "real users are failing." Recorded as a finding (add a `source` field
   to `dev_log()`'s signature), not fixed — out of this change's scope.
3. **ASR confidence is structurally absent from the wire protocol**, detailed in full in §5:
   `backend/voice-provider-sidecar/src/stt/deepgram.ts:9`,
   `backend/relay-rs/src/protocol.rs:49-54`, `backend/relay-rs/src/main.rs:94-115`.
4. **The mobile client's HTTP-402 fallback path appears never to have fired in the recorded
   corpus.** `apps/mobile/src/runtime/ConversationPort.ts:95-101` and `:119-126`: on any non-OK
   response (including 402) or thrown error, the client logs and calls `fallback.respond(input)` —
   so a real mobile session should not go fully silent on reservation-exceeded the way the raw
   relay response does. But `dev-logs/mobile.ndjson` shows **0** occurrences of
   `conversation_port.fell_back` or `conversation_port.http_error` anywhere, despite the relay
   logging thousands of `cost.reservation_exceeded`/`conversation.reservation_exceeded` events
   (finding #2's caveat applies directly: most of those are near-certainly pytest/redteam traffic
   hitting the relay's HTTP API directly, never touching `ConversationPort.ts` at all). Telemetry
   alone cannot say whether the fallback path works, only that it has not been observed firing —
   flagged, not resolved.
5. **`apps/mobile/src/App.tsx:355`'s hardcoded `pauseMs = 0`** — already found and documented by
   `docs/adr/0013-vad-and-endpointing-architecture.md`; cross-referenced in §2 because it directly
   determines what "endpointing wait time" means for this app today, not claimed as a new finding.
6. **`conversation.response`'s `source` field cannot be trusted to mean "a real provider answered."**
   Full detail and how it was found: §8a's note under G2. `source: "model"` covers both real
   provider calls and pytest's mocked-gateway test doubles (e.g. `ScriptGateway` in
   `backend/relay-py/tests/test_load_shift_gets_one_repair.py`) returning literal fixture strings —
   there is no field on `conversation.response` equivalent to `gateway_client.completion`'s
   `is_fake_adapter` (which `scripts/provider-report.mjs` already relies on for exactly this
   reason). Consequence: `scripts/voice-ux-gate.mjs`'s G2 (verbatim-repeat) and O1 (degraded-rate)
   both inherit this ambiguity on their `source: "model"`-scoped portion — reported plainly in the
   gate's own output rather than hidden, but not resolved. A real fix needs either a
   `is_fake_adapter`-equivalent field threaded onto `conversation.response`, or a join to
   `gateway_client.completion` by session/time proximity; neither is built here (backend files, out
   of this change's scope).

---

## 10. What this document deliberately does not cover

- **Barge-in yield timing as an audible fact** — owned by `audio-gate/`, not duplicated (§0, §8a).
- **Confirmation/load-direction as a hard veto** — owned by `evals/load_direction/` +
  `conversation_guard.py`, not duplicated (§6).
- **Every item already on `docs/UX-HUMAN-REVIEW.md`'s H1–H12** — cross-referenced, not rewritten.
- **Voice quality / TTS naturalness** — out of scope for both barge-in and endpointing research;
  belongs with `evals/utmosv2` (already in this repo).
- **Crisis/self-harm escalation design** — explicitly out of scope per
  `adhd-conversation-design/SKILL.md`'s own stated boundary; this document inherits that boundary.
- **A full read of the paywalled §6 Springer paper** — cited at reduced confidence, not chased
  further (see §6).
- **A primary numeric source for OpenAI's or Google's production voice latency** — both vendors'
  own docs were checked directly and neither publishes one (§3); this is reported as an absence, not
  papered over with a third-party estimate.
