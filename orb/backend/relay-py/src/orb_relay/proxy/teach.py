"""Teach-mode beat construction and cost ceiling (acceptance contract Track C: C2, C4, C5).

Pipeline: raw model text -> split into real sentences -> group into <=2-sentence speakable beats ->
classify each beat -> guarantee the turn ends by checking understanding. Sentence splitting uses
`pysbd` (docs/adr/0012-pysbd-for-teach-beat-splitting.md) rather than a hand-rolled regex: a regex on
`.`/`!`/`?` breaks on "Dr.", "e.g.", "U.S.", "3.14", ellipses, and quoted speech — exactly what a
real teach explanation is full of. This module also owns teach mode's explicit token ceiling and the
budget-aware decision of whether a turn can afford it (C2) — the model/tool fills language and
sentence content only; this code owns the structural guarantees, matching the "boxing" pattern the
rest of this product uses (see proxy/atomizer.py, proxy/phrasers.py).
"""

from __future__ import annotations

from dataclasses import dataclass

import pysbd

from ..cost.meter import Rates, SessionMeter, UsageDelta
from .phrasers import validate_phrase
from .schemas import Beat, BeatKind

# Contract C2: teach mode must NOT be clamped to the focus/converse cap (app.py's
# _FOCUS_CONVERSE_MAX_TOKENS = 180 output tokens, matching phrasers.py's 2-sentence/220-char
# deterministic floor) — but it still needs its OWN explicit ceiling, or a long teach reply silently
# voids the blueprint's ~7%-of-cost LLM assumption (docs/adr/0002-llm-provider.md re-derives that
# line for Haiku pricing).
#
# Sized off the same Rates used everywhere in this backend (rates_from_env's Haiku 4.5 defaults,
# paise_per_1k_llm_tokens_out=48): a teach turn at this ceiling costs ceil(480 * 48 / 1000) = 24
# paise in the worst case (output only), versus 9 paise for a focus/converse turn at 180 tokens —
# about 2.67x, not open-ended. Against the ~Rs4 (400 paise) DEFAULT_SESSION_RESERVATION_PAISE, that
# is still >= 16 max-ceiling teach turns in one session even in the worst case where every turn maxes
# the ceiling — comfortably above the >=3-turn grounding bar (contract C3).
TEACH_MAX_TOKENS = 480

# A beat is capped at this many sentences (contract C4: "~<=2 sentences per beat").
MAX_SENTENCES_PER_BEAT = 2

_segmenter = pysbd.Segmenter(language="en", clean=False)

# Deterministic floor phrase appended when the model's own reply doesn't already end by checking
# understanding or offering the next part (contract C5). Mirrors phrasers.py's existing pattern: a
# validated, capped, deterministic filler as the floor under LLM output, not a fresh convention.
_DEFAULT_CHECK_IN_BEAT = validate_phrase(
    "Want me to keep going, or should I slow down on that last part?"
)

# Heuristic phrase set for classifying a beat that offers to continue without literally ending in
# "?" (a model may phrase the offer as a statement, e.g. "Let me know and I'll continue.").
_CHECK_OFFER_PHRASES = (
    "want me to continue",
    "want me to keep going",
    "should i continue",
    "should i keep going",
    "should i go on",
    "want the next part",
    "ready for the next part",
    "shall we continue",
    "let me know if you want",
    "let me know and i'll continue",
    "does that make sense",
    "make sense so far",
)


def classify_beat_kind(beat_text: str) -> BeatKind:
    """CHECK when the beat asks a question or explicitly offers to continue; EXPLAIN otherwise."""
    normalized = " ".join(beat_text.strip().lower().split())
    if normalized.endswith("?"):
        return BeatKind.CHECK
    if any(phrase in normalized for phrase in _CHECK_OFFER_PHRASES):
        return BeatKind.CHECK
    return BeatKind.EXPLAIN


def split_into_beats(text: str, *, max_sentences_per_beat: int = MAX_SENTENCES_PER_BEAT) -> list[str]:
    """Group real sentences (pysbd-detected) into <=`max_sentences_per_beat`-sentence spoken beats."""
    cleaned = text.strip()
    if not cleaned:
        return []
    sentences = [s.strip() for s in _segmenter.segment(cleaned) if s.strip()]
    if not sentences:
        return [cleaned]
    return [
        " ".join(sentences[i : i + max_sentences_per_beat])
        for i in range(0, len(sentences), max_sentences_per_beat)
    ]


def build_beats(text: str, *, enforce_understanding_check: bool) -> list[Beat]:
    """The full beat pipeline for one reply.

    `enforce_understanding_check` is True only for teach-mode replies (contract C5): if the model's
    own last beat doesn't already check understanding or offer the next part, a deterministic beat
    is appended so the guarantee is a code-level invariant, not something that depends on the model
    remembering its instructions on any given turn.
    """
    raw_beats = split_into_beats(text)
    if not raw_beats:
        raw_beats = [text.strip() or _DEFAULT_CHECK_IN_BEAT]
    if enforce_understanding_check and classify_beat_kind(raw_beats[-1]) is not BeatKind.CHECK:
        raw_beats.append(_DEFAULT_CHECK_IN_BEAT)
    last_index = len(raw_beats) - 1
    return [
        Beat(index=i, text=beat_text, kind=classify_beat_kind(beat_text), is_final=(i == last_index))
        for i, beat_text in enumerate(raw_beats)
    ]


@dataclass(frozen=True)
class TeachCeilingDecision:
    max_tokens: int
    degraded: bool
    degrade_reason: str | None


def decide_teach_ceiling(
    meter: SessionMeter, *, rates: Rates, reduced_max_tokens: int
) -> TeachCeilingDecision:
    """Pick this turn's max_tokens ceiling from the session's remaining budget (contract C2).

    Reuses the existing reservation/pricing primitives (cost/meter.py) instead of a parallel
    accounting path. Deliberately does NOT estimate input tokens (e.g. chars/4) — an owner
    directive on this track rejected that as "an estimated ceiling is not a ceiling." The real
    ceiling is the provider's own hard `max_tokens` parameter passed to the gateway, which bounds
    *output* tokens exactly by construction (the provider enforces it server-side — the same
    mechanism proxy/atomizer.py's ATOMIZER_MAX_TOKENS already relies on), so the worst-case OUTPUT
    cost at that ceiling is exact, not estimated, and is what this function checks against the
    remaining budget. Real input-token cost is metered exactly from the gateway's actual reported
    usage via `meter.settle()` after the call returns, identically to every other route in this
    backend today — none of them pre-estimate input tokens either. A turn optimistically admitted
    here whose real total cost (input + output) exceeds what is left still fails closed at
    settlement (`ReservationExceededError` -> HTTP 402); it never silently overspends.

    A real pre-call token count (Anthropic/Gemini's own counting APIs) was not wired in: the
    `GatewayClient` seam (proxy/gateway_client.py) exposes only `complete()`, and adding a
    `count_tokens` call would mean extending `backend/gateway-sidecar` (TypeScript), which is
    outside this track's file scope. If that seam is added later, this function is the one call
    site to swap the estimate for a real count.
    """
    worst_case_output_only = UsageDelta(llm_tokens_out=TEACH_MAX_TOKENS)
    if not meter.would_exceed(worst_case_output_only, rates):
        return TeachCeilingDecision(TEACH_MAX_TOKENS, False, None)
    return TeachCeilingDecision(
        reduced_max_tokens,
        True,
        f"session_budget_low: reduced ceiling from {TEACH_MAX_TOKENS} to {reduced_max_tokens} "
        "output tokens",
    )
