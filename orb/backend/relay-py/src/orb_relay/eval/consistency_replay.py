"""K x N consistency/determinism replay harness for the Atomizer.

docs/BUILD-DIGEST.md §3/§6: "consistency gates: K=10 x N=200, step_count_mode_agreement >= 90%".
This module actually drives `proxy.atomizer.atomize()` K times per fixture task through the
injectable `GatewayClient` seam (`proxy/gateway_client.py`) and counts real agreement on
`steps_total` across the K runs — it does not fabricate an agreement number. That is the fix for
the previous state of `eval/gates.py`, which computed `step_count_mode_agreement` from a
synthetic, corpus-independent generator (`{"runs": [1]*9 + [1 + index % 2]}`) that never called
the atomizer at all.

**Claim boundary — read before citing this number**: there is no live `ANTHROPIC_API_KEY` in this
build environment, so every case here is replayed against `ScriptedGatewayClient`, a deterministic
fake that plays back a fixed, checked-in sequence of K completions per task
(`domain/evalsets/t0-contract-corpus.v1.json`'s `consistency_cases[].scripted_outputs`). Running
this harness therefore proves:
  1. `atomize()` is actually invoked K times per task through the real schema + semantic
     validation pipeline (not a mock of the whole pipeline).
  2. The mode-agreement arithmetic correctly detects both agreement AND disagreement (see
     `test_consistency_replay.py`'s deliberately-inconsistent fixture case and disagreement unit
     test).
It does NOT prove that a live model is self-consistent across repeated calls — that requires
running this exact harness with `HttpGatewayClient` against a real gateway-sidecar and a real
`ANTHROPIC_API_KEY`, which this environment cannot do.

**Scale gap vs. the blueprint**: the blueprint target is K=10 x N=200. This harness defaults to
K=5 (`case`-scoped, one scripted response per run) over a small hand-authored N (see the corpus's
`consistency_cases` — currently ~10, not 200) to stay runnable without real API spend. Getting to
K=10 x N=200 needs either 2000 real model calls captured once and replayed, or a live-key run
wired the same way — neither exists yet.
"""

from __future__ import annotations

import json
from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass

from ..cost.meter import UsageDelta
from ..proxy.atomizer import atomize
from ..proxy.gateway_client import GatewayCompletion

# §3: "consistency gates: K=10 x N=200". Kept at 5 here — see module docstring's claim boundary.
DEFAULT_CONSISTENCY_K = 5

# §3/§6: the gate's own agreement bar, applied per-task before it is counted as "agreeing".
MODE_AGREEMENT_THRESHOLD = 0.90


@dataclass(frozen=True)
class ConsistencyCase:
    """One fixture task plus the K scripted model completions to replay for it, in call order."""

    id: str
    task: str
    scripted_outputs: tuple[dict[str, object], ...]


class ScriptedGatewayClient:
    """Deterministic fake `GatewayClient` (structural match to the Protocol in
    `proxy/gateway_client.py`) for the T0 replay harness. Plays back a fixed, ordered sequence of
    JSON completion strings — one per `complete()` call, no randomness, no wall clock — so K
    repeated "runs" over the same task are fully reproducible and can be scripted to agree or
    disagree on purpose.
    """

    def __init__(self, responses: Sequence[str]) -> None:
        if not responses:
            raise ValueError("ScriptedGatewayClient needs at least one scripted response")
        self._responses = list(responses)
        self._call_count = 0

    async def complete(
        self, *, tenant_id: str, system: str, user_text: str, max_tokens: int
    ) -> GatewayCompletion:
        if self._call_count >= len(self._responses):
            raise RuntimeError(
                f"ScriptedGatewayClient exhausted its {len(self._responses)} scripted responses "
                "— the atomizer called complete() more times than the harness scripted (e.g. a "
                "repair-retry it didn't account for)"
            )
        text = self._responses[self._call_count]
        self._call_count += 1
        # Token counts are a rough deterministic proxy (word count), not a provider claim — this
        # is a T0 fake gateway and must never be read as real usage.
        return GatewayCompletion(
            text=text,
            usage=UsageDelta(
                llm_tokens_in=max(1, len(user_text.split())),
                llm_tokens_out=max(1, len(text.split())),
            ),
        )


async def replay_case(
    case: ConsistencyCase, *, tenant_id: str = "t0-consistency-replay"
) -> list[int]:
    """Runs `atomize(case.task, ...)` once per scripted output, returning the observed
    `steps_total` from each real (validated) run, in order. K == len(case.scripted_outputs).
    """
    gateway = ScriptedGatewayClient([json.dumps(output) for output in case.scripted_outputs])
    steps_totals: list[int] = []
    for _ in range(len(case.scripted_outputs)):
        result = await atomize(case.task, gateway=gateway, tenant_id=tenant_id)
        steps_totals.append(result.output.steps_total)
    return steps_totals


async def run_consistency_replay(
    cases: Sequence[ConsistencyCase], *, tenant_id: str = "t0-consistency-replay"
) -> dict[str, list[int]]:
    """Runs `replay_case` for every fixture task. Sequential by design: this is an eval-time
    replay over a handful of fixtures, not a hot path — concurrency would buy nothing but
    complexity here.
    """
    results: dict[str, list[int]] = {}
    for case in cases:
        results[case.id] = await replay_case(case, tenant_id=tenant_id)
    return results


def mode_agreement_ratio(steps_totals: Sequence[int]) -> float:
    """Fraction of the K runs that landed on the single most common `steps_total`. 1.0 means every
    run agreed; a K=5 run with a 3/2 split returns 0.6.
    """
    if not steps_totals:
        return 0.0
    counts = Counter(steps_totals)
    return max(counts.values()) / len(steps_totals)


def consistency_mode_agreement_metric(
    replay_results: Mapping[str, Sequence[int]], *, threshold: float = MODE_AGREEMENT_THRESHOLD
) -> float:
    """The `step_count_mode_agreement` CI metric: fraction of fixture TASKS whose own K-run mode
    agreement ratio clears `threshold`. Missing/empty replay results fail closed to -1, matching
    the fail-closed convention `evaluate_metrics` already uses for absent metrics.
    """
    if not replay_results:
        return -1
    agreeing = sum(
        1 for runs in replay_results.values() if mode_agreement_ratio(runs) >= threshold
    )
    return agreeing / len(replay_results)
