# fleet-crew: roadmap

Source: https://github.com/slee-persis/GVS5H ("ledger-based zero-shot self-orchestration" paper + benchmark harness). Reviewed 2026-09-13 against this crate's actual code, not just its description.

## Verdict: Adapt

Confirmed via grep: `crew/adapters/runner.py` and `crew/adapters/capability_probe.py` have **zero** retry/timeout/context-overflow handling. GVS5H's `openai_chat` (in `orchestrator.py`) has a concrete, transport-agnostic pattern worth porting: on a context-length-exceeded error, shrink the output budget and retry rather than fail outright; on HTTP 429, honor `retry-after` with bounded backoff; on an empty or suspiciously-clamped response, reroute instead of returning it as-is. These failure modes apply to CLI-subprocess adapters (claude.py/codex.py here) just as much as GVS5H's raw HTTP calls.

Already ahead of GVS5H elsewhere: `crew/parity/_tost.py` runs real two-one-sided-test statistical equivalence testing between adapters/models — more rigorous than GVS5H's raw pass@1 percentage reporting. Nothing to import on that front.
