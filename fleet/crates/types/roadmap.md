# types: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt

The single most concrete, cheaply-adoptable idea from GVS5H: add `infra_exhausted` and `finish_reason` to the canonical shared vocabulary. Confirmed via repo-wide grep — zero hits for either concept anywhere in `crates/`. Every other crate's adaptation involving "the model never gave a clean answer" vs. "the model answered and failed" depends on this distinction existing here first.
