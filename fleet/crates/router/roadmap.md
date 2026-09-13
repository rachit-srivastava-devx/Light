# router: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt

Same gap as `route`, one layer down: `Tier` (Lead/Worker/Cheap in `table.rs`) is a fixed, order-sensitive routing table with no failure-triggered promotion path. This is where the escalation policy itself (thresholds, max escalation depth, what counts as "repeatedly failed") should live, with `route`'s scoring consuming it.
