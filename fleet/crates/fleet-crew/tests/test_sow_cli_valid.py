import json

from tests.conftest import SOW_CLI_VALID_TAIL, run_sow_cli


def test_cli_accepts_valid_sow_and_prints_json() -> None:
    result = run_sow_cli(
        "Task: implement the parser.\nLeaves:\n"
        "- parser requirement | acceptance: requirement_present(1)\n" + SOW_CLI_VALID_TAIL
    )

    assert result.returncode == 0
    assert "Traceback" not in result.stderr
    assert "REFUS" not in result.stderr.upper()
    payload = json.loads(result.stdout)
    assert payload["leaves"][0]["acceptance_predicates"] == [
        {"expression": "requirement_present(1)"}
    ]
    assert payload["challenges"][0]["citation"] == "crew/sow/model.py:1"
    assert payload["clarifications"][0]["sow_line"] == 1
    assert len(payload["alternatives"]) == 2
    assert payload["ESTIMATE"]
    assert [row["number"] for row in payload["edge_cases"]] == [1, 2]


def test_cli_vague_task_asks_line_traced_questions() -> None:
    result = run_sow_cli("add a flag")

    assert result.returncode == 7
    assert "CLARIFYING QUESTIONS" in result.stderr
    assert "[SOW line 1]" in result.stderr
