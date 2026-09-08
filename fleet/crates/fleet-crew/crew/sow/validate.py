"""Re-validation of a constructed Sow."""

from __future__ import annotations

from pathlib import Path

from ._paths import _default_repo_root
from .citations import _validate_citation
from .errors import SOWRefusal
from .model import Sow
from .parse_meta_ext import _is_generic_question


def validate_sow(sow: Sow, *, repo_root: Path | None = None) -> Sow:
    """Run every mechanical SOW gate and return the same validated object."""

    root = _default_repo_root(repo_root)
    if not sow.restatement.strip():
        raise SOWRefusal("restatement is empty")
    if not sow.leaves:
        raise SOWRefusal("SOW has zero leaves")
    for leaf in sow.leaves:
        if len(leaf.acceptance_predicates) != 1:
            raise SOWRefusal(f"leaf {leaf.id!r} must have exactly one predicate")
    if not sow.challenges:
        raise SOWRefusal("challenge register is empty")
    for challenge in sow.challenges:
        _validate_citation(challenge.citation, root)
    for clarification in sow.clarifications:
        if clarification.category not in {"business", "technical"}:
            raise SOWRefusal("clarification category must be business or technical")
        if clarification.sow_line < 1 or clarification.sow_line > len(sow.lines):
            raise SOWRefusal(
                f"clarification line {clarification.sow_line} does not exist in the SOW"
            )
        if not sow.lines[clarification.sow_line - 1].strip():
            raise SOWRefusal("clarification must trace to a non-empty SOW line")
        if _is_generic_question(clarification.question):
            raise SOWRefusal(f"generic clarification refused: {clarification.question!r}")
    if len(sow.alternatives) < 2:
        raise SOWRefusal("SOW needs at least two named alternatives")
    if not sow.estimate.strip():
        raise SOWRefusal("SOW ESTIMATE is missing")
    if not sow.edge_cases or any(not edge_case.strip() for edge_case in sow.edge_cases):
        raise SOWRefusal("SOW has no enumerated edge cases")
    return sow
