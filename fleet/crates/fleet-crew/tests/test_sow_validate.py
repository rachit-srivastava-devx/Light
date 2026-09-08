from pathlib import Path

import pytest

from crew.sow import Alternative, Challenge, SOWRefusal, build_sow


def alternatives() -> tuple[Alternative, ...]:
    return (
        Alternative("Bridge", "invoke Python", "runtime dependency"),
        Alternative("Port", "rewrite in Rust", "validator drift"),
    )


def test_nonexistent_challenge_path_is_refused(repo: Path) -> None:
    with pytest.raises(SOWRefusal, match="real file:line") as refusal:
        build_sow(
            "Task: implement the parser.",
            repo_root=repo,
            challenges=(Challenge("missing evidence", "does/not/exist.py:99"),),
            clarifications=(),
            alternatives=alternatives(),
            estimate="1 engineer-day",
            edge_cases=("missing citation",),
        )
    assert refusal.value.exit_code == 7


def test_corpus_id_citation_is_accepted(repo: Path) -> None:
    sow = build_sow(
        "Task: implement the parser.",
        repo_root=repo,
        challenges=(Challenge("source", "F1"),),
        clarifications=(),
        alternatives=alternatives(),
        estimate="1 engineer-day",
        edge_cases=("edge",),
    )
    assert sow.challenges[0].citation == "F1"
