"""No spoken-language prompt may instruct the model to hand the mental load back to the user.

This exists because the two prompts and the egress guard were in direct contradiction:

  * `converse.v1.md` said *"do not invite a next step"* — the owner's correction is the opposite
    (propose the smallest step, don't make the user produce it).
  * `focus-companion.v1.md` said *"invite one next step"* — "invite" is the elicit verb, so the
    prompt instructed the model to emit exactly the shape
    `conversation_guard.shifts_mental_load()` now vetoes in focus mode. The prompt was asking for
    output the guard would refuse.

A prompt rule is only ever a `mitigates` (the model ignores it often enough to have been measured
doing so), so the guard is the real control. This gate guards the *other* direction: that we never
again ship a prompt whose instructions fight the guard. It checks the imperative phrasing WE write,
never model output.

## Why this file has a normalizer instead of a list of regexes

The first version of this gate anchored every pattern to the start of a line, and so it **missed
both of the two real defects above** while happily catching invented ones — a gate that measured
nothing, green from the first run. The two shipped defects sat mid-sentence
("acknowledge it and invite one next step"), and un-anchoring the patterns then flagged the prompts'
own *negative examples* ("Do not ask the user to produce…"), which are the correct wording.

So the discrimination this gate actually needs is between an instruction and a quoted anti-example,
and between an imperative and its negation. That is what `_rule_clauses` does, and there are two
pattern classes because negation flips which side is the defect:

  * **ELICIT_IMPERATIVES** — the defect only when *not* negated ("invite one next step" is bad;
    "do not invite one next step" was v1's rule and is caught by the other class).
  * **ANTI_PROPOSE_PROHIBITIONS** — the negation *is* the defect ("do not suggest a step" forbids
    the thing the product exists to do).
"""

from __future__ import annotations

import re

import pytest
from orb_relay.proxy.prompts import load_agent_prompt

# Modes whose replies the egress guard vetoes for load-shifting. `teach.v1` is deliberately absent:
# asking a learner to think is the pedagogy, and the guard exempts it by mode.
LOAD_CARRYING_PROMPTS = ("converse.v1", "focus-companion.v1")

# Double quotes only, on purpose: including the apostrophe would let two unrelated apostrophes
# ("don't" … "user's") pair up and silently delete the instruction between them.
_QUOTED = re.compile(r"[\"“”][^\"“”\n]{0,200}[\"“”]")
_BOLD = re.compile(r"\*+")
_NEGATED = re.compile(
    r"\b(?:do\s+not|don'?t|never|rather\s+than|instead\s+of|avoid)\b", re.IGNORECASE
)

# The defect ONLY when stated as a positive instruction.
ELICIT_IMPERATIVES = (
    re.compile(
        r"\bask\s+(?:the\s+user|them)\s+to\s+(?:pick|choose|decide|produce|generate|"
        r"come\s+up\s+with|identify|figure\s+out|break\s+(?:\w+\s+){1,3}down)",
        re.IGNORECASE,
    ),
    re.compile(r"\binvite\s+(?:one\s+|a\s+|the\s+)?next\s+step", re.IGNORECASE),
    re.compile(r"\blet\s+(?:the\s+user|them)\s+(?:decide|choose|pick)", re.IGNORECASE),
    re.compile(
        r"\b(?:leave|put)\s+(?:it|the\s+\w+)\s+(?:up\s+)?to\s+(?:the\s+user|them)", re.IGNORECASE
    ),
)

# The defect BECAUSE it is negated: forbidding the model from carrying the load.
ANTI_PROPOSE_PROHIBITIONS = (
    re.compile(
        r"\b(?:do\s+not|don'?t|never)\s+(?:\w+\s+){0,2}?"
        r"(?:suggest|propose|offer|name|invite|recommend)\s+(?:a\s+|the\s+|one\s+|any\s+)?"
        r"(?:next\s+|smallest\s+|first\s+)?(?:step|action|option)",
        re.IGNORECASE,
    ),
    # `invite` needs no object: the noun is often inside the quoted span that gets stripped, which
    # is exactly how the shipped `do not invite a "next step"` rule escaped the first version. There
    # is no benign "do not invite" in a spoken-language prompt.
    re.compile(r"\b(?:do\s+not|don'?t|never)\s+invite\b", re.IGNORECASE),
)


def _rule_clauses(text: str) -> list[str]:
    """Return only the clauses that are instructions to the model, negation-free.

    Drops indented example blocks (the BAD/GOOD tables), quoted spans (where anti-patterns are
    quoted on purpose), non-bullet prose, and any clause carrying a negation.
    """
    clauses: list[str] = []
    for raw in text.splitlines():
        if raw.startswith(("    ", "\t")):
            continue  # indented example block
        line = raw.strip()
        if not line.startswith(("- ", "* ")):
            continue  # prompt rules are bullets; surrounding prose is commentary
        line = _BOLD.sub("", _QUOTED.sub(" ", line.lstrip("-* ")))
        for clause in re.split(r"[.;:—]|\bbut\b", line):
            clause = clause.strip()
            if clause and not _NEGATED.search(clause):
                clauses.append(clause)
    return clauses


