import json

import pytest

from crew.parity import ParityRefusal, read_observations


def _lines(*rows: dict) -> list[str]:
    return [json.dumps(row) for row in rows]


def _row(**overrides) -> dict:
    base = {"prompter": "junior", "task": "task-1", "run": 1, "score": 5.0}
    base.update(overrides)
    return base


def test_reads_valid_jsonl() -> None:
    observations = read_observations(_lines(_row(), _row(run=2, score=4.5)))
    assert len(observations) == 2
    assert observations[0].prompter == "junior"


def test_blank_line_is_refused() -> None:
    with pytest.raises(ParityRefusal, match="blank JSONL record"):
        read_observations(["   "])


def test_invalid_json_is_refused() -> None:
    with pytest.raises(ParityRefusal, match="invalid JSON"):
        read_observations(["{not json"])


def test_missing_field_is_refused() -> None:
    with pytest.raises(ParityRefusal, match="expected exactly"):
        read_observations(_lines({"prompter": "junior", "task": "task-1", "run": 1}))


def test_non_finite_score_is_refused() -> None:
    with pytest.raises(ParityRefusal, match="finite"):
        read_observations(_lines(_row(score=float("nan"))))


def test_duplicate_observation_is_refused() -> None:
    with pytest.raises(ParityRefusal, match="duplicate"):
        read_observations(_lines(_row(), _row()))


def test_zero_observations_is_refused() -> None:
    with pytest.raises(ParityRefusal, match="zero observations"):
        read_observations([])


def test_boolean_run_is_refused() -> None:
    with pytest.raises(ParityRefusal, match="positive integer"):
        read_observations(_lines(_row(run=True)))
