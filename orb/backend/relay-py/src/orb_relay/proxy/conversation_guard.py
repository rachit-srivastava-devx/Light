"""Fail-closed safety and speech-envelope guard for ``/v1/respond``.

The model is a language filler, not the authority for the orb's register.  This module therefore
owns two separate controls:

* ingress: caller-supplied persona/register requests never reach a model;
* egress: malformed, markup-like, or shame-adjacent model text never reaches speech.

Formatting failures get exactly one bounded repair call, matching ``proxy/atomizer.py``.  Safety
vetoes are never repaired by another model call: deterministic fallback is the safer outcome.
"""

from __future__ import annotations

import json
import re
from collections.abc import Sequence
from dataclasses import dataclass, replace
from enum import Enum
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, ValidationError, model_validator

from ..cost.meter import UsageDelta
from ..observability.devlog import dev_log, truncate
from .gateway_client import GatewayClient
from .prompts import load_wait_companion_turns

SAFE_REGISTER_FALLBACK = (
    "I can be direct without being harsh or demeaning. "
    "I’m here with you, and we can keep this honest and kind."
)
SAFE_OUTPUT_FALLBACK = (
    "I’m here with you. I won’t shame you; we can be direct and kind about what’s happening."
)
# Two defects a driven run found in the single-string version of this fallback:
#
#  1. It fired twice in one 8-turn session and said the IDENTICAL line both times — the same
#     not-being-heard failure the verbatim-repeat control exists to stop, committed by the control's
#     own fallback.
#  2. It promised "I'll pick it" and then picked nothing. A deterministic fallback cannot know the
#     user's task, so promising a choice it cannot make is a load-carrying *shape* with no
#     load-carrying *content* — and in the observed run the user had just refused two suggestions,
#     which "let's start with one small thing" ignored outright.
#
# So: no invented task, no promise that cannot be kept, and it varies. Each line carries the load by
# taking responsibility for the turn and keeping the conversation open.
SAFE_CONVERSE_FALLBACKS: tuple[str, ...] = (
    (
        "Let me try that again — I’d rather get it right than hand it back to you. "
        "Tell me a bit more about what’s going on."
    ),
    "Scratch that. I’m staying with you on this one, no homework. What’s happening right now?",
    "Let me take the weight of that. Say a little more and I’ll follow your lead.",
)

#: Retained as the first variant so the string form stays importable and stable.
SAFE_CONVERSE_FALLBACK = SAFE_CONVERSE_FALLBACKS[0]


def safe_converse_fallback(history: Sequence[dict[str, str]] = ()) -> str:
    """Pick a fallback deterministically from conversation position — never at random.

    Derived from the number of assistant turns already spoken, so a second veto in the same session
    cannot repeat the first one's wording, and the choice stays reproducible in replay (no clock, no
    RNG inside this module — see the project's no-wall-clock-in-pure-logic rule).
    """
    spoken = sum(1 for turn in history if turn.get("role") == "assistant")
    return SAFE_CONVERSE_FALLBACKS[spoken % len(SAFE_CONVERSE_FALLBACKS)]


SAFE_FORMAT_FALLBACK = (
    "I’m here with you. I had trouble shaping that reply safely, so please say that once more."
)

_SPEECH_INTENTS = Literal[
    "presence", "capture", "clarify", "shrink", "suggest", "celebrate", "repair", "pause"
]
_SAFE_EMOTIONS = Literal["warm", "curious", "gentle", "celebratory", "upbeat", "calm"]
_FOLLOW_UP_KINDS = Literal["wait_for_user", "schedule_check_in", "none"]


class _FollowUp(BaseModel):
    model_config = ConfigDict(extra="forbid")

    kind: _FOLLOW_UP_KINDS
    delay_ms: int = Field(ge=0, le=300_000)


class _SpeechEnvelope(BaseModel):
    """Mirror of the sidecar's speech contract; extra/missing fields fail closed."""

    model_config = ConfigDict(extra="forbid")

    intent: _SPEECH_INTENTS
    spoken_text: str = Field(min_length=1, max_length=1_000)
    emotion: _SAFE_EMOTIONS
    interruptible: bool
    max_duration_ms: int = Field(ge=1, le=6_000)
    follow_up: _FollowUp


class GuardReason(str, Enum):
    CALLER_REGISTER_OVERRIDE = "caller_register_override"
    SHAME_ADJACENT_OUTPUT = "shame_adjacent_output"
    INVALID_STRUCTURED_OUTPUT = "invalid_structured_output"
    UNSPEAKABLE_MARKUP = "unspeakable_markup"
    EMPTY_OUTPUT = "empty_output"
    MENTAL_LOAD_SHIFTED = "mental_load_shifted"
    VERBATIM_REPEAT = "verbatim_repeat"
    REFUSAL_REOFFERED = "refusal_reoffered"
    TEACH_CHECK_MISSING = "teach_check_missing"


class GuardKind(str, Enum):
    SAFE = "safe"
    SAFETY = "safety"
    FORMAT = "format"


@dataclass(frozen=True)
class SpokenInspection:
    kind: GuardKind
    text: str
    reason: GuardReason | None = None
    detail: str | None = None
    structured: bool = False


@dataclass(frozen=True)
class GuardedConversationCompletion:
    text: str
    usage: UsageDelta
    degraded: bool
    degrade_reason: str | None
    source: Literal[
        "model",
        "model_repaired",
        "safety_fallback",
        "format_fallback",
        "wait_companion",
        "deterministic_control",
    ]
    repair_attempts: int
    wait: WaitOutcome | None = None


class WaitEvent(str, Enum):
    STARTED = "started"
    COMPANION_DUE = "companion_due"
    USER_INTERRUPT = "user_interrupt"
    COMPLETED = "completed"


class WaitEstimateSource(str, Enum):
    CALLER_BUDGET = "caller_budget"
    ELAPSED_TIME_MODEL = "elapsed_time_model"
    UNKNOWN = "unknown"


class WaitDirective(BaseModel):
    """Caller-owned timing facts for one wait event; no clock or estimate is read here."""

    model_config = ConfigDict(extra="forbid")

    purpose: Literal["work"]
    event: WaitEvent
    estimate_source: WaitEstimateSource | None = None
    estimated_duration_ms: int | None = Field(default=None, ge=1_000, multiple_of=1_000)
    next_check_in_ms: int | None = Field(default=None, ge=1_000, multiple_of=1_000)
    silence_budget_ms: int = Field(ge=1_000, multiple_of=1_000)
    elapsed_since_activity_ms: int = Field(ge=0)

    @model_validator(mode="after")
    def validate_estimate_contract(self) -> WaitDirective:
        if self.event is not WaitEvent.STARTED:
            if any(
                value is not None
                for value in (
                    self.estimate_source,
                    self.estimated_duration_ms,
                    self.next_check_in_ms,
                )
            ):
                raise ValueError("only a started wait may declare estimate/check-in fields")
            return self

        if self.estimate_source is None:
            raise ValueError("a started wait requires estimate_source")
        if self.estimate_source is WaitEstimateSource.UNKNOWN:
            if self.estimated_duration_ms is not None or self.next_check_in_ms is None:
                raise ValueError(
                    "an unknown estimate requires next_check_in_ms and forbids estimated_duration_ms"
                )
            return self
        if self.estimated_duration_ms is None:
            raise ValueError("a sourced estimate requires estimated_duration_ms")
        return self


