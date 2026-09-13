# route: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt

`score_and_select` (`src/score.rs`) picks the single minimum-historical-cost candidate and returns — confirmed no loop, no failure-triggered re-scoring. GVS5H's `escalate()`/`solve_layer` pattern chains cheap→expensive models, handing each layer's answer up as a "candidate, verify and redo" hint with a solver↔critic self-review loop before promotion. Add: when the currently-selected candidate's output repeatedly fails verify, hand the failing candidate plus its failure evidence to the next-cost-tier candidate as a redo hint, instead of only re-trying the same selection.
