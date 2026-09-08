from pathlib import Path

import pytest

from crew.sow import Alternative, Challenge, Clarification, SOWRefusal, build_sow


def alternatives() -> tuple[Alternative, ...]:
    return (
        Alternative("Bridge", "invoke Python", "runtime dependency"),
        Alternative("Port", "rewrite in Rust", "validator drift"),
    )


def test_clarification_must_trace_to_an_existing_line(repo: Path) -> None:
    with pytest.raises(SOWRefusal, match="does not exist") as refusal:
        build_sow(
            "Task: implement the parser.",
            repo_root=repo,
            challenges=(Challenge("source", "src/parser.py:1"),),
            clarifications=(Clarification("which parser mode is required?", 2),),
            alternatives=alternatives(),
            estimate="1 engineer-day",
            edge_cases=("bad line trace",),
        )
    assert refusal.value.exit_code == 7


def test_generic_clarification_object_is_refused(repo: Path) -> None:
    with pytest.raises(SOWRefusal, match="generic clarification"):
        build_sow(
            "Task: implement the parser.",
            repo_root=repo,
            challenges=(Challenge("source", "src/parser.py:1"),),
            clarifications=(Clarification("what are the requirements?", 1),),
            alternatives=alternatives(),
            estimate="1 engineer-day",
            edge_cases=("edge",),
        )


def test_single_alternative_is_refused(repo: Path) -> None:
    with pytest.raises(SOWRefusal, match="two named alternatives"):
        build_sow(
            "Task: implement the parser.",
            repo_root=repo,
            challenges=(Challenge("source", "src/parser.py:1"),),
            clarifications=(),
            alternatives=(Alternative("Bridge", "invoke Python", "runtime dependency"),),
            estimate="1 engineer-day",
            edge_cases=("edge",),
        )
