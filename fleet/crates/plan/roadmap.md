# plan: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt

GVS5H runs an explicit ideation phase (`_ideation_worker`) before its manager commits to a task list: propose several genuinely distinct candidate approaches, prose only, no code, with pitfalls noted for each. This crate has no equivalent — it goes straight from requirements/evidence to a single plan proposal. Add an ideation step for ambiguous/high-uncertainty task kinds (investigate, feature, refactor) that generates 2-3 distinct candidate approaches for plan-review to choose among, instead of planning a single approach up front.
