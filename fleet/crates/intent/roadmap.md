# intent: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt (minor)

GVS5H's `_strict()` check (per-model-name fallback that adds explicit, mandatory-format header instructions for models known to ignore free-form formatting) is a cheap robustness pattern worth reusing here as a fallback path for whichever model turns out not to reliably emit schema-constrained IntentSpec JSON on the first try.