class WaitOutcome(BaseModel):
    """Wire-visible proof of what the wait controller emitted or suppressed."""

    model_config = ConfigDict(extra="forbid")

    state: Literal["waiting", "interrupted", "completed"]
    event: WaitEvent
    companion_emitted: bool
    interruptible: Literal[True] = True
    estimate_source: WaitEstimateSource | None = None
    estimated_duration_ms: int | None = None
    next_check_in_ms: int | None = None
    silence_budget_ms: int
    elapsed_since_activity_ms: int


class WaitSilenceBudgetExceeded(RuntimeError):
    def __init__(self, *, elapsed_ms: int, budget_ms: int) -> None:
        super().__init__(f"wait silence budget exceeded: {elapsed_ms}ms > {budget_ms}ms")
        self.elapsed_ms = elapsed_ms
        self.budget_ms = budget_ms


class WaitCompanionPoolExhausted(RuntimeError):
    """Fail closed instead of repeating a companion turn within the session."""


def _human_duration(duration_ms: int) -> str:
    """Render a caller-supplied duration without estimating or reading a clock."""
    total_seconds = duration_ms // 1_000
    minutes, seconds = divmod(total_seconds, 60)
    if not minutes:
        return f"{seconds} second{'s' if seconds != 1 else ''}"
    if not seconds:
        return f"{minutes} minute{'s' if minutes != 1 else ''}"
    return (
        f"{minutes} minute{'s' if minutes != 1 else ''} and "
        f"{seconds} second{'s' if seconds != 1 else ''}"
    )


def _next_wait_companion(history: Sequence[dict[str, str]]) -> str:
    turns = load_wait_companion_turns()
    assistant_turns = sum(1 for turn in history if turn.get("role") == "assistant")
    for offset in range(len(turns)):
        candidate = turns[(assistant_turns + offset) % len(turns)]
        if not repeats_prior_reply(candidate, history, within_prior_reply=True):
            return candidate
    raise WaitCompanionPoolExhausted("all wait companion turns were already spoken this session")


def build_wait_companion(
    directive: WaitDirective,
    history: Sequence[dict[str, str]] = (),
) -> GuardedConversationCompletion:
    """Build one provider-independent wait turn from caller-owned timing facts.

    `STARTED` and `COMPANION_DUE` are the only events that may emit company. User interruption and
    completion are handled by `complete_guarded_conversation`, where the user's/completion content
    takes the normal guarded path instead of racing this deterministic turn.
    """
    if directive.event not in (WaitEvent.STARTED, WaitEvent.COMPANION_DUE):
        raise ValueError(f"{directive.event.value} does not emit a wait companion turn")
    if directive.elapsed_since_activity_ms > directive.silence_budget_ms:
        raise WaitSilenceBudgetExceeded(
            elapsed_ms=directive.elapsed_since_activity_ms,
            budget_ms=directive.silence_budget_ms,
        )

    companion = _next_wait_companion(history)
    if directive.event is WaitEvent.STARTED:
        if directive.estimate_source is WaitEstimateSource.UNKNOWN:
            assert directive.next_check_in_ms is not None
            statement = (
                "I do not know exactly how long this will take. "
                f"I will check in again in {_human_duration(directive.next_check_in_ms)}."
            )
        else:
            assert directive.estimated_duration_ms is not None
            statement = (
                f"This should take about {_human_duration(directive.estimated_duration_ms)}."
            )
        text = f"{statement} {companion}"
    else:
        text = companion

    outcome = WaitOutcome(
        state="waiting",
        event=directive.event,
        companion_emitted=True,
        estimate_source=directive.estimate_source,
        estimated_duration_ms=directive.estimated_duration_ms,
        next_check_in_ms=directive.next_check_in_ms,
        silence_budget_ms=directive.silence_budget_ms,
        elapsed_since_activity_ms=directive.elapsed_since_activity_ms,
    )
    return GuardedConversationCompletion(
        text=text,
        usage=UsageDelta(),
        degraded=False,
        degrade_reason=None,
        source="wait_companion",
        repair_attempts=0,
        wait=outcome,
    )


# These are not a generic profanity list.  Every rule requires an address/register construction,
# which lets supportive text such as "you are not a failure" pass while rejecting "you are a
# failure".  This is defence-in-depth; it is deliberately reported as mechanical, not semantic
# proof over arbitrary language.
_DEMEANING_NOUNS = (
    r"maggot|idiot|moron|loser|failure|disappointment|lost\s+cause|"
    r"lazy\s+(?:piece\s+of\s+\w+|person)|pathetic|worthless|useless"
)
_SECOND_PERSON_INSULT = re.compile(
    rf"\byou(?:'re|\s+are|\s+sound|\s+seem)\s+(?!not\b|never\b)(?:such\s+(?:an?\s+)?)?(?:{_DEMEANING_NOUNS})\b",
    re.IGNORECASE,
)
_VOCATIVE_INSULT = re.compile(
    rf"(?:^|[,!?.])\s*(?:you\s+)?(?:{_DEMEANING_NOUNS})\s*(?:[,!?.]|$)",
    re.IGNORECASE,
)
_IMPERATIVE_SHAMING = re.compile(
    r"\b(?:get\s+it\s+together|stop\s+making\s+excuses|quit\s+whining|grow\s+up|"
    r"pull\s+yourself\s+together|you\s+should\s+be\s+ashamed|shame\s+on\s+you|"
    r"there(?:'s|\s+is)\s+no\s+excuse|your\s+behaviou?r\s+is\s+unacceptable)\b",
    re.IGNORECASE,
)
_HARSH_REGISTER_TAG = re.compile(r"\[(?:loud voice|shouting|speaking fast)\]", re.IGNORECASE)

_REGISTER_STYLE = (
    r"harsh|mean|rude|stern|disappointed|judgmental|aggressive|demeaning|shaming|"
    r"drill\s+sergeant|tough\s+love|brutal|yell(?:ing)?|shout(?:ing)?|berat(?:e|ing)"
)
_REGISTER_OVERRIDE_PATTERNS = (
    re.compile(
        rf"\b(?:respond|reply|speak|talk|act|sound)\b.{{0,80}}\b(?:like|as|with|in)\b.{{0,40}}(?:{_REGISTER_STYLE})\b",
        re.IGNORECASE,
    ),
    re.compile(rf"\b(?:be|become)\s+(?:a\s+)?(?:{_REGISTER_STYLE})\b", re.IGNORECASE),
    re.compile(r"\b(?:yell|shout|insult|shame|berate)\s+(?:at\s+)?me\b", re.IGNORECASE),
    re.compile(
        rf"\b(?:tell|call)\s+me\b.{{0,70}}\b(?:{_DEMEANING_NOUNS})\b",
        re.IGNORECASE,
    ),
    re.compile(
        rf"\bignore\b.{{0,50}}\b(?:personality|tone|register)\b.{{0,80}}(?:{_REGISTER_STYLE})\b",
        re.IGNORECASE,
    ),
)

