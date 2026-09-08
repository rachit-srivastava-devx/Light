"""SOW value types: challenges, clarifications, alternatives, and the root."""

from __future__ import annotations

from dataclasses import dataclass, field

from .leaf import AtomicLeaf


@dataclass(frozen=True)
class Challenge:
    """A risk whose evidence is a real source citation or corpus id."""

    risk: str
    citation: str
    mitigation: str = ""


@dataclass(frozen=True)
class Clarification:
    """A question tied to one non-empty SOW line."""

    question: str
    sow_line: int
    category: str = "technical"


@dataclass(frozen=True)
class Alternative:
    """A named implementation option and its explicit trade-off."""

    name: str
    description: str
    tradeoff: str


@dataclass(frozen=True)
class Sow:
    """The complete intent artefact emitted by this layer."""

    restatement: str
    leaves: tuple[AtomicLeaf, ...]
    challenges: tuple[Challenge, ...]
    clarifications: tuple[Clarification, ...]
    alternatives: tuple[Alternative, ...]
    estimate: str
    edge_cases: tuple[str, ...]
    lines: tuple[str, ...] = field(repr=False)
