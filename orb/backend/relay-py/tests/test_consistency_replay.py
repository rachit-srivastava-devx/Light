"""Consistency/determinism replay harness (eval/consistency_replay.py).

The invariant under test: `step_count_mode_agreement` must be a REAL measurement — the harness
must actually call `atomize()` K times per task and actually detect both agreement and
disagreement — not a hardcoded or pre-baked number. See eval/consistency_replay.py's module
docstring for the harness's honest claim boundary (it proves the replay machinery is correct
against a scripted fake gateway; it is not evidence that a live model is self-consistent).
"""

from __future__ import annotations

import json

import pytest

from orb_relay.eval.consistency_replay import (
    ConsistencyCase,
    ScriptedGatewayClient,
    consistency_mode_agreement_metric,
    mode_agreement_ratio,
    replay_case,
    run_consistency_replay,
)

VALID_ONE_STEP = {
    "steps": [{"step_text": "Open the document", "est_min": 1, "done_signal": "document is open"}],
    "steps_total": 1,
}
VALID_TWO_STEP = {
    "steps": [
        {"step_text": "Open the document", "est_min": 1, "done_signal": "document is open"},
        {"step_text": "Write the first sentence", "est_min": 3, "done_signal": "one sentence is written"},
    ],
    "steps_total": 2,
}


def test_mode_agreement_ratio_all_agree() -> None:
    assert mode_agreement_ratio([1, 1, 1, 1, 1]) == 1.0


def test_mode_agreement_ratio_detects_split() -> None:
    # 3/5 majority -> a real, non-trivial ratio, not 1.0 and not 0.0.
    assert mode_agreement_ratio([1, 1, 1, 2, 2]) == pytest.approx(0.6)


def test_mode_agreement_ratio_empty_is_zero() -> None:
    assert mode_agreement_ratio([]) == 0.0


async def test_replay_case_calls_atomize_once_per_scripted_output() -> None:
    case = ConsistencyCase(
        id="consistent-task",
        task="send one email",
        scripted_outputs=(VALID_ONE_STEP, VALID_ONE_STEP, VALID_ONE_STEP, VALID_ONE_STEP, VALID_ONE_STEP),
    )
    steps_totals = await replay_case(case)
    assert steps_totals == [1, 1, 1, 1, 1]


async def test_replay_case_surfaces_real_disagreement() -> None:
    """The harness must be able to observe disagreement, not just report perfect agreement — this
    is what proves the metric isn't hardcoded.
    """
    case = ConsistencyCase(
        id="inconsistent-task",
        task="organize the closet",
        scripted_outputs=(VALID_ONE_STEP, VALID_TWO_STEP, VALID_ONE_STEP, VALID_TWO_STEP, VALID_ONE_STEP),
    )
    steps_totals = await replay_case(case)
    assert steps_totals == [1, 2, 1, 2, 1]
    assert mode_agreement_ratio(steps_totals) == pytest.approx(0.6)


async def test_run_consistency_replay_covers_every_case() -> None:
    cases = [
        ConsistencyCase(id="a", task="task a", scripted_outputs=(VALID_ONE_STEP,) * 5),
        ConsistencyCase(id="b", task="task b", scripted_outputs=(VALID_TWO_STEP,) * 5),
    ]
    results = await run_consistency_replay(cases)
    assert set(results.keys()) == {"a", "b"}
    assert results["a"] == [1, 1, 1, 1, 1]
    assert results["b"] == [2, 2, 2, 2, 2]


def test_consistency_mode_agreement_metric_mixed_population() -> None:
    # 2 of 3 tasks individually agree >= 0.9 -> metric is 2/3, not 1.0 and not 0.0.
    replay_results = {
        "agrees-fully": [1, 1, 1, 1, 1],
        "agrees-fully-2": [2, 2, 2, 2, 2],
        "disagrees": [1, 2, 1, 2, 1],
    }
    metric = consistency_mode_agreement_metric(replay_results)
    assert metric == pytest.approx(2 / 3)


def test_consistency_mode_agreement_metric_fails_closed_on_empty() -> None:
    assert consistency_mode_agreement_metric({}) == -1


async def test_scripted_gateway_client_exhaustion_raises() -> None:
    gateway = ScriptedGatewayClient([json.dumps(VALID_ONE_STEP)])
    await gateway.complete(tenant_id="t", system="s", user_text="task", max_tokens=10)
    with pytest.raises(RuntimeError):
        await gateway.complete(tenant_id="t", system="s", user_text="task", max_tokens=10)


def test_scripted_gateway_client_requires_at_least_one_response() -> None:
    with pytest.raises(ValueError):
        ScriptedGatewayClient([])