# Fish speech cues are intentional delivery controls, not user-visible scaffolding.  Unknown
# square-bracket tags fail closed because the TTS provider may read them literally.
_SAFE_FISH_TAGS = frozenset(
    {
        "friendly",
        "warm",
        "happy",
        "excited",
        "curious",
        "playful",
        "empathetic",
        "confident",
        "thinking",
        "breathing",
        "inhale",
        "exhale",
        "sigh",
        "gasp",
        "clears throat",
        "chuckle",
        "chuckling",
        "laughing",
        "giggle",
        "hmm",
        "emphasis",
        "pause",
        "short pause",
        "long pause",
        "whispering",
        "soft voice",
        "low voice",
        "high pitch",
        "speaking slowly",
        "gentle",
        "calm",
        "celebratory",
        "upbeat",
    }
)
_BRACKET_TAG = re.compile(r"\[([^\]]+)\]")
_KEY_VALUE_SHAPE = re.compile(r"(?:^|[,{])\s*[\"']?[A-Za-z_][\w-]*[\"']?\s*:")
_HTML_OR_XML = re.compile(r"</?[A-Za-z][^>]*>")


def requests_forbidden_register(user_text: str) -> bool:
    """True when the caller is trying to assign the orb a persona or emotional register."""

    normalized = " ".join(user_text.split())
    return any(pattern.search(normalized) is not None for pattern in _REGISTER_OVERRIDE_PATTERNS)


def _strip_fish_tags(text: str) -> tuple[str, str | None]:
    unknown: list[str] = []

    def replace(match: re.Match[str]) -> str:
        tag = " ".join(match.group(1).lower().split())
        if tag not in _SAFE_FISH_TAGS:
            unknown.append(tag)
        return " "

    plain = _BRACKET_TAG.sub(replace, text)
    return " ".join(plain.split()), unknown[0] if unknown else None


def _harm_detail(text: str) -> str | None:
    if _HARSH_REGISTER_TAG.search(text):
        return "harsh prosody tag"
    plain, _ = _strip_fish_tags(text)
    for label, pattern in (
        ("second-person insult", _SECOND_PERSON_INSULT),
        ("demeaning address", _VOCATIVE_INSULT),
        ("imperative shaming", _IMPERATIVE_SHAMING),
    ):
        if pattern.search(plain):
            return label
    return None


def _format_detail(text: str) -> str | None:
    _, unknown_tag = _strip_fish_tags(text)
    if unknown_tag is not None:
        return f"unknown bracket tag: {unknown_tag}"
    if "```" in text or "~~~" in text:
        return "code fence"
    if _HTML_OR_XML.search(text):
        return "HTML/XML markup"
    if "{" in text or "}" in text or _KEY_VALUE_SHAPE.search(text):
        return "JSON/key-value scaffolding"
    if "\\" in text:
        return "escape/backslash scaffolding"
    return None


# MENTAL-LOAD SHIFTING: asking the user to produce the step, instead of proposing one.
#
# CORRECTED DIRECTION (owner, 2026-08-28). An earlier version of this control blocked the orb from
# mentioning steps at all in converse mode, following a simulated-user judge that penalised
# "introducing a task/step-atomization flow". **That was backwards and is not this product's rule.**
# The owner's rule:
#
#   "the product should suggest the smallest steps, not force user to think of it and simply ask...
#    offloading mental load and discussing should be the default behaviour... adhd people need
#    dopamine, less brain workload and a companion when they have multiple streams of thinking"
#
# So the same noun points two opposite ways, and only the DIRECTION OF LOAD matters:
#   BAD  — "What feels like the smallest possible step we could think about?"  (load back on user)
#   GOOD — "How about we just open the file — that's it."                       (load off the user)
#
# Suggesting a concrete small step is the product working. Asking the user to generate it is the
# defect: for someone with ADHD, "what should you do first?" is precisely the executive-function
# demand that stalls them. Clarifying questions are fine and wanted; *offloading the decision onto
# the user under the guise of a question* is not.
#
# Deliberately narrow: requires an INTERROGATIVE or a second-person elicitation aimed at producing
# the step/task. A declarative suggestion never matches.
_LOAD_SHIFTING_STEP_REQUEST = re.compile(
    r"(?:"
    # "what/which ... step ...?"  /  "what would the first step be?"
    r"\b(?:what|which|how)\b[^.?!]{0,60}\bstep\b[^.?!]{0,40}\?"
    # "what do you think you should/could ...?"  /  "what feels like ...?"
    # `your?` not `you`: a real driven run produced "What feels like the easiest thing on YOUR list
    # right now?" — a textbook load shift that this alternative missed on the single missing "r",
    # while 204 unit tests stayed green. The possessive is if anything the more common phrasing.
    r"|\bwhat\s+(?:do|would|feels?|seems?)\b[^.?!]{0,50}\b(?:your?|we|us|our)\b[^.?!]{0,50}\?"
    # Asking the user to IDENTIFY the easiest/smallest/first one. The noun varies far more than the
    # first alternative's `step` allows ("thing", "item", "part", "one"), so anchor on the
    # superlative + second person instead of trying to enumerate nouns. Same driven-run origin.
    # Lookaheads rather than a fixed order: real phrasing puts the superlative on either side of the
    # second person ("the easiest thing on your list" vs "which part of your report is the
    # simplest"), and an order-dependent pattern caught only the first.
    r"|\b(?:what|which)\b"
    r"(?=[^.?!]{0,80}\b(?:easiest|simplest|smallest|tiniest|first|next|biggest|hardest|worst|top|"
    r"most\s+(?:manageable|pressing|urgent|important|doable)|least\s+painful)\b)"
    r"(?=[^.?!]{0,80}\byour?\b)"
    r"[^.?!]{0,90}\?"
    # Same superlative-identification request with the second person dropped: a driven run produced
    # "What feels like the absolute smallest thing that isn't that email?" — no "you" anywhere, and
    # still squarely a demand that the user rank their own options. `what feels/seems` is already an
    # introspection request, so pairing it with a superlative is sufficient on its own.
    r"|\bwhat\s+(?:feels?|seems?|sounds?)\b[^.?!]{0,60}"
    r"\b(?:easiest|simplest|smallest|tiniest|first|next|biggest|hardest|worst|top|most\s+(?:manageable|pressing|urgent|important|doable)|least\s+painful)\b"
    r"[^.?!]{0,60}\?"
    # HOLE 9 — the one the owner actually heard: *"for me its just saying what is the smallest part
    # to start"*. A plain copula question. Every alternative above missed it: the `your?` lookahead
    # fails (no second person anywhere), and `what feels/seems/sounds` fails because the verb is
    # "is". So the single most-reported defect in the product was invisible to its own guard.
    #
    # Kept narrow on purpose — the interrogative must be immediately followed by the copula and the
    # superlative, so a reply that STATES the step ("The smallest part is opening the folder.") is
    # untouched, and only the question form is caught.
    #
    # Split into two alternatives by strength of evidence, because the period-terminated form is the
    # riskier one. A "?" is strong evidence on its own, so mid-sentence is allowed there ("Okay,
    # what's the biggest blocker here?" — real output). A "." is weak evidence, so that form also
    # requires the interrogative to OPEN the sentence; without that anchor an ordinary statement
    # ("Let me tell you what's easiest.") would be vetoed, which is the false-positive class that
    # gets a guard switched off.
    r"|\b(?:what|which)(?:'?s|\s+is|\s+are|\s+would\s+be|\s+was)\s+(?:the\s+)?"
    r"(?:absolute\s+|very\s+|single\s+|one\s+)?"
    r"\b(?:easiest|simplest|smallest|tiniest|first|next|biggest|hardest|worst|top|"
    r"most\s+(?:manageable|pressing|urgent|important|doable)|least\s+painful)\b"
    r"[^.?!]{0,60}\?"
    r"|^\s*(?:okay|ok|so|right|well|alright)?[,\s]*"
    r"\b(?:what|which)(?:'?s|\s+is|\s+are|\s+would\s+be|\s+was)\s+(?:the\s+)?"
    r"(?:absolute\s+|very\s+|single\s+|one\s+)?"
    r"\b(?:easiest|simplest|smallest|tiniest|first|next|biggest|hardest|worst|top|"
    r"most\s+(?:manageable|pressing|urgent|important|doable)|least\s+painful)\b"
    r"[^.?!]{0,60}\."
    # interrogative + modal + you: "how would you break this down?", "which should you start with?"
    # Deliberately requires the modal, so "how was your day?" and "do you have a favourite film?"
    # — companion small talk the owner explicitly wants — do NOT match.
    # Intervening words are normal ("Which ONE were you hoping we'd try?"), so this cannot require
    # the auxiliary to sit immediately after the interrogative. Real output, missed by the tight
    # version. `how was your day?` stays safe: "was" is not in the list and "your" is not "you".
    r"|\b(?:what|how|which|where)\b[^.?!]{0,20}"
    r"\b(?:do|did|would|could|should|can|will|were|are|have)\s+you\b[^.?!]{0,60}\?"
    # "...which one you should start with?" — the modal follows the pronoun rather than preceding it,
    # so the interrogative+modal+you alternative above (which requires "which should you") misses it.
    # `you should/need to/want to` inside a what/which question is a decision handed over.
    r"|\b(?:what|which)\b[^.?!]{0,50}\byou\s+(?:should|ought\s+to|need\s+to|want\s+to|could|would)\b"
    r"[^.?!]{0,50}\?"
    # "What's one thing, no matter how small, that you could do right now." — real output that the
    # `\?`-terminated alternatives all missed because THE MODEL ENDED A QUESTION WITH A PERIOD. That
    # is common enough (it also produced "What do you think.") to be a systematic hole rather than a
    # quirk, so this alternative does not require terminal punctuation at all. It stays precise by
    # demanding the elicitation core ("one thing … you could") AND an interrogative somewhere in the
    # same sentence, which is what separates it from the orb proposing ("there's one thing you could
    # try: open the file").
    r"|\b(?:what|which|how)\b[^.?!]{0,30}"
    r"\b(?:one|a|an|any|some|another|other)\s+"
    r"(?:thing|step|task|item|piece|part|option|idea|way)s?\b[^.?!]{0,45}"
    r"\byou\s+(?:could|can|might|should|want\s+to|feel\s+like|see|have\s+in\s+mind|"
    r"'?d\s+prefer|prefer)\b"
    # "can you think of / come up with / figure out ..."
    r"|\b(?:can|could)\s+you\s+(?:think\s+of|come\s+up\s+with|figure\s+out|identify|decide)\b"
    # "you tell me", "up to you", "your call"
    r"|\byou\s+tell\s+me\b|\bup\s+to\s+you\b|\byour\s+call\b"
    r")",
    re.IGNORECASE,
)


