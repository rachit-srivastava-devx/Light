# store: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt (minor)

Confirmed via repo-wide grep: no `infra_exhausted`-equivalent concept exists anywhere. Whatever table records a lane/attempt's terminal state should distinguish "gave up after N attempts with no clean model response" (infra failure) from "model responded and was simply wrong" (a real, scoreable outcome) — GVS5H tracks this explicitly because the two cases call for different next steps (retry infra vs. try a different approach).
