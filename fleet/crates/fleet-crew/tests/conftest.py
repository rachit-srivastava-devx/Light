import subprocess
import sys
from pathlib import Path

import pytest

from crew.parity import Observation

CRATE_ROOT = Path(__file__).resolve().parents[1]

PARITY_MARGIN = 0.5
PARITY_JUSTIFICATION = "Half a point is the preregistered maximum immaterial rubric difference."


def synthetic_observations(*, shift: float = 0.0, runs: int = 8, tasks: int = 8) -> list[Observation]:
    observations = []
    offsets = (-0.30, -0.20, -0.10, 0.0, 0.0, 0.10, 0.20, 0.30)
    for prompter, group_shift in (("junior", 0.0), ("senior", shift)):
        for task in range(1, tasks + 1):
            for run in range(1, runs + 1):
                observations.append(
                    Observation(prompter, f"task-{task}", run, 5.0 + offsets[run - 1] + group_shift)
                )
    return observations

SOW_CLI_VALID_TAIL = """Challenges:
- source citation [crew/sow/model.py:1]
Clarifications:
- technical: which parser mode is required? [SOW line 1]
Alternatives:
- Bridge: invoke crew.sow | tradeoff: Python remains a runtime dependency
- Port: implement validation in Rust | tradeoff: duplicates the source of truth
Estimate:
- 1 engineer-day, assuming no contract changes
Edge cases:
- Python is unavailable
- task text differs by whitespace
"""


def run_sow_cli(task: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-m", "crew.sow.cli", "--task", task],
        cwd=CRATE_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


@pytest.fixture()
def repo(tmp_path: Path) -> Path:
    """A tmpdir repo root with one real, five-line source file for citations."""

    source = tmp_path / "src" / "parser.py"
    source.parent.mkdir(parents=True)
    source.write_text("\n".join(f"line {n}" for n in range(1, 6)) + "\n", encoding="utf-8")
    return tmp_path
