"""Capped deterministic phrasers for rare conversational/check-in responses.

BUILD-DIGEST §1 names `ConversationalResponder` and `CheckInPhraser`; §4 boxes both as language
fillers, not decision-makers. This T0 implementation is the deterministic floor used when no model
is available or when a model output violates the length contract.
"""

from __future__ import annotations

from dataclasses import dataclass

MAX_CONVERSATIONAL_SENTENCES = 2
MAX_RESPONSE_CHARS = 220


class PhraseValidationError(ValueError):
    """A phrase crossed the user-facing response contract."""


@dataclass(frozen=True)
class Phrase:
    text: str
    source: str


def _sentence_count(text: str) -> int:
    return sum(1 for c in text if c in ".!?")


def validate_phrase(text: str) -> str:
    cleaned = " ".join(text.strip().split())
    if not cleaned:
        raise PhraseValidationError("phrase must not be empty")
    if len(cleaned) > MAX_RESPONSE_CHARS:
        raise PhraseValidationError("phrase exceeds max response length")
    if _sentence_count(cleaned) > MAX_CONVERSATIONAL_SENTENCES:
        raise PhraseValidationError("phrase exceeds two-sentence cap")
    return cleaned


def conversational_response(user_text: str) -> Phrase:
    """Rare hot-path question/chit-chat response. No model call here; the shell is testable."""
    text = user_text.strip()
    if not text:
        return Phrase("I'm here. Let's keep the next move small.", "deterministic_empty")
    if text.endswith("?"):
        return Phrase("Good question. Let's answer just enough to choose the next action.", "deterministic_question")
    return Phrase("I hear you. Let's come back to the next visible step.", "deterministic_chitchat")


def check_in_phrase(stuck_count: int, step_text: str) -> Phrase:
    """Mostly templated cached audio; repeated stuck gets a gentler shrink prompt."""
    if stuck_count < 0:
        raise PhraseValidationError("stuck_count must be >= 0")
    step = validate_phrase(step_text)
    if stuck_count >= 2:
        return Phrase(f"Let's shrink it. Touch only the first visible part of: {step}", "deterministic_repeated_stuck")
    return Phrase(f"Still with you. The current step is: {step}", "deterministic_check_in")
