# control: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt (minor)

GVS5H's manager forces a strategy switch after the same task is reissued twice with no progress (`multiagent_solve`'s stall check). Fleet's closest analog is the LLD §8 retry policy: 2 repair attempts, then the 3rd recurrence of the same failure signature stops automatic retry and asks a human. Extend that: before falling to "ask a human" on the 3rd recurrence, try one automatic "different approach" instruction first (borrowed from GVS5H's stall→switch pattern), and only ask a human if that also fails.

Naming note: `impl_govern/escalate.rs`'s `Escalation` enum (Continue/Throttle/Downgrade/UseCached/Pause) is a **token-budget consumption ladder**, confirmed unrelated to GVS5H's quality-escalation ladder (see the `router`/`route` roadmaps) — don't conflate the two when implementing this.
