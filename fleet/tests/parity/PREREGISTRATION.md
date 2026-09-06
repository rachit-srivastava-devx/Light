# S4b prompter-parity — pre-registration

Committed **before** the observations were collected. The margin below was chosen from the rubric's
structure, not from the data. Git history is the evidence: this file's commit precedes the commit
carrying `var/parity/observations.jsonl`.

## Design
- 2 prompters (terse "junior" phrasing vs detailed "senior" phrasing of the *same* task)
- 8 tasks × 8 runs → **N = 128 observations**
- Agent: `stub` (deterministic), so this measures the *harness's* prompter sensitivity with model
  variance held at zero. A live-model run is a separate, later experiment and will differ.

## Margin: **0.5 rubric points**
The rubric has **4 mechanical items** (diff applies · artifact frozen · attestation verifies ·
receipt written), so a score is an integer 0–4. Half a rubric point is below the smallest
observable difference between two runs, which makes it the largest difference that is
unambiguously *not* a practical difference in delivered output. A margin of 1.0 would permit a
full rubric item to differ between a junior and a senior and still be called equivalent; that is
exactly the claim under test, so 1.0 would beg the question.

## Test
TOST (two one-sided tests), α = 0.05, equivalence bounds ±0.5.
**Not** a non-significant ANOVA: absence of significance is not evidence of equivalence.

## Declared in advance
- `EQUIVALENT` — both one-sided tests reject; the 90% CI on the difference lies inside ±0.5.
- `NOT-EQUIVALENT` — it does not.
- `UNDERPOWERED` — N cannot detect the margin. This is a distinct verdict and must never be
  reported as equivalence.

I do not know the outcome at the time of writing, and a `NOT-EQUIVALENT` result will be published
as-is rather than re-run with a wider margin.

---

## Amendment, 2026-08-24 — rubric changed from 4 items to 10

Written **before** the live analysis was run; the observations exist (`var/parity/live.jsonl`,
128/128) but no TOST has been executed against them at the time of writing. Git history is again
the evidence: this amendment's commit precedes the commit carrying the result.

`D32` showed the original 4-item rubric measured the deterministic kernel, not the output, and hit
a ceiling at 4/4. It has been replaced by 10 output-shaped items and the agent is now `freelane`
(a real, prompt-sensitive model), so the pilot shows genuine spread (N=12, min=0, max=4).

**The margin stays 0.5, and the justification is unchanged in substance.** Scores remain integers;
one rubric item is worth 1.0, so half a point is still strictly below the smallest observable
difference between two runs. Widening the margin because the scale grew from 0–4 to 0–10 would
make the test easier to pass at exactly the moment the test became capable of failing, which is
the manoeuvre pre-registration exists to prevent.

I do not know the outcome. A `NOT-EQUIVALENT` result will be published as-is; I will not re-run
with a wider margin, a different rubric, or a different model to obtain a friendlier number.

---

## Second amendment, 2026-08-24 — re-run with three lanes

Written before the second live analysis. `D33`'s run returned NOT-EQUIVALENT with 99 of 128
observations scoring zero, because the single lane was busy or rate-limited: the variance was
service flakiness, not prompter effect. Three keyless lanes with proven failover now exist
(`bin/lanes.conf`, verified by `bin/lane-probe.sh`), so the nuisance factor should shrink.

**Nothing about the test changes.** Margin stays ±0.5, α stays 0.05, N stays 128, TOST not ANOVA.
The only change is the agent's reliability. Loosening the test because the previous run failed
would be the exact manoeuvre pre-registration exists to prevent.

**Declared in advance:** if the zero rate is still above ~30%, the result is reported as
uninterpretable regardless of the verdict — a difference measured mostly over failed runs is not a
difference between prompters. I will publish the zero rate beside the verdict either way.

---

## Third amendment, 2026-08-24 — pacing added, test unchanged

`D55` established the remaining obstacle was call rate: sequential runs succeed, twelve rapid ones
do not. The harness now paces observations (`PARITY_PACE_S`, default 6s) and the pilot requires a
floor of scoreable observations rather than an exact count, so one skipped run no longer aborts the
experiment.

**Nothing about the test changes.** Margin ±0.5, α 0.05, N 128, TOST not ANOVA, and the >30%
zero-rate rule from the second amendment still stands. Pacing changes how fast observations are
collected, not what counts as one.

First paced pilot: **N=7, min 4, max 6** — real spread from a real model, which is the first time
this experiment has had data capable of answering the question rather than data about its own
plumbing.
