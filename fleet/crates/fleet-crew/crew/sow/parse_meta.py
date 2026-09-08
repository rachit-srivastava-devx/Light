"""Challenge and clarification parsing."""

from __future__ import annotations

import re
from collections.abc import Sequence

from .citations import CORPUS_ID, FILE_CITATION
from .errors import SOWRefusal
from .model import Challenge, Clarification
from .parse_meta_ext import _is_generic_question
from .parse_sections import _strip_bullet

_TRACE = re.compile(r"(?:\[\s*)?(?:sow\s*)?line\s*[:=]?\s*(?P<line>\d+)", re.IGNORECASE)


def _find_citations(text: str) -> list[str]:
    citations = [match.group(0) for match in FILE_CITATION.finditer(text)]
    citations.extend(
        token for token in re.findall(r"\b[A-Z]{1,3}\d+\b", text) if CORPUS_ID.fullmatch(token)
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
        question = re.sub(r"^(?:business|technical)\s*:\s*", "", question, flags=re.IGNORECASE)
        category = "business" if re.search(r"\bbusiness\b", text, re.IGNORECASE) else "technical"
        if _is_generic_question(question):
            raise SOWRefusal(f"generic clarification refused: {question!r}")
        clarifications.append(Clarification(question, int(trace.group("line")), category))
    return tuple(clarifications)
