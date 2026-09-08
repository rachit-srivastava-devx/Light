"""Alternatives/estimate/edge-case parsing and the generic-question heuristic."""

from __future__ import annotations

import re
from collections.abc import Sequence

from .errors import SOWRefusal
from .model import Alternative
from .parse_sections import _strip_bullet

_GENERIC_QUESTIONS = (
    "what are the requirements?",
    "what is the expected outcome?",
    "please provide more details.",
    "please provide more details",
    "can you clarify?",
    "what should we do?",
)


def _parse_alternatives(explicit: Sequence[str]) -> tuple[Alternative, ...]:
    alternatives: list[Alternative] = []
    for line in explicit:
        text = _strip_bullet(line)
        name, separator, remainder = text.partition(":")
        tradeoff_match = re.search(r"\s*\|\s*trade-?off\s*:\s*", remainder, flags=re.IGNORECASE)
        description = remainder[: tradeoff_match.start()] if tradeoff_match else remainder
        tradeoff = remainder[tradeoff_match.end() :] if tradeoff_match else ""
        if not separator or not name.strip() or not description.strip() or tradeoff_match is None:
            raise SOWRefusal(
                "alternative must be named and use 'NAME: DESCRIPTION | tradeoff: TRADEOFF'"
            )
        if not tradeoff.strip():
            raise SOWRefusal(f"alternative {name.strip()!r} has an empty trade-off")
        alternatives.append(Alternative(name.strip(), description.strip(), tradeoff.strip()))
    if len(alternatives) < 2:
        raise SOWRefusal("SOW needs at least two named alternatives")
    names = [alternative.name.casefold() for alternative in alternatives]
    if len(names) != len(set(names)):
        raise SOWRefusal("SOW alternatives must have distinct names")
    return tuple(alternatives)


def _parse_estimate(explicit: Sequence[str]) -> str:
    estimate = " ".join(_strip_bullet(line) for line in explicit if _strip_bullet(line)).strip()
    if not estimate:
        raise SOWRefusal("SOW ESTIMATE is missing")
    return estimate


def _parse_edge_cases(explicit: Sequence[str]) -> tuple[str, ...]:
    edge_cases = tuple(_strip_bullet(line) for line in explicit if _strip_bullet(line))
    if not edge_cases:
        raise SOWRefusal("SOW has no enumerated edge cases")
    return edge_cases


def _is_generic_question(question: str) -> bool:
    normalized = re.sub(r"\s+", " ", question.strip().lower())
    if normalized in _GENERIC_QUESTIONS or normalized.rstrip("?") in {
        item.rstrip("?") for item in _GENERIC_QUESTIONS
    }:
        return True
    return bool(
        re.fullmatch(
            r"(?:what|which|how) (?:are|is|should|do|does) (?:the )?"
            r"(?:requirements|goal|expected outcome|details|next steps|scope)\??",
            normalized,
        )
    )
