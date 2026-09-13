# builder: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt

Two concrete gaps, both confirmed by reading GVS5H's `multiagent.py`:

1. **Cutoff summarization.** When a GVS5H worker is cut off by its token limit mid-solution, `_summarize_cutoff` runs a separate call to summarize the partial attempt (approach pursued, what was established/ruled out, what remained) instead of just discarding it. Fleet's builder has no equivalent (confirmed: no cutoff/truncation handling in `crates/builder/src/impl_/spawn/`). Add this so a budget-exhausted lease attempt leaves a usable digest instead of nothing.
2. **Repair-retry context.** Once `verify`'s `GateEvidence.failures` is structured (see the `verify` roadmap), thread the first failing case into the next lease's prompt on retry instead of a blind re-attempt — this is GVS5H's single biggest scoring lever per the paper's own transcript analysis.
