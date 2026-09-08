"""Atomic-leaf construction from preamble or an explicit Leaves: section."""

from __future__ import annotations

import re
from collections.abc import Sequence

from .errors import SOWRefusal
from .leaf import AtomicLeaf
from .parse_sections import _strip_bullet

_PREDICATE_MARKER = re.compile(r"(?:\|\s*)?(?:predicate|acceptance)\s*[:=]\s*", re.IGNORECASE)


def _build_leaves(preamble: Sequence[str], explicit: Sequence[str]) -> tuple[AtomicLeaf, ...]:
    raw = [_strip_bullet(line) for line in explicit if _strip_bullet(line)]
    if not raw:
        body = " ".join(preamble)
        body = re.sub(r"^\s*task\s*:\s*", "", body, flags=re.IGNORECASE).strip()
        raw = [part.strip() for part in re.split(r"(?<=[.!?])\s+", body) if part.strip()]
    if not raw:
        raise SOWRefusal("SOW has no atomic leaves")

    leaves: list[AtomicLeaf] = []
    for index, requirement_line in enumerate(raw, start=1):
        predicate_matches = list(_PREDICATE_MARKER.finditer(requirement_line))
        requirement = requirement_line
        if predicate_matches:
            requirement = requirement_line[: predicate_matches[0].start()].rstrip(" |:")
            values = []
            for marker_index, marker in enumerate(predicate_matches):
                end = (
                    predicate_matches[marker_index + 1].start()
                    if marker_index + 1 < len(predicate_matches)
                    else len(requirement_line)
                )
                values.append(requirement_line[marker.end() : end].strip(" |;:"))
            # An empty marker is deliberately represented as zero predicates so
            # the cardinality gate reports the actual defect to the CLI caller.
            predicates = tuple(value for value in values if value)
        else:
            # The no-model implementation emits a deterministic, evaluable
            # presence check for every proposed requirement.
            predicates = (f"requirement_present({index})",)
        leaves.append(AtomicLeaf(f"L{index}", requirement, predicates))
    return tuple(leaves)
