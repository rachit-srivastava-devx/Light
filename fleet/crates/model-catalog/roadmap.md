# model-catalog: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt (minor)

Document GVS5H's context-overflow shrink-and-retry heuristic (`orchestrator.py`'s `openai_chat`: on HTTP 400 with a context-length error, reduce the requested output budget and retry) as a named recovery behavior here for the case where a model's context window is unknown or gets exceeded, rather than treating it as an undifferentiated provider error.
