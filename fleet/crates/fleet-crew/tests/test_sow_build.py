from pathlib import Path

import pytest

from crew.sow import SOWRefusal, build_sow
from crew.sow.leaf import AtomicLeaf


@pytest.fixture()
def repo(tmp_path: Path) -> Path:
    source = tmp_path / "contracts" / "submission.v1.json"
    source.parent.mkdir(parents=True)
    source.write_text("{\n}\n", encoding="utf-8")
    return tmp_path


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


def test_build_sow_has_one_machine_predicate_per_leaf(repo: Path) -> None:
    sow = build_sow(valid_task(), repo_root=repo)
    assert sow.restatement
    assert sow.leaves
    assert all(len(leaf.acceptance_predicates) == 1 for leaf in sow.leaves)


def test_empty_task_is_refused() -> None:
    """§9 gap-closing test: an empty task string is a named, explicit refusal."""

    with pytest.raises(SOWRefusal):
        build_sow("")


def test_two_predicates_on_one_leaf_is_refused() -> None:
    """Mutation-testing target: flipping `len(normalized) != 1` to `< 1` in
    `AtomicLeaf.__post_init__` would let two predicates silently pass. This
    test kills that mutant."""

    with pytest.raises(SOWRefusal, match="exactly one"):
        AtomicLeaf("L2", "two predicates", ("first", "second"))


def test_zero_predicates_are_refused() -> None:
    with pytest.raises(SOWRefusal, match="exactly one"):
        AtomicLeaf("L0", "no predicate", ())


def test_generic_clarification_is_refused(repo: Path) -> None:
    task = """Task: implement the parser.
Challenges:
- parser source is covered by the current module [contracts/submission.v1.json:1]
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
        build_sow(task, repo_root=repo)
    assert refusal.value.exit_code == 7
    assert refusal.value.receipt["event"] == "refusal"
