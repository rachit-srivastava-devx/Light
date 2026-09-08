"""JSON-serializable view of a Sow."""

from __future__ import annotations

from .model import Sow


def _sow_payload(sow: Sow) -> dict[str, object]:
    """Return the stable JSON shape emitted by the command-line entry point."""

    return {
        "restatement": sow.restatement,
        "leaves": [
            {
                "id": leaf.id,
                "requirement": leaf.requirement,
                "acceptance_predicates": [
                    {"expression": predicate.expression}
                    for predicate in leaf.acceptance_predicates
                ],
            }
            for leaf in sow.leaves
        ],
        "challenges": [
            {"risk": challenge.risk, "citation": challenge.citation, "mitigation": challenge.mitigation}
            for challenge in sow.challenges
        ],
        "clarifications": [
            {
                "question": clarification.question,
                "sow_line": clarification.sow_line,
                "category": clarification.category,
            }
            for clarification in sow.clarifications
        ],
        "alternatives": [
            {
                "name": alternative.name,
                "description": alternative.description,
                "tradeoff": alternative.tradeoff,
            }
            for alternative in sow.alternatives
        ],
        "ESTIMATE": sow.estimate,
        "edge_cases": [
            {"number": index, "case": edge_case}
            for index, edge_case in enumerate(sow.edge_cases, start=1)
        ],
        "lines": list(sow.lines),
    }
