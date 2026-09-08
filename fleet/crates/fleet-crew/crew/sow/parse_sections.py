"""Task-string preamble/section splitting."""

from __future__ import annotations

import re
from collections.abc import Sequence

from .errors import SOWRefusal

_SECTION = re.compile(
    r"^\s*(?P<name>leaves?|challenges?|clarifications?|alternatives?|estimates?|edge[ -]?cases?)\s*:\s*$",
    re.IGNORECASE,
)

_SECTION_NAMES = {
    "leaf": "leaves",
    "leaves": "leaves",
    "challenge": "challenges",
    "challenges": "challenges",
    "clarification": "clarifications",
    "clarifications": "clarifications",
    "alternative": "alternatives",
    "alternatives": "alternatives",
    "estimate": "estimate",
    "estimates": "estimate",
    "edge case": "edge_cases",
    "edge cases": "edge_cases",
}


def _clean_task(task: str) -> tuple[str, ...]:
    if not isinstance(task, str) or not task.strip():
        raise SOWRefusal("task is empty or not a string")
    lines = tuple(line.rstrip() for line in task.splitlines() if line.strip())
    if not lines:
        raise SOWRefusal("task is empty or whitespace")
    return lines


def _split_sections(lines: Sequence[str]) -> tuple[list[str], dict[str, list[str]]]:
    preamble: list[str] = []
    sections: dict[str, list[str]] = {
        "leaves": [],
        "challenges": [],
        "clarifications": [],
        "alternatives": [],
        "estimate": [],
        "edge_cases": [],
    }
    current: str | None = None
    for line in lines:
        match = _SECTION.match(line)
        if match:
            normalized = match.group("name").lower().replace("-", " ")
            current = _SECTION_NAMES[normalized]
            continue
        if current is None:
            preamble.append(line)
        else:
            sections[current].append(line)
    return preamble, sections


def _strip_bullet(line: str) -> str:
    return re.sub(r"^\s*(?:[-*]|\d+[.)])\s*", "", line).strip()
