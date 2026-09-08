import pytest

from crew.parity import Observation, ParityRefusal, analyze_parity
from tests.conftest import PARITY_JUSTIFICATION, PARITY_MARGIN, synthetic_observations

MARGIN = PARITY_MARGIN
JUSTIFICATION = PARITY_JUSTIFICATION


def test_zero_margin_is_refused_before_reading_data() -> None:
    with pytest.raises(ParityRefusal, match="finite positive number"):
        analyze_parity(synthetic_observations(), margin=0.0, margin_justification=JUSTIFICATION)


def test_blank_margin_justification_is_refused() -> None:
    with pytest.raises(ParityRefusal, match="justification must be non-empty"):
        analyze_parity(synthetic_observations(), margin=MARGIN, margin_justification="   ")


def test_single_prompter_is_refused() -> None:
    observations = [Observation("junior", "task-1", run, 5.0) for run in range(1, 6)]
    with pytest.raises(ParityRefusal, match="expected exactly two prompters"):
        analyze_parity(observations, margin=MARGIN, margin_justification=JUSTIFICATION)


def test_zero_variance_both_groups_is_refused_not_equivalent() -> None:
    observations = synthetic_observations(runs=1, tasks=100)
    with pytest.raises(ParityRefusal, match="non-zero variance"):
        analyze_parity(observations, margin=MARGIN, margin_justification=JUSTIFICATION)
