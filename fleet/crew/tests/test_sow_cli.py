import json
import subprocess
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
CREW = ROOT / "crew"


def run_cli(task: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-m", "crew.sow", "--task", task],
        cwd=CREW,
        text=True,
        capture_output=True,
        check=False,
    )


VALID_TAIL = """Challenges:
- source citation [crew/crew/sow.py:1]
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


@pytest.mark.parametrize(
    ("leaf", "message"),
    [
        ("- parser requirement | acceptance:", "has 0 acceptance predicates"),
        (
            "- parser requirement | acceptance: first | acceptance: second",
            "has 2 acceptance predicates",
        ),
    ],
)
def test_cli_refuses_invalid_leaf_predicate_cardinality(leaf: str, message: str) -> None:
    result = run_cli(f"Task: implement the parser.\nLeaves:\n{leaf}\n{VALID_TAIL}")

    assert result.returncode == 7
    assert message in result.stderr
    assert result.stdout == ""


@pytest.mark.parametrize(
    ("challenge", "message"),
    [
        (
            "- source risk without evidence",
            "challenge row has no file:line or corpus citation",
        ),
        (
            "- source risk [crew/crew/not_a_real_file.py:1]",
            "challenge citation is not a real file:line or corpus id",
        ),
    ],
)
def test_cli_refuses_challenge_without_real_citation(challenge: str, message: str) -> None:
    task = f"""Task: implement the parser.
Leaves:
- parser requirement | acceptance: requirement_present(1)
Challenges:
{challenge}
Clarifications:
- technical: which parser mode is required? [SOW line 1]
Alternatives:
- Bridge: invoke crew.sow | tradeoff: Python remains a runtime dependency
- Port: implement validation in Rust | tradeoff: duplicates the source of truth
Estimate:
- 1 engineer-day
Edge cases:
- invalid challenge evidence
"""

    result = run_cli(task)

    assert result.returncode == 7
    assert message in result.stderr
    assert result.stdout == ""


def test_cli_refuses_clarification_without_sow_line() -> None:
    task = """Task: implement the parser.
Leaves:
- parser requirement | acceptance: requirement_present(1)
Challenges:
- source citation [crew/crew/sow.py:1]
Clarifications:
- technical: which parser mode is required?
"""

    result = run_cli(task)

    assert result.returncode == 7
    assert "clarification has no SOW line trace" in result.stderr
    assert result.stdout == ""


def test_cli_accepts_valid_sow_and_prints_json() -> None:
    result = run_cli(
        """Task: implement the parser.
Leaves:
- parser requirement | acceptance: requirement_present(1)
""" + VALID_TAIL
    )

    assert result.returncode == 0
    # `python -m crew.sow` emits a benign <frozen runpy> RuntimeWarning because the module is
    # also imported in-process by this test file. Assert the absence of a real fault, not of
    # all output -- an exact == "" here fails on a warning that is not the product's.
    assert "Traceback" not in result.stderr
    assert "REFUS" not in result.stderr.upper()
    payload = json.loads(result.stdout)
    assert payload["leaves"][0]["acceptance_predicates"] == [
        {"expression": "requirement_present(1)"}
    ]
    assert payload["challenges"][0]["citation"] == "crew/crew/sow.py:1"
    assert payload["clarifications"][0]["sow_line"] == 1
    assert len(payload["alternatives"]) == 2
    assert payload["ESTIMATE"]
    assert [row["number"] for row in payload["edge_cases"]] == [1, 2]


def test_cli_vague_task_asks_line_traced_questions() -> None:
    result = run_cli("add a flag")

    assert result.returncode == 7
    assert "CLARIFYING QUESTIONS" in result.stderr
    assert "[SOW line 1]" in result.stderr
