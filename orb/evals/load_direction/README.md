# `load_direction` — driven end-to-end check of who is doing the thinking

Not a unit test. POSTs a real ADHD conversation to the live relay (`:8765` → gateway `:8082` → a
real model) and scores every reply. Unit tests on `shifts_mental_load` were 204-green while the
running product still handed the load back; this is the instrument that caught it.

```bash
# backend chain must be up first: scripts/dev.sh (or gateway on 8082 + relay-py on 8765)
. scripts/load-env.sh
backend/relay-py/.venv/bin/python evals/load_direction/drive.py
```

Exit `0` only if every turn replied AND there were zero load-shifts, zero vacuous replies, and zero
verbatim repeats. Exit `6` otherwise. Evidence lands in `evidence-load-direction/drive.json`.

## Scored on three axes, because one instrument flatters itself

| axis | what it catches |
|---|---|
| `shifts_mental_load` (the production predicate) | the reply hands the thinking back |
| carries / position / company heuristics | the reply *dodged* the veto by saying nothing useful |
| verbatim-repeat check | the reply is one the user already heard |

`teach` mode is exempt from the vacuity axis: an explanation legitimately proposes no step, and
scoring it as vacuous measured the wrong property.

## What it found (2026-08-28), across five runs

| run | load-shifts | degraded | verbatim repeats | p50 |
|---|---|---|---|---|
| #1 baseline | 0/7 reported — **but the predicate was wrong** | 0 | 0 | 880 ms |
| #2 after 2 predicate fixes | 0/7 | 1 (false positive) | 0 | 888 ms |
| #3 after reciprocity exemption | 0/7 | 0 | **1** | 947 ms |
| #4 with a 2nd refusal turn | 0/8 | 2 (both correct) | 1 (the fallback itself) | 986 ms |
| #5 after repeat control + varied fallback | **0/8** | **0** | **0** | 1060 ms |

Four defects that only a driven run exposed:

1. **The predicate missed real load-shifts.** `"What feels like the easiest thing on your list?"`
   scored clean — the pattern wanted the noun `step` and the pronoun `you`, and the model said
   "thing" and "your". One missing letter, invisible to every unit test.
2. **Prosody markup broke the matcher.** The model emits `"What do [emphasis] you think?"`, so any
   word-sequence pattern fails on a token the listener never hears. Matching now runs on the spoken
   words only.
3. **A false positive that did more harm than the miss.** A genuinely good discussion reply — a real
   position, a counterpoint, then "What do you think?" — was vetoed and replaced with a canned
   task-steering line. Reciprocity in a debate is the product; the veto suppressed it. Hence
   `_BARE_RECIPROCITY`, anchored to the end of a sentence so
   `"what do you think you should do first?"` is still caught.
4. **The orb repeated itself verbatim at an explicit refusal.** User: "no, not that one" → the
   byte-identical previous suggestion. `converse.v1.md` already forbade repeating and the model did
   it anyway, which is the third time in this repo a prompt rule proved to be a `mitigates`. Now a
   structural control (`repeats_prior_reply`) with one bounded repair, then a deterministic fallback.

Then the fallback turned out to commit both sins it was written to prevent — it fired the identical
string twice in one session, and it promised "I'll pick it" while picking nothing. It now varies by
conversation position (deterministically, no RNG) and promises nothing it cannot keep.

## The eight holes, and why this predicate is a `mitigates`

Nine driven runs surfaced **eight distinct phrasings** no earlier version of `shifts_mental_load`
caught. Each was a one-line fix; the point is that there were eight of them, from a model that was
not trying to evade anything:

| # | what got through | why it did |
|---|---|---|
| 1 | "the easiest thing on **your** list" | pattern wanted `\byou\b` |
| 2 | "the easiest **thing**" | pattern wanted the noun `step` |
| 3 | "What do **[emphasis]** you think?" | prosody markup split the phrase |
| 4 | "…you could do right now**.**" | every alternative required `?` |
| 5 | "Which one **were** you hoping…" | `were` was not in the modal list |
| 6 | "the **most pressing** thing" | not in the superlative list |
| 7 | "**another** option" | determiner not in the list |
| 8 | "another option you **see**" | perception verb where a modal was expected |

Eight holes is not a pattern nearly finished — it is evidence the phrasing space is unbounded. So:

**`shifts_mental_load` is `mitigates`, never `kills (structural)`.** It measurably reduces the harm
(every run since #2 has scored 0 load-shifts *that it can see*) and it cannot eliminate it. Recall on
an unseen set is **UNMEASURED**. Do not read the green suite as coverage — the suite contains exactly
the phrasings already found. The durable control is a different class (a judge on egress), which is
the N/25 work in `queue-b/13-load-direction-and-wait-time-companion.md`.

The three controls this file exercises, labelled honestly:

| control | label | why |
|---|---|---|
| `converse.v1.md` / `focus-companion.v1.md` prompt rules | `mitigates` | measured being violated, three separate times |
| `shifts_mental_load` egress veto | `mitigates` | eight known holes; unbounded phrasing space |
| `repeats_prior_reply` egress veto | `kills (mechanical)` | exact-match over spoken words; no judgement, no phrasing space to evade |

## Honest limits

- Latency is HTTP round-trip on the text path only. It says nothing about audio, and the 250 ms
  budget is a separate measurement.
- The heuristics are regexes and will mislabel. Two `[neither]` turns in run #5 were good replies my
  own scorer failed to recognise; the reported number is a floor on quality, not a ceiling.
- 8 turns is one conversation shape. Recall on an adversarial set and precision on a clean set are
  measured by the bounded calibration below; this 8-turn drive remains the production-wire smoke.

## Bounded unseen calibration: 25 adversarial prompts + 20 clean replies

`run_bounded_eval.py` calls the live gateway sidecar with the real domain prompt, preserves each raw
reply, then replays that exact reply through `complete_guarded_conversation`. Gemini judges the raw
and guarded reply together, so the report publishes a real before/after denominator rather than
comparing two unrelated generations.

```bash
# gateway-sidecar must be live on :8082 with ORB_LLM_GATEWAY_ADAPTER=gemini
. scripts/load-env.sh
evals/.venv/bin/python evals/load_direction/run_bounded_eval.py
```

The hard ceiling is 140 provider calls: 25 raw generations, at most 25 one-shot guard repairs, and
45 single-item judge calls with at most one format retry each. Single-item judging is required
because gateway speech normalization collapses line breaks and truncates batched judge rows. Normal
runs spend fewer because only a regex-positive raw reply is repaired and valid judge rows are not
retried. Set `LOAD_DIRECTION_RESUME=1` to reuse the checkpointed 25 raw/guarded pairs after a judge
transport/format failure without paying to generate them again.
Evidence lands in `evals/load_direction/evidence-load-direction/bounded-eval.json` with provider-
reported adapter, model, INR cost, and every verbatim failure.

Metrics are kept separate:

- adversarial recall = regex true positives / Gemini-judged raw load shifts;
- classifier precision = regex true positives / all regex positives;
- clean-set allow precision = legitimate replies allowed / 20 known-good replies;
- before/after = Gemini-judged acceptable raw replies / 25 versus guarded replies / 25.

Control labels remain deliberately narrow: prompt rules, regex, and Gemini judge are `mitigates`;
exact spoken-repeat blocking is `kills (mechanical)`; the bare-refusal no-model short circuit is
`kills (structural)` for the explicitly recognized refusal grammar only.
