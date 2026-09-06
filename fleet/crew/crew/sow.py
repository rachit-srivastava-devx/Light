"""Construction and mechanical validation of Statements of Work.

The intent layer has no model adapter yet.  ``build_sow`` therefore accepts a
human/task string and a small, deterministic convention for optional evidence
and clarifications embedded in that string.  It refuses ambiguity instead of
inventing evidence.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable, Sequence


class SOWRefusal(ValueError):
    """A mechanically detected intent defect.

    ``exit_code`` is deliberately typed according to the repository contract.
    The attached receipt is the event a parent ledger writer can persist before
    returning control to its caller.
    """

    exit_code = 7

    def __init__(self, reason: str):
        self.reason = reason
        self.receipt = {
            "event": "refusal",
            "exit_code": self.exit_code,
            "body": {"reason": reason},
        }
        super().__init__(reason)


@dataclass(frozen=True)
class MachinePredicate:
    """One machine-evaluable acceptance expression."""

    expression: str

    def __post_init__(self) -> None:
        if not self.expression.strip():
            raise SOWRefusal("acceptance predicate is empty")


@dataclass(frozen=True)
class AtomicLeaf:
    """An atomic requirement with exactly one acceptance predicate."""

    id: str
    requirement: str
    predicates: tuple[MachinePredicate | str, ...]

    def __post_init__(self) -> None:
        if not self.id.strip() or not self.requirement.strip():
            raise SOWRefusal("atomic leaf must have an id and requirement")
        normalized = tuple(
            predicate
            if isinstance(predicate, MachinePredicate)
            else MachinePredicate(str(predicate))
            for predicate in self.predicates
        )
        object.__setattr__(self, "predicates", normalized)
        if len(normalized) != 1:
            raise SOWRefusal(
                f"leaf {self.id!r} has {len(normalized)} acceptance predicates; expected exactly one"
            )

    @property
    def acceptance_predicates(self) -> tuple[MachinePredicate, ...]:
        return self.predicates  # type: ignore[return-value]

    @property
    def acceptance_predicate(self) -> MachinePredicate:
        return self.acceptance_predicates[0]


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


# References used by the blueprint's failure corpus are intentionally narrow:
# one or more uppercase namespace letters followed by an integer.
_CORPUS_ID = re.compile(r"^[A-Z]{1,3}\d+$")
_FILE_CITATION = re.compile(
    r"(?P<path>(?:/|\.\.?/)?[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*):(?P<line>[1-9]\d*)"
)
_TRACE = re.compile(r"(?:\[\s*)?(?:sow\s*)?line\s*[:=]?\s*(?P<line>\d+)", re.I)
_PREDICATE_MARKER = re.compile(r"(?:\|\s*)?(?:predicate|acceptance)\s*[:=]\s*", re.I)
_SECTION = re.compile(
    r"^\s*(?P<name>leaves?|challenges?|clarifications?|alternatives?|estimates?|edge[ -]?cases?)\s*:\s*$",
    re.I,
)
_GENERIC_QUESTIONS = (
    "what are the requirements?",
    "what is the expected outcome?",
    "please provide more details.",
    "please provide more details",
    "can you clarify?",
    "what should we do?",
)


def _default_repo_root(repo_root: Path | None) -> Path:
    if repo_root is not None:
        return repo_root.resolve()
    # crew/crew/sow.py -> repository root
    return Path(__file__).resolve().parents[2]


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
            current = {
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
            }[normalized]
            continue
        if current is None:
            preamble.append(line)
        else:
            sections[current].append(line)
    return preamble, sections


def _strip_bullet(line: str) -> str:
    return re.sub(r"^\s*(?:[-*]|\d+[.)])\s*", "", line).strip()


def _build_leaves(preamble: Sequence[str], explicit: Sequence[str]) -> tuple[AtomicLeaf, ...]:
    raw = [_strip_bullet(line) for line in explicit if _strip_bullet(line)]
    if not raw:
        body = " ".join(preamble)
        body = re.sub(r"^\s*task\s*:\s*", "", body, flags=re.I).strip()
        raw = [part.strip() for part in re.split(r"(?<=[.!?])\s+", body) if part.strip()]
    if not raw:
        raise SOWRefusal("SOW has no atomic leaves")

    leaves: list[AtomicLeaf] = []
    for index, requirement_line in enumerate(raw, start=1):
        predicate_matches = list(_PREDICATE_MARKER.finditer(requirement_line))
        requirement = requirement_line
        predicates: tuple[str, ...]
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
            predicates = (f'requirement_present({index})',)
        leaves.append(AtomicLeaf(f"L{index}", requirement, predicates))
    return tuple(leaves)


def _find_citations(text: str) -> list[str]:
    citations = [match.group(0) for match in _FILE_CITATION.finditer(text)]
    citations.extend(
        token
        for token in re.findall(r"\b[A-Z]{1,3}\d+\b", text)
        if _CORPUS_ID.fullmatch(token)
    )
    return list(dict.fromkeys(citations))


def _parse_challenges(preamble: Sequence[str], explicit: Sequence[str]) -> tuple[Challenge, ...]:
    source = list(explicit)
    if not source:
        source = [line for line in preamble if _find_citations(line)]
    if not source:
        raise SOWRefusal("challenge register has no evidence citation")

    challenges: list[Challenge] = []
    for line in source:
        text = _strip_bullet(line)
        citations = _find_citations(text)
        if not citations:
            raise SOWRefusal(f"challenge row has no file:line or corpus citation: {text!r}")
        citation = citations[0]
        risk = re.sub(r"\s*(?:\[[^]]+\]|@)?\s*" + re.escape(citation), "", text).strip(" -:")
        challenges.append(Challenge(risk=risk or "unspecified challenge", citation=citation))
    return tuple(challenges)


def _parse_clarifications(explicit: Sequence[str]) -> tuple[Clarification, ...]:
    clarifications: list[Clarification] = []
    for line in explicit:
        text = _strip_bullet(line)
        trace = _TRACE.search(text)
        if trace is None:
            raise SOWRefusal(f"clarification has no SOW line trace: {text!r}")
        question = _TRACE.sub("", text).strip(" []:-")
        question = re.sub(r"^(?:business|technical)\s*:\s*", "", question, flags=re.I)
        category = "business" if re.search(r"\bbusiness\b", text, re.I) else "technical"
        if _is_generic_question(question):
            raise SOWRefusal(f"generic clarification refused: {question!r}")
        clarifications.append(Clarification(question, int(trace.group("line")), category))
    return tuple(clarifications)


def _parse_alternatives(explicit: Sequence[str]) -> tuple[Alternative, ...]:
    alternatives: list[Alternative] = []
    for line in explicit:
        text = _strip_bullet(line)
        name, separator, remainder = text.partition(":")
        tradeoff_match = re.search(r"\s*\|\s*trade-?off\s*:\s*", remainder, flags=re.I)
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


def _resolve_file(citation: str, root: Path) -> tuple[Path, int] | None:
    match = _FILE_CITATION.fullmatch(citation)
    if match is None:
        return None
    path = Path(match.group("path"))
    line = int(match.group("line"))
    candidates = [path] if path.is_absolute() else [root / path, root.parent / path]
    for candidate in candidates:
        resolved = candidate.resolve()
        try:
            resolved.relative_to(root)
        except ValueError:
            continue
        if resolved.is_file():
            try:
                total_lines = len(resolved.read_text(encoding="utf-8").splitlines())
            except (OSError, UnicodeError) as exc:
                raise SOWRefusal(f"cannot read challenge citation {citation!r}") from exc
            if line <= total_lines:
                return resolved, line
    return None


def _validate_citation(citation: str, root: Path) -> None:
    if not citation.strip():
        raise SOWRefusal("challenge citation is empty")
    if _CORPUS_ID.fullmatch(citation):
        return
    if _resolve_file(citation, root) is None:
        raise SOWRefusal(f"challenge citation is not a real file:line or corpus id: {citation!r}")


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
    restatement = re.sub(r"^\s*task\s*:\s*", "", restatement, flags=re.I).strip()
    if not restatement:
        raise SOWRefusal("restatement is empty")
    restatement = (
        f"{restatement} Inferred constraint: preserve existing contract shapes and keep "
        "acceptance checks deterministic."
    )
    generated_leaves = tuple(leaves) if leaves is not None else _build_leaves(preamble, sections["leaves"])
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
            {
                "risk": challenge.risk,
                "citation": challenge.citation,
                "mitigation": challenge.mitigation,
            }
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


def main(argv: Sequence[str] | None = None) -> int:
    """Build a validated SOW from one task string and print JSON."""

    parser = argparse.ArgumentParser(description="Build and validate a Fleet SOW")
    parser.add_argument(
        "--task", required=True, help="task text, including optional SOW sections"
    )
    args = parser.parse_args(argv)
    try:
        sow = build_sow(args.task)
    except SOWRefusal as refusal:
        print(f"SOW refused: {refusal}", file=sys.stderr)
        try:
            lines = _clean_task(args.task)
        except SOWRefusal:
            lines = ()
        if lines:
            print("CLARIFYING QUESTIONS", file=sys.stderr)
            # D42: naming the deficiency is not showing how to answer it. This used to print the
            # reason and stop; the structured section format was discoverable only by reading this
            # file. A refusal a user cannot act on is a dead end however accurate it is.
            print(f"1. {refusal}? [SOW line 1]", file=sys.stderr)
            print("", file=sys.stderr)
            print("Answer by re-running with the sections filled in:", file=sys.stderr)
            print("", file=sys.stderr)
            for example_line in (
                '  fleet sow --task "add a --version flag',
                "  leaves:",
                "  - print the version and exit 0 | acceptance: --version prints a semver, exits 0",
                "  challenges:",
                "  - may collide with an existing flag | citation: keel/fleet/src/main.rs:40",
                "  alternatives:",
                "  - json-output: emit JSON instead | tradeoff: harder for humans to read",
                "  - build-info: add commit and date | tradeoff: leaks build-host details",
                "  estimates:",
                "  - 30 minutes",
                "  edge cases:",
                '  - --version combined with another flag"',
            ):
                print(example_line, file=sys.stderr)
            print("", file=sys.stderr)
            print(
                "Each challenge needs a real citation (file:line or corpus id), and at least two",
                file=sys.stderr,
            )
            print("named alternatives with tradeoffs -- one option is not a choice.", file=sys.stderr)
        return refusal.exit_code
    print(json.dumps(_sow_payload(sow), sort_keys=True))
    return 0


__all__ = [
    "AtomicLeaf",
    "Alternative",
    "Challenge",
    "Clarification",
    "MachinePredicate",
    "SOWRefusal",
    "Sow",
    "build_sow",
    "main",
    "validate_sow",
]


if __name__ == "__main__":
    raise SystemExit(main())