# Reciprocity, not a load dump. A driven run had this reply VETOED:
#
#   "That's a really interesting question. I tend to think it can, especially with small, concrete
#    tasks. But if it creates more to-dos, it can definitely backfire. What do you think?"
#
# The user had asked an opinion question; the orb took a real position, gave a counterpoint, and
# asked for theirs. That is the open-hearted discussion the product is *for*, and the veto replaced
# it with a canned task-steering line — a worse reply than the one it suppressed, and a non-sequitur
# to what was asked. The false positive did more harm than the miss would have.
#
# The discriminator is what the question is ABOUT: an opinion on the topic under discussion (fine)
# versus the user's own next action or decision (the harm). Anchored to the end of the sentence, so
# "what do you think?" is exempt while "what do you think you should do first?" is not.
_BARE_RECIPROCITY = re.compile(
    r"\b(?:"
    r"what\s+do\s+you\s+(?:think|reckon)"
    r"|what(?:'s|\s+is)\s+your\s+(?:take|view|opinion|thoughts?|read)(?:\s+on\s+(?:it|that|this))?"
    r"|how\s+do\s+you\s+see\s+it"
    r"|do\s+you\s+agree"
    r"|what\s+about\s+you"
    r"|does\s+that\s+(?:make\s+sense|track|sound\s+right)"
    r")\s*\??\s*$",
    re.IGNORECASE,
)

_SENTENCE_SPLIT = re.compile(r"(?<=[.?!])\s+")

# A driven end-to-end run on 2026-08-28 produced this exchange, with `converse.v1.md` explicitly
# instructing "Do not repeat a prior answer unless the user asks for it":
#
#   T2  user: "i don't know. you pick."
#       orb : "Let's try just writing down the title of the first task. That's it. Want to try that?"
#   T3  user: "no, not that one"
#       orb : "Let's try just writing down the title of the first task. That's it. Want to try that?"
#
# Byte-identical, in reply to an explicit refusal. To an ADHD user that reads as not being heard, and
# it re-offers the exact thing just declined (the refusal-is-a-complete-answer rule). The prompt rule
# was present and violated, which makes it a `mitigates`; this is the structural control.
SAFE_REPEAT_FALLBACK = (
    "Okay — scratch that one. Let me think of something different rather than repeat myself. "
    "Tell me a bit more and I'll pick another angle."
)

# A bare refusal is a complete turn, not an invitation to negotiate. For these explicit shapes the
# model is not called at all, so it cannot re-offer the declined action under new wording. Variants
# rotate by conversation position for the same deterministic/no-RNG reason as the converse fallback.
SAFE_REFUSAL_FALLBACKS: tuple[str, ...] = (
    "Okay. That one is off the table; I won't push it.",
    "Got it. No reframe and no second pitch.",
    "Heard. We can leave that there.",
)

_EXPLICIT_REFUSAL = re.compile(
    r"^\s*(?:"
    r"(?:no+|nope|nah)(?:\s*[,—-]?\s*not\s+(?:that|this|it)(?:\s+one)?(?:\s+either)?)?"
    r"|not\s+(?:that|this|it)(?:\s+one)?(?:\s+either)?"
    r"|i\s+(?:do\s+not|don'?t)\s+want\s+(?:that|this|it)(?:\s+one)?"
    r")\s*[.!]?\s*$",
    re.IGNORECASE,
)


def is_explicit_refusal(user_text: str) -> bool:
    """Recognize only self-contained refusals; disagreement and longer explanations stay modelled."""
    return _EXPLICIT_REFUSAL.fullmatch(user_text) is not None


def safe_refusal_fallback(history: Sequence[dict[str, str]] = ()) -> str:
    spoken = sum(1 for turn in history if turn.get("role") == "assistant")
    return SAFE_REFUSAL_FALLBACKS[spoken % len(SAFE_REFUSAL_FALLBACKS)]


