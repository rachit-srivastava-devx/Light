import pytest

from orb_relay.eval.gates import (
    CI_GATES,
    INTENT_RULES_PATH,
    _percentile,
    _safe_intent,
    aggregate_local_corpus,
    all_passed,
    evaluate_metrics,
)
from orb_relay.observability.metrics import (
    AudioGapWatchdog,
    HopHistogram,
    MetricValidationError,
    SentinelCanary,
    loudness_floor_ok,
)
from orb_relay.proxy.phrasers import (
    PhraseValidationError,
    check_in_phrase,
    conversational_response,
    validate_phrase,
)


def passing_metrics() -> dict[str, float]:
    return {gate.metric: gate.threshold for gate in CI_GATES}


def test_eval_gates_pass_on_threshold_boundaries() -> None:
    results = evaluate_metrics(passing_metrics())
    assert all_passed(results) is True


def test_eval_gates_fail_closed_on_missing_metric() -> None:
    metrics = passing_metrics()
    metrics.pop(CI_GATES[0].metric)
    results = evaluate_metrics(metrics)
    assert all_passed(results) is False
    assert results[0].observed == -1


def test_local_domain_corpus_aggregates_only_checked_in_contracts() -> None:
    metrics, evidence = aggregate_local_corpus()
    assert evidence["source"] == "local_domain_corpus"
    claim_boundary = evidence["claim_boundary"]
    assert isinstance(claim_boundary, str)
    assert claim_boundary.startswith("T0 replay")
    assert metrics["is_atomic_rate"] == 1.0
    assert metrics["first_step_startable_rate"] == 1.0
    assert metrics["schema_safety_caught_rate"] == 1.0
    assert metrics["voice_p50_ms"] <= 1100
    assert metrics["voice_p99_ms"] <= 2000
    assert metrics["silence_events"] == 0
    assert metrics["loudness_floor_pass_rate"] == 1.0
    assert metrics["presence_ready_p95_ms"] <= 120
    assert metrics["speech_request_p95_ms"] <= 250
    assert metrics["step_count_mode_agreement"] >= 0.9
    assert metrics["intent_accuracy"] == 1.0
    assert metrics["belief_brier"] <= 0.15
    assert metrics["belief_replay_coverage_rate"] == 1.0
    assert metrics["policy_false_interrupt_rate"] == 0
    assert metrics["cost_paise_per_session_p95"] <= 400
    assert evidence["atomizer_cases"] == 300
    assert evidence["voice_replay_cases"] == 200
    # NOT expanded like the other categories: this is a real K x N atomizer replay
    # (eval/consistency_replay.py), so the count is the real, small, checked-in fixture set —
    # see that module's docstring for why (no live ANTHROPIC_API_KEY in this environment) and how
    # far this still is from the blueprint's K=10 x N=200 target.
    assert evidence["consistency_cases"] == 10
    assert evidence["classifier_cases"] == 500
    assert evidence["belief_replay_cases"] == 5000
    assert evidence["cost_replay_cases"] == 200
    assert evidence["latency_replay_cases"] == 200
    assert all_passed(evaluate_metrics(metrics)) is True


def test_intent_eval_uses_shared_mobile_rule_contract() -> None:
    assert INTENT_RULES_PATH.exists()
    assert _safe_intent("done, next one") == "done"
    assert _safe_intent("I'm stuck, what do I do") == "stuck"
    assert _safe_intent("can I stop?") == "chitchat"


def test_histogram_percentiles_and_bad_samples() -> None:
    h = HopHistogram()
    for value in [10, 20, 30]:
        h.observe(value)
    assert h.percentile(50) == 20
    with pytest.raises(MetricValidationError):
        h.observe(float("nan"))


@pytest.mark.parametrize("sample_count", [1, 2, 3, 4])
@pytest.mark.parametrize("percentile", [0, 1, 25, 50, 75, 95, 99, 100])
def test_percentile_call_sites_share_hdr_nearest_rank(sample_count: int, percentile: int) -> None:
    values = [float(value) for value in range(10, 10 * sample_count + 1, 10)]
    histogram = HopHistogram()
    for value in values:
        histogram.observe(value)

    assert histogram.percentile(percentile) == _percentile(values, percentile)


def test_hdr_percentile_is_nearest_rank_and_memory_is_bounded() -> None:
    histogram = HopHistogram()
    for value in [10, 20, 30, 40]:
        histogram.observe(value)
    assert histogram.percentile(25) == 10

    allocated_buckets = histogram.storage_bucket_count
    for value in range(10_000):
        histogram.observe(float(value))
    assert histogram.total_count == 10_004
    assert histogram.storage_bucket_count == allocated_buckets


def test_gap_watchdog_counts_only_real_gaps() -> None:
    w = AudioGapWatchdog(max_gap_ms=300)
    w.observe_audio(0)
    w.observe_audio(250)
    w.observe_audio(700)
    assert w.gap_events == 1


def test_gap_watchdog_measures_rendered_intervals_and_capture_boundaries() -> None:
    w = AudioGapWatchdog(max_gap_ms=250, session_started_at_ms=0)
    w.observe_interval(100, 1_000)
    w.observe_interval(1_100, 2_000)
    w.finish(2_100)
    assert w.measured is True
    assert w.gap_events == 0
    assert w.longest_gap_ms == 100


def test_gap_watchdog_fails_closed_when_capture_has_no_audio() -> None:
    w = AudioGapWatchdog(max_gap_ms=250, session_started_at_ms=0)
    w.finish(1_000)
    assert w.measured is False
    assert w.gap_events == 1
    assert w.longest_gap_ms == 1_000


def test_gap_watchdog_rejects_overlapping_or_reversed_intervals() -> None:
    w = AudioGapWatchdog(max_gap_ms=250)
    w.observe_interval(10, 20)
    with pytest.raises(MetricValidationError):
        w.observe_interval(19, 30)
    with pytest.raises(MetricValidationError):
        AudioGapWatchdog(max_gap_ms=250).observe_interval(30, 20)


def test_loudness_floor_and_sentinel_canary() -> None:
    assert loudness_floor_ok([-42, -36, -30], floor_dbfs=-35) is True
    assert loudness_floor_ok([], floor_dbfs=-35) is False
    c = SentinelCanary(expected_interval_ms=1000)
    assert c.stale(0) is True
    c.mark_success(100)
    assert c.stale(500) is False
    assert c.stale(1200) is True


def test_phrasers_are_capped_and_fail_closed() -> None:
    assert conversational_response("").source == "deterministic_empty"
    assert check_in_phrase(2, "open the tax portal").source == "deterministic_repeated_stuck"
    with pytest.raises(PhraseValidationError):
        validate_phrase("One. Two. Three.")
    with pytest.raises(PhraseValidationError):
        check_in_phrase(-1, "step")
