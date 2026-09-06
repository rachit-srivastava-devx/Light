from pathlib import Path

import pytest

from crew.sow import Alternative, AtomicLeaf, Challenge, Clarification, SOWRefusal, build_sow, validate_sow


ROOT = Path(__file__).resolve().parents[2]


def valid_task() -> str:
    return """Task: add deterministic intent validation while preserving the existing contracts.
Challenges:
- contract drift can break generated consumers [contracts/submission.v1.json:1]
Clarifications:
- technical: should the generated predicate be persisted or only returned? [SOW line 1]
Alternatives:
- Bridge: invoke crew.sow from Fleet | tradeoff: requires Python at runtime
- Duplicate: port validation to Rust | tradeoff: risks validator drift
Estimate:
- 1 engineer-day, assuming the existing JSON contract remains stable
Edge cases:
- Python is unavailable
- acceptance belongs to a different exact task
"""


def test_build_sow_has_one_machine_predicate_per_leaf() -> None:
    sow = build_sow(valid_task(), repo_root=ROOT)
    assert sow.restatement
    assert sow.leaves
    assert all(len(leaf.acceptance_predicates) == 1 for leaf in sow.leaves)


def test_generic_clarification_is_refused() -> None:
    task = """Task: implement the parser.
Challenges:
- parser source is covered by the current module [crew/crew/sow.py:1]
Clarifications:
- business: what are the requirements? [SOW line 1]
Alternatives:
- Bridge: invoke Python | tradeoff: runtime dependency
- Port: rewrite in Rust | tradeoff: duplicated logic
Estimate:
- 1 engineer-day
Edge cases:
- empty input
"""
    with pytest.raises(SOWRefusal) as refusal:
        build_sow(task, repo_root=ROOT)
    assert refusal.value.exit_code == 7
    assert refusal.value.receipt["event"] == "refusal"


def test_zero_predicates_are_refused() -> None:
    with pytest.raises(SOWRefusal) as refusal:
        AtomicLeaf("L0", "no predicate", ())
    assert "exactly one" in str(refusal.value)
    assert refusal.value.exit_code == 7


def test_two_predicates_are_refused() -> None:
    with pytest.raises(SOWRefusal) as refusal:
        AtomicLeaf("L2", "two predicates", ("first", "second"))
    assert "exactly one" in str(refusal.value)
    assert refusal.value.exit_code == 7


def test_nonexistent_challenge_path_is_refused() -> None:
    task = "Task: implement the parser."
    with pytest.raises(SOWRefusal) as refusal:
        build_sow(
            task,
            repo_root=ROOT,
            challenges=(Challenge("missing evidence", "does/not/exist.py:99"),),
            clarifications=(),
            alternatives=(
                Alternative("Bridge", "invoke Python", "runtime dependency"),
                Alternative("Port", "rewrite in Rust", "validator drift"),
            ),
            estimate="1 engineer-day",
            edge_cases=("missing citation",),
        )
    assert "real file:line" in str(refusal.value)
    assert refusal.value.exit_code == 7


def test_clarification_must_trace_to_an_existing_line() -> None:
    task = "Task: implement the parser."
    with pytest.raises(SOWRefusal) as refusal:
        build_sow(
            task,
            repo_root=ROOT,
            challenges=(Challenge("source", "crew/crew/sow.py:1"),),
            clarifications=(Clarification("which parser mode is required?", 2),),
            alternatives=(
                Alternative("Bridge", "invoke Python", "runtime dependency"),
                Alternative("Port", "rewrite in Rust", "validator drift"),
            ),
            estimate="1 engineer-day",
            edge_cases=("bad line trace",),
        )
    assert "does not exist" in str(refusal.value)
    assert refusal.value.exit_code == 7