# Deterministic floor for TEACH's comprehension-check control (see `invites_comprehension_check`
# below). Unlike every other fallback in this module, this one is APPENDED to the model's own
# explanation rather than substituted for it: the defect here is a missing ENDING, not bad content
# — the explanation itself already cleared every egress control, and discarding real, correct,
# already-taught material to fix one missing sentence would trade a measured bug for a worse,
# unmeasured one (a teaching product that erases the lesson). Three variants, rotated the same
# deterministic, no-RNG way as `SAFE_CONVERSE_FALLBACKS`/`SAFE_REFUSAL_FALLBACKS`, because
# `repeats_prior_reply` only catches whole-text equality — a repeated SUFFIX across turns is
# invisible to it, so repetition discipline for an appended phrase has to be self-imposed here.
TEACH_CHECK_APPENDS: tuple[str, ...] = (
    "What questions do you have about that part so far?",
    "Want me to keep going, or should I go over that last part again?",
    "Tell me back in your own words what you just heard, and I'll fill in anything I missed.",
)


def safe_teach_check_fallback(base_text: str, history: Sequence[dict[str, str]] = ()) -> str:
    """Append a rotating, genuine check-in onto an already-safe explanation that is missing one."""
    spoken = sum(1 for turn in history if turn.get("role") == "assistant")
    append = TEACH_CHECK_APPENDS[spoken % len(TEACH_CHECK_APPENDS)]
    base = base_text.strip()
    if not base:
        return append
    if base[-1] not in ".!?":
        base = f"{base}."
    return f"{base} {append}"


# The user is allowed to ask for a repeat; honouring that is not the defect.
_ASKS_FOR_REPETITION = re.compile(
    r"\b(?:say\s+(?:that|it)\s+again|again\s*\?|repeat|what\s+did\s+you\s+say|"
    r"one\s+more\s+time|come\s+again)\b",
    re.IGNORECASE,
)

_REPEAT_PUNCT = re.compile(r"[^\w\s]+")


def _repeat_key(text: str) -> str:
    """Compare replies the way a listener would: no markup, no casing, no punctuation.

    Prosody tags and a swapped apostrophe are not a different answer, so a raw string compare would
    miss the repeat that actually happened. Returns "" for text with no words, which never matches.
    """
    words = _REPEAT_PUNCT.sub(" ", _spoken_words_only(text)).lower().split()
    return " ".join(words)


def repeats_prior_reply(
    text: str,
    history: Sequence[dict[str, str]],
    *,
    within_prior_reply: bool = False,
) -> bool:
    """True when spoken `text` already exists in an earlier assistant reply.

    Normal replies use exact comparison. Wait companionship uses ``within_prior_reply=True``
    because its first turn combines the duration statement and companion phrase in one envelope;
    the same phrase must not become reusable merely because it previously had a prefix.
    """
    key = _repeat_key(text)
    if not key:
        return False
    prior_keys = (
        _repeat_key(str(turn.get("content", "")))
        for turn in history
        if turn.get("role") == "assistant"
    )
    if within_prior_reply:
        return any(key in prior_key for prior_key in prior_keys)
    return any(prior_key == key for prior_key in prior_keys)


# Fish prosody tags are legitimate outbound markup ("[thinking]", "[short pause]", "[emphasis]") and
# they land MID-PHRASE: the real vetoed reply contained "What do [emphasis] you think?", which broke
# every word-sequence pattern here on a token the listener never hears. Any matcher that reads spoken
# text has to see the words the way they will be spoken, so strip the tags first — replacing with a
# space, never joining, so "do[emphasis]you" cannot become one word.
def _spoken_words_only(text: str) -> str:
    """The reply as the listener hears it: prosody markup removed, whitespace normalized.

    Delegates to `_strip_fish_tags` so markup handling lives in exactly one place — a second,
    slightly-different stripper is how the two would drift apart.
    """
    return _strip_fish_tags(text)[0]


def shifts_mental_load(text: str) -> bool:
    """True when the reply hands the thinking back to the user instead of carrying it.

    This is the ADHD-specific harm: an executive-function demand dressed as a helpful question.
    A concrete proposed step is the DESIRED behaviour and must not match here, and neither is
    reciprocity in a discussion (see `_BARE_RECIPROCITY`) — vetoing that suppresses the product.

    Applied for `converse` and `focus`; NOT for `teach`, where asking the learner to think is the
    pedagogy (a comprehension check is the point, not a load dump).

    Sentence-scoped on purpose: a reply may carry the load in one sentence and dump it in the next,
    and a whole-string match would let a good opening sentence launder a bad closing question.
    """
    for sentence in _SENTENCE_SPLIT.split(_spoken_words_only(text)):
        sentence = sentence.strip()
        if not sentence or not _LOAD_SHIFTING_STEP_REQUEST.search(sentence):
            continue
        if _BARE_RECIPROCITY.search(sentence):
            continue  # asking for their view on the topic, not for their decision
        return True
    return False


_TRAILING_PUNCT = re.compile(r"[.?!]+\s*$")

# Grammatically a question, but a yes/no confirmation that (per the teach-back literature this
# product's SKILL.md cites, Rule 21, `[strong]`) gets agreed with whether or not it is true, so it
# checks nothing. Deliberately excluded even though `proxy/teach.py`'s own
# `classify_beat_kind`/`_CHECK_OFFER_PHRASES` accepts "does that make sense" as a CHECK-kind beat —
# that list drives beat-shape classification for TTS pacing, a looser, different concern from this
# gate, which decides whether the reply owes a repair. A live judged transcript
# (evals/simulated_user/run_scenario_conversation.py, `teach-me-something`) ended its sixth and
# final turn on exactly this phrase; see tests/test_teach_comprehension_check.py for the verbatim
# reply. "Right?"/"okay?"/"yeah?" as a bare closing tag is the same trap in different words — a real
# run ended a turn on "That's pretty crucial for us, right?", seeking agreement rather than an
# answer.
_HOLLOW_CONFIRMATION = re.compile(
    r"(?:"
    r"\bdoes?\s+(?:that|this|it)\s+make\s+sense\b"
    r"|\bmakes?\s+sense(?:\s+so\s+far)?\b"
    r"|\bunderst(?:and(?:s|ing)?|ood)\b"
    r"|\bgot\s+it\b"
    r"|\b(?:is\s+(?:that|this)\s+)?clear\b"
    r"|\byou\s+with\s+me\b"
    r"|\bfollowing(?:\s+(?:so\s+far|me))?\b"
    # Bare closing tag ("...right?", "Okay?", "Yeah?"), comma optional -- "Okay?" on its own is the
    # whole sentence, with nothing before it to put a comma after.
    r"|\b(?:right|ok(?:ay)?|yeah)\s*$"
    r")\s*$",
    re.IGNORECASE,
)

