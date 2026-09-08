"""Build a validated Sow from a task string."""

from __future__ import annotations

import re
from collections.abc import Iterable
from pathlib import Path

from .errors import SOWRefusal
from .leaf import AtomicLeaf
from .model import Alternative, Challenge, Clarification, Sow
from .parse_leaves import _build_leaves
from .parse_meta import _parse_challenges, _parse_clarifications
from .parse_meta_ext import _parse_alternatives, _parse_edge_cases, _parse_estimate
from .parse_sections import _clean_task, _split_sections
from .validate import validate_sow


def build_sow(
    task: str,
    *,
    repo_root: Path | None = None,
    leaves: Iterable[AtomicLeaf] | None = None,
    challenges: Iterable[Challenge] | None = None,
    clarifications: Iterable[Clarification] | None = None,
    alternatives: Iterable[Alternative] | None = None,
    estimate: str | None = None,
    edge_cases: Iterable[str] | None = None,
) -> Sow:
    """Build and mechanically validate a SOW from a task string.

    For text-only input, artefacts are read from ``Leaves:``, ``Challenges:``,
    ``Clarifications:``, ``Alternatives:``, ``Estimate:``, and ``Edge cases:``
    sections. A challenge citation must be embedded in the task because this
    layer never fabricates evidence.
    """

    lines = _clean_task(task)
    preamble, sections = _split_sections(lines)
    restatement = " ".join(preamble).strip()
    restatement = re.sub(r"^\s*task\s*:\s*", "", restatement, flags=re.IGNORECASE).strip()
    if not restatement:
        raise SOWRefusal("restatement is empty")
    restatement = (
        f"{restatement} Inferred constraint: preserve existing contract shapes and keep "
        "acceptance checks deterministic."
    )
    generated_leaves = (
        tuple(leaves) if leaves is not None else _build_leaves(preamble, sections["leaves"])
    )
    generated_challenges = (
        tuple(challenges)
        if challenges is not None
        else _parse_challenges(preamble, sections["challenges"])
    )
    generated_clarifications = (
        tuple(clarifications)
        if clarifications is not None
        else _parse_clarifications(sections["clarifications"])
    )
    generated_alternatives = (
        tuple(alternatives)
        if alternatives is not None
        else _parse_alternatives(sections["alternatives"])
    )
    generated_estimate = estimate if estimate is not None else _parse_estimate(sections["estimate"])
    generated_edge_cases = (
        tuple(edge_cases) if edge_cases is not None else _parse_edge_cases(sections["edge_cases"])
    )
    sow = Sow(
        restatement=restatement,
        leaves=generated_leaves,
        challenges=generated_challenges,
        clarifications=generated_clarifications,
        alternatives=generated_alternatives,
        estimate=generated_estimate,
        edge_cases=generated_edge_cases,
        lines=lines,
    )
    return validate_sow(sow, repo_root=repo_root)