def _prohibition_text(text: str) -> str:
    """Full text with quoted spans and indented examples removed (negation is the signal here)."""
    kept = [ln for ln in text.splitlines() if not ln.startswith(("    ", "\t"))]
    return _BOLD.sub("", _QUOTED.sub(" ", "\n".join(kept)))


def findings(text: str) -> list[str]:
    """Every load-offloading instruction in `text`. Exposed so the red-proof script can reuse it."""
    hits = [
        m.group(0).strip()
        for clause in _rule_clauses(text)
        for pat in ELICIT_IMPERATIVES
        if (m := pat.search(clause))
    ]
    hits += [
        m.group(0).strip()
        for pat in ANTI_PROPOSE_PROHIBITIONS
        if (m := pat.search(_prohibition_text(text)))
    ]
    return hits


@pytest.mark.parametrize("prompt_id", LOAD_CARRYING_PROMPTS)
def test_prompt_never_instructs_the_model_to_offload_the_decision(prompt_id: str) -> None:
    hits = findings(load_agent_prompt(prompt_id))
    assert not hits, (
        f"{prompt_id}.md instructs the model to hand the mental load back: {hits}. "
        "The orb proposes the smallest step itself; it never makes the user generate one. "
        "`conversation_guard.shifts_mental_load` vetoes that output, so this instruction would make "
        "the prompt fight the guard."
    )


@pytest.mark.parametrize("prompt_id", LOAD_CARRYING_PROMPTS)
def test_prompt_positively_instructs_the_model_to_propose(prompt_id: str) -> None:
    """The absence of a bad rule is not the presence of a good one."""
    text = load_agent_prompt(prompt_id).lower()
    assert any(m in text for m in ("propose, don't elicit", "name one specific next step")), (
        f"{prompt_id}.md has no positive instruction to propose a concrete smallest step. "
        "Removing the wrong rule is not the same as stating the right one."
    )


class TestTheGateActuallyFires:
    """Without these, the gate above is green-on-arrival and proves nothing.

    The two `SHIPPED_*` strings are the verbatim rules that were live in this repo earlier today.
    """

    SHIPPED_FOCUS_V1 = (
        "- If they mention a task, acknowledge it and invite one next step; never emit a task plan "
        "here —\n  that is the atomizer's job."
    )
    SHIPPED_CONVERSE_V1 = (
        '- Do not redirect to a task and do not invite a "next step" — there may not be one, and '
        "assuming\n  one is unwelcome when the user is venting, thinking out loud, or just talking."
    )

    def test_catches_the_shipped_focus_rule(self) -> None:
        assert findings(self.SHIPPED_FOCUS_V1), "missed the mid-sentence elicit imperative"

    def test_catches_the_shipped_converse_rule(self) -> None:
        assert findings(self.SHIPPED_CONVERSE_V1), "missed the prohibition against proposing"

    @pytest.mark.parametrize(
        "regression",
        [
            "- Ask the user to pick which step feels smallest.",
            "- Let the user decide what happens next.",
            "- Leave it up to the user.",
            "- Do not suggest a next step.",
            "- Never offer the first action yourself.",
            "- Ask them to break the task down.",
        ],
    )
    def test_catches_plausible_future_regressions(self, regression: str) -> None:
        assert findings(regression), regression

    @pytest.mark.parametrize(
        "correct_wording",
        [
            # Every one of these is the RIGHT wording and must not be flagged. Several are the
            # prompts' own quoted anti-examples — the case that broke the naive un-anchored version.
            (
                "- **Do not ask the user to produce, rank, or choose the step.** "
                '"It\'s up to you" hands the work back.'
            ),
            '- v1 of this prompt said "do not invite a next step". That was wrong.',
            "- **Propose, don't elicit.** Name a specific step and make it absurdly small.",
            "- Never end on a question that requires them to plan, decide, or summarise.",
            "- If a choice genuinely belongs to the user, give at most two concrete named options.",
            "- Never ask them to remember something for you.",
        ],
    )
    def test_does_not_flag_correct_wording(self, correct_wording: str) -> None:
        assert not findings(correct_wording), findings(correct_wording)


def test_teach_mode_is_deliberately_excluded_from_this_gate() -> None:
    """Documents the carve-out as an assertion so a future edit cannot quietly widen the gate."""
    assert "teach.v1" not in LOAD_CARRYING_PROMPTS
    assert load_agent_prompt("teach.v1"), "teach.v1 must still exist and be non-empty"
