import pytest

from crew.parity import Observation, ParityRefusal, analyze_parity
from tests.conftest import PARITY_JUSTIFICATION, PARITY_MARGIN, synthetic_observations

MARGIN = PARITY_MARGIN
JUSTIFICATION = PARITY_JUSTIFICATION


def test_positive_control_same_distribution_is_equivalent() -> None:
    result = analyze_parity(synthetic_observations(), margin=MARGIN, margin_justification=JUSTIFICATION)

    assert result.verdict == "EQUIVALENT"
    assert result.n == 128
    assert result.ci_90[0] > -MARGIN
    assert result.ci_90[1] < MARGIN
    assert result.lower_test_p_value < 0.05
    assert result.upper_test_p_value < 0.05


def test_negative_control_shifted_by_twice_margin_is_not_equivalent() -> None:
    result = analyze_parity(
        synthetic_observations(shift=2 * MARGIN), margin=MARGIN, margin_justification=JUSTIFICATION
    )

    assert result.verdict == "NOT-EQUIVALENT"
    assert result.difference == pytest.approx(-2 * MARGIN)
    assert result.upper_test_p_value < 0.05
    assert result.lower_test_p_value > 0.05


def test_underpowered_control_refuses_instead_of_claiming_equivalence() -> None:
    observations = [
        Observation(prompter, "task-1", run, 5.0)
        for prompter in ("junior", "senior")
        for run in range(1, 6)
    ]

    with pytest.raises(ParityRefusal) as caught:
        analyze_parity(observations, margin=MARGIN, margin_justification=JUSTIFICATION)

    assert caught.value.exit_code == 7
    assert caught.value.result is not None
    assert caught.value.result.verdict == "UNDERPOWERED"
    assert caught.value.result.n == 10
    assert caught.value.result.required_n == 128


def test_underpowered_by_imbalance_despite_high_total_n() -> None:
    """Mutation-testing target: flipping `powered = n >= MIN_TOTAL_N and
    all(...)` to `or` would let a high total N with one skewed prompter
    count as powered. 200 total observations, but "senior" has only 20 --
    below the 64-per-prompter floor -- must still refuse as UNDERPOWERED."""

    observations = [
        Observation("junior", f"task-{task}", run, 5.0 + 0.01 * run)
        for task in range(1, 19)
        for run in range(1, 11)
    ] + [
        Observation("senior", f"task-{task}", run, 5.0 + 0.01 * run)
        for task in range(1, 3)
        for run in range(1, 11)
    ]
    assert len(observations) >= 128
    assert len([o for o in observations if o.prompter == "senior"]) < 64

    with pytest.raises(ParityRefusal) as caught:
        analyze_parity(observations, margin=MARGIN, margin_justification=JUSTIFICATION)

    assert caught.value.result is not None
    assert caught.value.result.verdict == "UNDERPOWERED"