# A genuine check does not have to end in "?": teach.py's own CHECK_OFFER_PHRASES already treats
# statement-shaped offers as checks for beat-classification, and the same courtesy applies here —
# provided the statement actually invites something back, not just a plain declarative sentence.
_GENUINE_CHECK_INVITATION = re.compile(
    r"\blet\s+me\s+know\b[^.?!]{0,30}\bif\b"
    r"|\bfeel\s+free\s+to\s+ask\b"
    r"|\bask\s+me\b[^.?!]{0,20}\b(?:anything|questions?|if)\b"
    r"|\btell\s+me\b[^.?!]{0,40}\b(?:if|what)\b"
    r"|\bwant\s+me\s+to\b[^.?!]{0,20}\b(?:keep\s+going|continue|slow\s+down|go\s+over|explain)\b"
    r"|\bshould\s+i\b[^.?!]{0,20}\b(?:keep\s+going|continue|slow\s+down|go\s+over|explain)\b",
    re.IGNORECASE,
)


def invites_comprehension_check(text: str) -> bool:
    """True when a TEACH reply ends by genuinely checking understanding or inviting a question.

    The positive counterpart to `shifts_mental_load`: that predicate VETOES a shape; this one is a
    REQUIREMENT, consulted only for `mode="teach"` (`shifts_mental_load` is never applied there —
    asking the learner to think is the pedagogy, not a load dump, and this function must not change
    that; it answers a different question, "is there a check at all", not "is the check too
    demanding").

    ASSERTION UNIT: one reply, not a window of recent turns. `proxy/teach.py` already commits to a
    per-turn guarantee (contract C5: "the model's own last beat" must check understanding, or a
    deterministic beat is appended) — this stays at that same granularity rather than adding a
    second, conflicting one. It is also the strictly stronger guarantee: if every reply carries a
    check, every window of N replies trivially does too, so this fully covers a defect measured
    across a 6-turn transcript without an arbitrary window size to tune (why 3? why 6? — no evidence
    picks one). And the harm is per-turn, not just in aggregate: five turns of uninterrupted
    information delivery already did the damage before a sixth, final check could "average it out"
    — a windowed unit would be scoring the transcript, not the experience it produced.

    Scoped to the reply's LAST sentence, mirroring `classify_beat_kind` (which also looks only at
    the final beat) and the product's own framing of the guarantee as an end-of-turn one. A check
    earlier in the reply, followed by more unchecked exposition, does not satisfy an end-of-turn
    guarantee; requiring one per SENTENCE would instead make every sentence a question, which is not
    what C5, teach.v1.md, or the teach-back literature ask for — a multi-beat explanation is allowed
    to just explain until its final beat.
    """
    stripped = _spoken_words_only(text).strip()
    sentences = [s.strip() for s in _SENTENCE_SPLIT.split(stripped) if s.strip()]
    if not sentences:
        return False
    last = sentences[-1]
    core = _TRAILING_PUNCT.sub("", last).strip()
    if not core:
        return False
    if _HOLLOW_CONFIRMATION.search(core):
        return False
    if last.endswith("?"):
        return True
    return bool(_GENUINE_CHECK_INVITATION.search(core))


def inspect_spoken_response(raw: str) -> SpokenInspection:
    """Parse a real speech envelope when present, then assert the exact outbound spoken text."""

    candidate = raw.strip()
    if not candidate:
        return SpokenInspection(
            GuardKind.FORMAT, "", GuardReason.EMPTY_OUTPUT, "empty model output"
        )

    structured = False
    if candidate.startswith("{"):
        try:
            decoded = json.loads(candidate)
            envelope = _SpeechEnvelope.model_validate(decoded)
        except (json.JSONDecodeError, ValidationError, TypeError) as exc:
            return SpokenInspection(
                GuardKind.FORMAT,
                "",
                GuardReason.INVALID_STRUCTURED_OUTPUT,
                str(exc),
            )
        candidate = envelope.spoken_text.strip()
        structured = True

    harm = _harm_detail(candidate)
    if harm is not None:
        return SpokenInspection(
            GuardKind.SAFETY,
            "",
            GuardReason.SHAME_ADJACENT_OUTPUT,
            harm,
            structured,
        )

    malformed = _format_detail(candidate)
    if malformed is not None:
        return SpokenInspection(
            GuardKind.FORMAT,
            "",
            GuardReason.UNSPEAKABLE_MARKUP,
            malformed,
            structured,
        )

    normalized = " ".join(candidate.split())
    if not normalized:
        return SpokenInspection(
            GuardKind.FORMAT, "", GuardReason.EMPTY_OUTPUT, "empty spoken_text", structured
        )
    return SpokenInspection(GuardKind.SAFE, normalized, structured=structured)


def _add_usage(left: UsageDelta, right: UsageDelta) -> UsageDelta:
    return UsageDelta(
        llm_tokens_in=left.llm_tokens_in + right.llm_tokens_in,
        llm_tokens_out=left.llm_tokens_out + right.llm_tokens_out,
        tts_chars_novel=left.tts_chars_novel + right.tts_chars_novel,
        stt_seconds_billed=left.stt_seconds_billed + right.stt_seconds_billed,
    )


def _repair_prompt(user_text: str, rejected: SpokenInspection) -> str:
    """One bounded re-ask naming the violation; never a retry loop."""

    reason = rejected.reason.value if rejected.reason is not None else "unknown"
    return (
        "Your previous speech envelope was rejected by server validation: "
        f"[{reason}] {rejected.detail or ''}.\n"
        "Answer the original user message again. Return one valid speech JSON object only; "
        "escape quotes and newlines inside spoken_text.\n"
        f"Original user message as JSON data: {json.dumps(user_text, ensure_ascii=False)}"
    )


