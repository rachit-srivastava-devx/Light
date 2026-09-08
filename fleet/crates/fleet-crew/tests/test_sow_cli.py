import pytest

from tests.conftest import SOW_CLI_VALID_TAIL, run_sow_cli


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
    result = run_sow_cli(f"Task: implement the parser.\nLeaves:\n{leaf}\n{SOW_CLI_VALID_TAIL}")

    assert result.returncode == 7
    assert message in result.stderr
    assert result.stdout == ""


def test_cli_refuses_challenge_without_real_citation() -> None:
    task = """Task: implement the parser.
Leaves:
- parser requirement | acceptance: requirement_present(1)
Challenges:
- source risk [crew/sow/not_a_real_file.py:1]
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

    result = run_sow_cli(task)

    assert result.returncode == 7
    assert "challenge citation is not a real file:line or corpus id" in result.stderr
    assert result.stdout == ""


def test_cli_refuses_clarification_without_sow_line() -> None:
    task = """Task: implement the parser.
Leaves:
- parser requirement | acceptance: requirement_present(1)
Challenges:
- source citation [crew/sow/model.py:1]
Clarifications:
- technical: which parser mode is required?
"""

    result = run_sow_cli(task)

    assert result.returncode == 7
    assert "clarification has no SOW line trace" in result.stderr
    assert result.stdout == ""
