# planner: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt

Same gap as `plan` — this crate is the module-sequencing half of plan production. Once `plan` produces distinct candidate approaches, `planner` needs to sequence modules against whichever approach was selected, and should surface if a later module reveals the chosen approach doesn't hold up (mirroring GVS5H's manager reissuing a switched approach rather than patching a stuck one).