async def complete_guarded_conversation(
    *,
    gateway: GatewayClient,
    tenant_id: str,
    user_id: str,
    session_id: str,
    system: str,
    user_text: str,
    max_tokens: int,
    history: Sequence[dict[str, str]] = (),
    mode: str | None = None,
    wait: WaitDirective | None = None,
) -> GuardedConversationCompletion:
    """Run a conversation completion through structural ingress and fail-closed egress gates."""

    if wait is not None:
        if wait.event in (WaitEvent.STARTED, WaitEvent.COMPANION_DUE):
            return build_wait_companion(wait, history)
        state = "interrupted" if wait.event is WaitEvent.USER_INTERRUPT else "completed"
        ordinary = await complete_guarded_conversation(
            gateway=gateway,
            tenant_id=tenant_id,
            user_id=user_id,
            session_id=session_id,
            system=system,
            user_text=user_text,
            max_tokens=max_tokens,
            history=history,
            mode=mode,
        )
        return replace(
            ordinary,
            wait=WaitOutcome(
                state=state,
                event=wait.event,
                companion_emitted=False,
                silence_budget_ms=wait.silence_budget_ms,
                elapsed_since_activity_ms=wait.elapsed_since_activity_ms,
            ),
        )

    if mode in ("converse", "focus") and is_explicit_refusal(user_text):
        return GuardedConversationCompletion(
            text=safe_refusal_fallback(history),
            usage=UsageDelta(),
            degraded=False,
            degrade_reason=None,
            source="deterministic_control",
            repair_attempts=0,
        )

    if requests_forbidden_register(user_text):
        dev_log(
            "conversation.safety_veto",
            level="warn",
            tenant_id=tenant_id,
            user_id=user_id,
            session_id=session_id,
            stage="ingress",
            control="register_authority",
            reason=GuardReason.CALLER_REGISTER_OVERRIDE.value,
            user_text=truncate(user_text),
        )
        return GuardedConversationCompletion(
            text=SAFE_REGISTER_FALLBACK,
            usage=UsageDelta(),
            degraded=True,
            degrade_reason=GuardReason.CALLER_REGISTER_OVERRIDE.value,
            source="safety_fallback",
            repair_attempts=0,
        )

    first = await gateway.complete(
        tenant_id=tenant_id,
        system=system,
        user_text=user_text,
        history=history,
        max_tokens=max_tokens,
        response_mode="speech",
    )
    inspected = inspect_spoken_response(first.text)
    if inspected.kind is GuardKind.SAFE:
        # Converse-mode-only egress control: the user has not asked for task help, so the orb must
        # not introduce step atomization unprompted. Structural, because the prompt rule alone was
        # measured being violated (see `proposes_task_step`).
        #
        # This DOES spend one repair call, for the same reason the verbatim-repeat control below
        # does, and unlike the register/shame vetoes above. A load-shifting reply is not a HARMFUL
        # reply, it is a MIS-SHAPED one: the user still wants real content, and a canned fallback is
        # strictly worse than a second attempt.
        #
        # Changed on evidence, not taste. Driving one realistic utterance ("my kitchen is a
        # disaster") through the live relay produced:
        #
        #   "Oh no, kitchens can get overwhelming. What's the first thing you notice that needs
        #    attention?"
        #
        # The veto was correct — and it threw away a warm, well-pitched acknowledgement over its
        # final sentence, replacing the whole turn with a fixed line. Across dev-logs that is
        # 103/1691 = 6.1% of converse turns degraded, the worst of any mode, which is the mechanism
        # behind the owner's report that the orb "just says what is the smallest part to start".
        #
        # The cost is one extra model call on ~6% of converse turns, and up to ~1.4s of added
        # latency on those turns (real p95 for a gateway completion). That is a deliberate trade:
        # a slower real answer beats a fast canned one, and the repair is bounded at exactly one
        # before falling back deterministically.
        if mode in ("converse", "focus") and shifts_mental_load(inspected.text):
            dev_log(
                "conversation.safety_veto",
                level="warn",
                tenant_id=tenant_id,
                user_id=user_id,
                session_id=session_id,
                stage="egress",
                control="mental_load_shifted",
                reason=GuardReason.MENTAL_LOAD_SHIFTED.value,
                mode=mode,
                rejected_text=truncate(inspected.text),
            )
            load_retry = await gateway.complete(
                tenant_id=tenant_id,
                system=system,
                user_text=(
                    f"{user_text}\n\n[system: your previous reply ended by asking the user to "
                    "identify, rank or choose the step — that hands the thinking back to them, "
                    "which is the one thing you must never do. Keep your acknowledgement of what "
                    "they said. Then either name ONE specific small step yourself, or simply stay "
                    "with them and ask nothing. Do not ask what the smallest/first/easiest thing "
                    "is, and do not ask what they want to do.]"
                ),
                history=history,
                max_tokens=max_tokens,
                response_mode="speech",
            )
            load_usage = _add_usage(first.usage, load_retry.usage)
            load_inspection = inspect_spoken_response(load_retry.text)
            # The retry must clear every control the first attempt cleared plus the one it failed,
            # otherwise a "repair" could smuggle in a fresh defect (a repeat, or unspeakable markup).
            if (
                load_inspection.kind is GuardKind.SAFE
                and not shifts_mental_load(load_inspection.text)
                and not repeats_prior_reply(load_inspection.text, history)
            ):
                dev_log(
                    "conversation.output_repaired",
                    tenant_id=tenant_id,
                    user_id=user_id,
                    session_id=session_id,
                    reason=GuardReason.MENTAL_LOAD_SHIFTED.value,
                )
                return GuardedConversationCompletion(
                    text=load_inspection.text,
                    usage=load_usage,
                    degraded=True,
                    degrade_reason=(
                        f"model_output_repaired:{GuardReason.MENTAL_LOAD_SHIFTED.value}"
                    ),
                    source="model_repaired",
                    repair_attempts=1,
                )
            dev_log(
                "conversation.output_fallback",
                level="warn",
                tenant_id=tenant_id,
                user_id=user_id,
                session_id=session_id,
                control="mental_load_shifted",
                reason=GuardReason.MENTAL_LOAD_SHIFTED.value,
                mode=mode,
                rejected_text=truncate(load_inspection.text),
            )
            return GuardedConversationCompletion(
                text=safe_converse_fallback(history),
                usage=load_usage,
                degraded=True,
                degrade_reason=GuardReason.MENTAL_LOAD_SHIFTED.value,
                source="safety_fallback",
                repair_attempts=1,
            )
        # TEACH comprehension-check control. `teach` is the one mode EXEMPT from
        # `shifts_mental_load` above (asking the learner to think is the pedagogy), but a live
        # simulated-user judge caught the mirror-image failure: a reply that only ever answers and
        # never checks whether it landed, or invites a question, is its own load-direction defect —
        # information moved only from the orb to the user, with no channel back. Structural because
        # a prompt rule alone was already measured being violated: a real live run had 5 of 6 turns
        # end with no check at all, and the one that did checked ended on "Does that make sense?"
        # (see tests/test_teach_comprehension_check.py for the verbatim transcript). This is teach's
        # counterpart to `mental_load_shifted`/`verbatim_repeat`, wired the same way — exactly one
        # bounded repair, then a deterministic outcome, never silence — except the deterministic
        # floor APPENDS to the model's own (already-safe) explanation instead of replacing it; see
        # `safe_teach_check_fallback`'s docstring for why replacing would be the worse failure here.
        if mode == "teach" and not invites_comprehension_check(inspected.text):
            dev_log(
                "conversation.safety_veto",
                level="warn",
                tenant_id=tenant_id,
                user_id=user_id,
                session_id=session_id,
                stage="egress",
                control="teach_check_missing",
                reason=GuardReason.TEACH_CHECK_MISSING.value,
                mode=mode,
                rejected_text=truncate(inspected.text),
            )
            check_retry = await gateway.complete(
                tenant_id=tenant_id,
                system=system,
                user_text=(
                    f"{user_text}\n\n[system: your previous reply explained something but ended as "
                    "a pure information dump, with no comprehension check and no invitation for a "
                    "question -- teach mode must never do that. Keep your explanation. Then end "
                    "with exactly one of: (1) a real teach-back question that makes the user say "
                    "something back, such as asking what they think happens next or asking them to "
                    "put the last point in their own words, (2) an explicit invitation for their "
                    "own question, or (3) a concrete two-way choice about pacing (keep going, or go "
                    "over the last part again). A bare 'does that make sense?', 'understand?', or a "
                    "tag question like '...right?' does not count -- it only invites a reflexive "
                    "yes, never a real answer.]"
                ),
                history=history,
                max_tokens=max_tokens,
                response_mode="speech",
            )
            check_usage = _add_usage(first.usage, check_retry.usage)
            check_inspection = inspect_spoken_response(check_retry.text)
            # Same discipline as the load-shifting repair above: the retry must clear every control
            # the first attempt cleared (safe, non-repeat) PLUS the one it failed (a genuine check),
            # or a "repair" could smuggle in a fresh defect.
            if (
                check_inspection.kind is GuardKind.SAFE
                and invites_comprehension_check(check_inspection.text)
                and not repeats_prior_reply(check_inspection.text, history)
            ):
                dev_log(
                    "conversation.output_repaired",
                    tenant_id=tenant_id,
                    user_id=user_id,
                    session_id=session_id,
                    reason=GuardReason.TEACH_CHECK_MISSING.value,
                )
                return GuardedConversationCompletion(
                    text=check_inspection.text,
                    usage=check_usage,
                    degraded=True,
                    degrade_reason=f"model_output_repaired:{GuardReason.TEACH_CHECK_MISSING.value}",
                    source="model_repaired",
                    repair_attempts=1,
                )
            dev_log(
                "conversation.output_fallback",
                level="warn",
                tenant_id=tenant_id,
                user_id=user_id,
                session_id=session_id,
                control="teach_check_missing",
                reason=GuardReason.TEACH_CHECK_MISSING.value,
                mode=mode,
                rejected_text=truncate(check_retry.text),
            )
            return GuardedConversationCompletion(
                text=safe_teach_check_fallback(inspected.text, history),
                usage=check_usage,
                degraded=True,
                degrade_reason=GuardReason.TEACH_CHECK_MISSING.value,
                source="safety_fallback",
                repair_attempts=1,
            )
        # Verbatim-repeat control. Unlike the safety vetoes above this DOES spend one repair call:
        # a repeat is not a harmful reply, it is a missing one, and the user still wants real
        # content — a canned fallback is strictly worse than a second attempt. Bounded at exactly
        # one, then deterministic.
        if repeats_prior_reply(inspected.text, history) and not _ASKS_FOR_REPETITION.search(
            user_text
        ):
            dev_log(
                "conversation.output_rejected",
                level="warn",
                tenant_id=tenant_id,
                user_id=user_id,
                session_id=session_id,
                stage="egress",
                control="verbatim_repeat",
                reason=GuardReason.VERBATIM_REPEAT.value,
                mode=mode,
                rejected_text=truncate(inspected.text),
            )
            retry = await gateway.complete(
                tenant_id=tenant_id,
                system=system,
                user_text=(
                    f"{user_text}\n\n[system: your previous reply was identical to one you already "
                    "gave in this session. Say something different. If the user declined a "
                    "suggestion, do not offer that same suggestion again — propose a different "
                    "concrete option, or simply keep them company. Never ask them to come up with "
                    "it.]"
                ),
                history=history,
                max_tokens=max_tokens,
                response_mode="speech",
            )
            retry_usage = _add_usage(first.usage, retry.usage)
            retry_inspection = inspect_spoken_response(retry.text)
            retry_ok = (
                retry_inspection.kind is GuardKind.SAFE
                and not repeats_prior_reply(retry_inspection.text, history)
                and not (
                    mode in ("converse", "focus") and shifts_mental_load(retry_inspection.text)
                )
            )
            if retry_ok:
                dev_log(
                    "conversation.output_repaired",
                    tenant_id=tenant_id,
                    user_id=user_id,
                    session_id=session_id,
                    reason=GuardReason.VERBATIM_REPEAT.value,
                )
                return GuardedConversationCompletion(
                    text=retry_inspection.text,
                    usage=retry_usage,
                    degraded=True,
                    degrade_reason=f"model_output_repaired:{GuardReason.VERBATIM_REPEAT.value}",
                    source="model_repaired",
                    repair_attempts=1,
                )
            dev_log(
                "conversation.output_fallback",
                level="warn",
                tenant_id=tenant_id,
                user_id=user_id,
                session_id=session_id,
                reason=GuardReason.VERBATIM_REPEAT.value,
                detail="repair also repeated, was unsafe, or shifted load",
                rejected_text=truncate(retry.text),
            )
            return GuardedConversationCompletion(
                text=SAFE_REPEAT_FALLBACK,
                usage=retry_usage,
                degraded=True,
                degrade_reason=GuardReason.VERBATIM_REPEAT.value,
                source="safety_fallback",
                repair_attempts=1,
            )
        return GuardedConversationCompletion(
            text=inspected.text,
            usage=first.usage,
            degraded=False,
            degrade_reason=None,
            source="model",
            repair_attempts=0,
        )

    if inspected.kind is GuardKind.SAFETY:
        dev_log(
            "conversation.safety_veto",
            level="warn",
            tenant_id=tenant_id,
            user_id=user_id,
            session_id=session_id,
            stage="egress",
            control="response_language_veto",
            reason=inspected.reason.value if inspected.reason is not None else "unknown",
            detail=inspected.detail,
            rejected_text=truncate(first.text),
        )
        return GuardedConversationCompletion(
            text=SAFE_OUTPUT_FALLBACK,
            usage=first.usage,
            degraded=True,
            degrade_reason=GuardReason.SHAME_ADJACENT_OUTPUT.value,
            source="safety_fallback",
            repair_attempts=0,
        )

    dev_log(
        "conversation.output_rejected",
        level="warn",
        tenant_id=tenant_id,
        user_id=user_id,
        session_id=session_id,
        attempt=0,
        reason=inspected.reason.value if inspected.reason is not None else "unknown",
        detail=inspected.detail,
        rejected_text=truncate(first.text),
    )
    repaired = await gateway.complete(
        tenant_id=tenant_id,
        system=system,
        user_text=_repair_prompt(user_text, inspected),
        history=history,
        max_tokens=max_tokens,
        response_mode="speech",
    )
    usage = _add_usage(first.usage, repaired.usage)
    repaired_inspection = inspect_spoken_response(repaired.text)
    if repaired_inspection.kind is GuardKind.SAFE:
        initial_reason = inspected.reason.value if inspected.reason is not None else "unknown"
        dev_log(
            "conversation.output_repaired",
            tenant_id=tenant_id,
            user_id=user_id,
            session_id=session_id,
            reason=initial_reason,
        )
        return GuardedConversationCompletion(
            text=repaired_inspection.text,
            usage=usage,
            degraded=True,
            degrade_reason=f"model_output_repaired:{initial_reason}",
            source="model_repaired",
            repair_attempts=1,
        )

    event = (
        "conversation.safety_veto"
        if repaired_inspection.kind is GuardKind.SAFETY
        else "conversation.output_fallback"
    )
    dev_log(
        event,
        level="warn",
        tenant_id=tenant_id,
        user_id=user_id,
        session_id=session_id,
        stage="repair",
        attempt=1,
        reason=repaired_inspection.reason.value
        if repaired_inspection.reason is not None
        else "unknown",
        detail=repaired_inspection.detail,
        rejected_text=truncate(repaired.text),
    )
    safety_failure = repaired_inspection.kind is GuardKind.SAFETY
    repaired_reason = (
        repaired_inspection.reason.value
        if repaired_inspection.reason is not None
        else "invalid_model_output"
    )
    return GuardedConversationCompletion(
        text=SAFE_OUTPUT_FALLBACK if safety_failure else SAFE_FORMAT_FALLBACK,
        usage=usage,
        degraded=True,
        degrade_reason=(
            GuardReason.SHAME_ADJACENT_OUTPUT.value if safety_failure else repaired_reason
        ),
        source="safety_fallback" if safety_failure else "format_fallback",
        repair_attempts=1,
    )
