import json
import subprocess
import sys
from pathlib import Path

import pytest

from crew.parity import Observation, ParityRefusal, analyze_parity


ROOT = Path(__file__).resolve().parents[2]
CREW = ROOT / "crew"
MARGIN = 0.5
JUSTIFICATION = "Half a point is the preregistered maximum immaterial rubric difference."


def synthetic_observations(*, shift: float = 0.0, runs: int = 8) -> list[Observation]:
    observations = []
    # Deterministic symmetric variation: both controls have known means and avoid
    # a flaky random draw changing the expected verdict.
    offsets = (-0.30, -0.20, -0.10, 0.0, 0.0, 0.10, 0.20, 0.30)
    for prompter, group_shift in (("junior", 0.0), ("senior", shift)):
        for task in range(1, 9):
            for run in range(1, runs + 1):
                observations.append(
                    Observation(
                        prompter,
                        f"task-{task}",
                        run,
                        5.0 + offsets[run - 1] + group_shift,
                    )
                )
    return observations


def test_positive_control_same_distribution_is_equivalent() -> None:
    result = analyze_parity(
        synthetic_observations(), margin=MARGIN, margin_justification=JUSTIFICATION
    )

    assert result.verdict == "EQUIVALENT"
    assert result.n == 128
    assert result.ci_90[0] > -MARGIN
    assert result.ci_90[1] < MARGIN
    assert result.lower_test_p_value < 0.05
    assert result.upper_test_p_value < 0.05


def test_negative_control_shifted_by_twice_margin_is_not_equivalent() -> None:
    result = analyze_parity(
        synthetic_observations(shift=2 * MARGIN),
        margin=MARGIN,
        margin_justification=JUSTIFICATION,
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


def test_cli_underpowered_emits_result_and_refusal_receipt(tmp_path: Path) -> None:
    input_path = tmp_path / "observations.jsonl"
    observations = [
        {
            "prompter": prompter,
            "task": "task-1",
            "run": run,
            "score": 5.0,
        }
        for prompter in ("junior", "senior")
        for run in range(1, 6)
    ]
    input_path.write_text("".join(json.dumps(row) + "\n" for row in observations))

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "crew.parity",
            str(input_path),
            "--margin",
            str(MARGIN),
            "--margin-justification",
            JUSTIFICATION,
        ],
        cwd=CREW,
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 7
    result = json.loads(completed.stdout)
    assert result["verdict"] == "UNDERPOWERED"
    assert result["tost"]["p_value"] is None
    assert json.loads(completed.stderr)["event"] == "refusal"
