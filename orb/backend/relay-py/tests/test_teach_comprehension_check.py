"""TEACH's comprehension-check control: `invites_comprehension_check` and its guard wiring.

Why this exists. `evals/simulated_user/run_scenario_conversation.py`'s `teach-me-something`
scenario is a live, LLM-judged conversation (real model, real user-simulator, real judge) run
against the real `/v1/respond` endpoint, driving `mode="teach"`. Run twice in this session against
the live relay:

  * Run 1: judge said `success=True`, reasoning ended "... the agent checks for understanding or
    invites questions from the user at any point, instead only providing information..." — but
    5 of its 6 real turns end on a bare information dump or a hollow confirmation. The judge missed
    it; a structural check should not.
  * Run 2: judge said `success=False` (process exit 6), reasoning: "the assistant did not
    explicitly check for understanding or invite questions from the user at any point, instead only
    providing information in response to user prompts or questions."

`RUN_1_VERBATIM` and `RUN_2_VERBATIM` below are the exact spoken text returned by the live model on
each of those runs (2026-08-28) — real model output, not invented examples. This is also why the
control cannot be a prompt rule alone: `domain/agents/teach.v1.md` already told the model to end
every turn with a check before either of these runs, and the model did it in only 2 of 10 real
turns.

Denominator on the claim above: 10 real verbatim assistant turns observed across the 2 live runs;
`invites_comprehension_check` returns True for exactly the 2 turn-1 (mode-entry) turns and False for
the other 8 — see `test_red_proof_*` below. `open-domain-vent` (the sibling scenario, `mode:
converse`) is unrelated to this file; it is unaffected by anything here (see
`test_converse_and_focus_are_never_touched_by_this_control` at the bottom).
"""

from __future__ import annotations

import pytest
from orb_relay.cost.meter import UsageDelta
from orb_relay.proxy.conversation_guard import (
    TEACH_CHECK_APPENDS,
    complete_guarded_conversation,
    invites_comprehension_check,
    safe_teach_check_fallback,
)
from orb_relay.proxy.gateway_client import GatewayCompletion
from orb_relay.proxy.prompts import load_agent_prompt

# ---------------------------------------------------------------------------------------------
# Real verbatim transcripts (see module docstring). Fish-tag markup ([warm], [short pause], ...)
# is included exactly as spoken, because the predicate must work on real TTS-marked-up text, not
# a cleaned-up paraphrase.
# ---------------------------------------------------------------------------------------------

RUN_1_VERBATIM = [
    "[warm] Absolutely! What topic are you curious about learning today?",
    (
        "[inhale] [friendly] Photosynthesis is how plants make their own food using [emphasis] "
        "sunlight, water, and carbon dioxide. It's like their own little kitchen!"
    ),
    (
        "[inhale] [friendly] Right! The plant then uses the energy from that sunlight to combine "
        "the water and carbon dioxide. Think of it like [emphasis] baking!"
    ),
    (
        '[happy] They "bake" sugars! These sugars are the plant\'s [emphasis] food, giving them '
        "the energy they need to grow and thrive."
    ),
    (
        "[breathing] [gentle] As a byproduct, they also release [emphasis] oxygen back into the "
        "air. That's pretty crucial for us, right?"
    ),
    (
        "[thinking] [hmm] You could say that! The sugars are the main product, and oxygen is "
        "like an extra bonus they release. [short pause] Does that make sense?"
    ),
]

RUN_2_VERBATIM = [
    "[warm] I'm ready! What amazing thing would you like to explore today?",
    (
        "[inhale] [friendly] Photosynthesis! It's how plants create their own food. [short "
        "pause] They use three main ingredients: sunlight, water, and carbon dioxide."
    ),
    (
        "[happy] Yep! The plant captures sunlight using a green pigment called [emphasis] "
        "chlorophyll in its leaves."
    ),
    (
        "[breathing] [friendly] Great! So, the chlorophyll helps the plant absorb the sunlight. "
        "Then, it takes in [emphasis] carbon dioxide from the air."
    ),
]


class TestRedProofRun1:
    """Judge said success=True on this transcript. The structural check disagrees with the judge
    on 5 of its 6 turns, which is the whole reason a structural check exists alongside one."""

    def test_turn_1_mode_entry_question_passes(self) -> None:
        assert invites_comprehension_check(RUN_1_VERBATIM[0]) is True

    @pytest.mark.parametrize("index", [1, 2, 3, 4, 5])
    def test_turns_2_through_6_are_flagged_as_missing_a_genuine_check(self, index: int) -> None:
        assert invites_comprehension_check(RUN_1_VERBATIM[index]) is False, RUN_1_VERBATIM[index]

    def test_turn_6_is_specifically_the_hollow_does_that_make_sense_trap(self) -> None:
        assert RUN_1_VERBATIM[5].rstrip().endswith("Does that make sense?")
        assert invites_comprehension_check(RUN_1_VERBATIM[5]) is False

    def test_turn_5_is_specifically_the_hollow_tag_question_trap(self) -> None:
        assert RUN_1_VERBATIM[4].rstrip().endswith("right?")
        assert invites_comprehension_check(RUN_1_VERBATIM[4]) is False


class TestRedProofRun2:
    """The run the live judge itself failed (exit code 6). Included so the predicate is proven
    against the exact case the task report was written from, not only the borderline one."""

    def test_turn_1_mode_entry_question_passes(self) -> None:
        assert invites_comprehension_check(RUN_2_VERBATIM[0]) is True

    @pytest.mark.parametrize("index", [1, 2, 3])
    def test_turns_2_through_4_are_flagged_as_missing_a_genuine_check(self, index: int) -> None:
        assert invites_comprehension_check(RUN_2_VERBATIM[index]) is False, RUN_2_VERBATIM[index]


# ---------------------------------------------------------------------------------------------
# Pure predicate: synthetic positive/negative denominator, beyond the two real transcripts above.
# ---------------------------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text",
    [
        # Real teach-back: makes the user produce a prediction or a restatement.
        "Plants use sunlight to build sugar. What do you think happens to that sugar once it's made?",
        "Can you tell me back in your own words what chlorophyll does?",
        # Explicit invitation for the user's own question.
        "What questions do you have about that part before I keep going?",
        "Let me know if any of that didn't land and I'll go over it again.",
        # Concrete, two-way pacing choice (the existing proxy/teach.py deterministic beat's shape).
        "Want me to keep going, or should I slow down on that last part?",
        "Should I continue, or explain that differently first?",
        # Matches the existing test_teach.py acceptance case for classify_beat_kind/build_beats.
        "Photosynthesis converts light into sugar. Do you want the next part?",
    ],
)
def test_genuine_checks_and_invitations_are_recognized(text: str) -> None:
    assert invites_comprehension_check(text) is True, text


@pytest.mark.parametrize(
    "text",
    [
        # The canonical hollow confirmation this whole control exists to catch.
        "Does that make sense?",
        "Does that make sense so far?",
        "Understand?",
        "Do you understand?",
        "Got it?",
        "Is that clear?",
        "You with me?",
        "Following so far?",
        "That covers the basics, right?",
        "Okay?",
        # A pure information dump: no question, no invitation, nothing for the user to respond to.
        "Plants convert sunlight into chemical energy stored as sugar.",
        "They release oxygen as a byproduct of the reaction.",
        # Empty / whitespace-only text.
        "",
        "   ",
    ],
)
def test_hollow_confirmations_and_bare_dumps_are_rejected(text: str) -> None:
    assert invites_comprehension_check(text) is False, text


def test_only_the_last_sentence_is_scoped_an_early_question_does_not_count() -> None:
    """The guarantee is END-of-turn (contract C5); a question earlier in the reply followed by
    more unchecked exposition must not satisfy it."""
    text = (
        "What do you already know about photosynthesis? Anyway, plants use sunlight to make sugar."
    )
    assert invites_comprehension_check(text) is False


def test_a_multi_beat_explanation_only_needs_the_final_beat_to_check() -> None:
    """The converse -- a reply is not required to turn every sentence into a question."""
    text = (
        "Photosynthesis converts light into sugar. Chlorophyll absorbs the light. "
        "What questions do you have so far?"
    )
    assert invites_comprehension_check(text) is True


def test_fish_tags_mid_phrase_do_not_break_detection() -> None:
    """Mirrors the real hole found in `shifts_mental_load`: prosody tags can land mid-phrase."""
    text = "Plants make their own food. What do [emphasis] you think happens next?"
    assert invites_comprehension_check(text) is True


# ---------------------------------------------------------------------------------------------
# safe_teach_check_fallback: the deterministic floor, and its self-consistency with the predicate
# it exists to satisfy.
# ---------------------------------------------------------------------------------------------


def test_fallback_appends_rather_than_replaces() -> None:
    base = "Plants use sunlight, water, and carbon dioxide to build sugar."
    result = safe_teach_check_fallback(base, history=())
    assert result.startswith(base)
    assert result != base


def test_fallback_output_always_satisfies_the_predicate_it_exists_to_satisfy() -> None:
    for n in range(len(TEACH_CHECK_APPENDS) * 2):
        history = [{"role": "assistant", "content": "x"}] * n
        result = safe_teach_check_fallback("Some explanation without an ending.", history=history)
        assert invites_comprehension_check(result), result


def test_fallback_rotates_deterministically_by_turn_position() -> None:
    seen = {
        safe_teach_check_fallback(
            "Explaining something.", history=[{"role": "assistant", "content": "x"}] * n
        )
        for n in range(len(TEACH_CHECK_APPENDS))
    }
    assert len(seen) == len(TEACH_CHECK_APPENDS), "must not say the identical line every time"


def test_fallback_never_returns_empty_even_with_a_blank_base() -> None:
    assert safe_teach_check_fallback("", history=()).strip()
    assert safe_teach_check_fallback("   ", history=()).strip()


# ---------------------------------------------------------------------------------------------
# Guard wiring: complete_guarded_conversation, mirroring tests/test_load_shift_gets_one_repair.py's
# shape for its sibling controls (mental_load_shifted, verbatim_repeat).
# ---------------------------------------------------------------------------------------------

NO_CHECK_REPLY = (
    "Plants use sunlight, water, and carbon dioxide to build sugar. It's like their own kitchen."
)
CLEAN_CHECKED_REPAIR = (
    "Plants use sunlight, water, and carbon dioxide to build sugar. "
    "What do you think happens to that sugar once it's made?"
)


class ScriptGateway:
    """Returns queued replies in order; exhaustion is an AssertionError, not a silent repeat, so a
    missing/extra call is caught rather than masked. Mirrors test_load_shift_gets_one_repair.py."""

    def __init__(self, *replies: str) -> None:
        self._replies = list(replies)
        self.calls: list[dict[str, object]] = []

    async def complete(self, **kwargs: object) -> GatewayCompletion:
        self.calls.append(kwargs)
        if not self._replies:
            raise AssertionError(
                f"gateway called {len(self.calls)} time(s) but only "
                f"{len(self.calls) - 1} reply/replies were scripted"
            )
        return GatewayCompletion(
            self._replies.pop(0), UsageDelta(llm_tokens_in=10, llm_tokens_out=5)
        )


async def _run(gateway: ScriptGateway, *, mode: str = "teach", history=()):
    return await complete_guarded_conversation(
        gateway=gateway,
        tenant_id="t0",
        user_id="u1",
        session_id="s1",
        system="teach warmly",
        user_text="how does photosynthesis work",
        max_tokens=480,
        history=history,
        mode=mode,
    )


@pytest.mark.asyncio
async def test_teach_reply_that_already_checks_understanding_costs_one_call() -> None:
    gateway = ScriptGateway(CLEAN_CHECKED_REPAIR)
    result = await _run(gateway)
    assert result.text == CLEAN_CHECKED_REPAIR
    assert result.source == "model"
    assert result.degraded is False
    assert result.repair_attempts == 0
    assert len(gateway.calls) == 1


@pytest.mark.asyncio
async def test_missing_check_is_repaired_not_replaced() -> None:
    gateway = ScriptGateway(NO_CHECK_REPLY, CLEAN_CHECKED_REPAIR)
    result = await _run(gateway)
    assert result.text == CLEAN_CHECKED_REPAIR, (
        "the repaired reply should be spoken, not a canned line"
    )
    assert result.source == "model_repaired"
    assert result.degrade_reason == "model_output_repaired:teach_check_missing"
    assert result.repair_attempts == 1
    assert len(gateway.calls) == 2
    assert result.usage.llm_tokens_in == 20
    assert result.usage.llm_tokens_out == 10


@pytest.mark.asyncio
async def test_repair_prompt_names_the_defect_and_bans_the_hollow_shape() -> None:
    gateway = ScriptGateway(NO_CHECK_REPLY, CLEAN_CHECKED_REPAIR)
    await _run(gateway)
    repair_text = str(gateway.calls[1]["user_text"])
    assert "information dump" in repair_text
    assert "does that make sense" in repair_text.lower()
    assert "reflexive" in repair_text.lower()
    # The original user message must still be there, same discipline as the load-shift repair.
    assert "how does photosynthesis work" in repair_text


@pytest.mark.asyncio
async def test_a_second_missing_check_falls_back_by_appending_never_by_erasing_content() -> None:
    """The bound (exactly one repair) AND the never-silent, never-erase guarantee together."""
    still_broken = "Plants use sunlight, water, and carbon dioxide to build sugar and grow tall."
    gateway = ScriptGateway(NO_CHECK_REPLY, still_broken)
    result = await _run(gateway)
    assert result.source == "safety_fallback"
    assert result.degrade_reason == "teach_check_missing"
    assert result.repair_attempts == 1
    assert len(gateway.calls) == 2
    assert result.text.startswith(NO_CHECK_REPLY), (
        "real taught content must be preserved, not erased"
    )
    assert result.text != NO_CHECK_REPLY, (
        "something must have been appended -- never falls back to silence"
    )
    assert result.text.strip()
    assert invites_comprehension_check(result.text)


@pytest.mark.asyncio
async def test_repair_cannot_smuggle_in_a_verbatim_repeat() -> None:
    prior = (
        "Plants use sunlight, water, and carbon dioxide to build sugar. What questions do you have?"
    )
    history = [
        {"role": "user", "content": "teach me something"},
        {"role": "assistant", "content": prior},
    ]
    gateway = ScriptGateway("Chlorophyll absorbs the light the plant uses.", prior)
    result = await _run(gateway, history=history)
    assert result.source == "safety_fallback", (
        "a retry that repeats a prior assistant turn must not be accepted as a repair"
    )
    assert result.repair_attempts == 1


@pytest.mark.asyncio
async def test_converse_and_focus_are_never_touched_by_this_control() -> None:
    """Must not leak into converse/focus: a pure information reply with no question anywhere is
    completely normal outside teach mode and must cost exactly one call, unmodified."""
    for mode in ("converse", "focus"):
        gateway = ScriptGateway("Kitchens pile up fast when a lot is going on at once.")
        result = await _run(gateway, mode=mode)
        assert result.text == "Kitchens pile up fast when a lot is going on at once."
        assert result.source == "model"
        assert result.degraded is False
        assert result.repair_attempts == 0
        assert len(gateway.calls) == 1


# ---------------------------------------------------------------------------------------------
# Prompt-wording gate (the skill's second, non-structural half): domain/agents/teach.v1.md must
# actually state the obligation this file's structural control exists to back up. Mirrors the
# convention in tests/test_prompts_carry_the_load.py -- a prompt that regresses to prose-only or
# re-legitimizes the hollow shape should fail a test, not wait for the next live judge run.
# ---------------------------------------------------------------------------------------------


def test_teach_prompt_bans_the_hollow_confirmation_shape_by_name() -> None:
    text = load_agent_prompt("teach.v1").lower()
    assert "does that make sense" in text
    assert "reflexive" in text


def test_teach_prompt_states_the_check_obligation_as_ordered_and_every_turn() -> None:
    text = load_agent_prompt("teach.v1").lower()
    assert "every turn" in text
    assert "teach-back" in text


def test_teach_prompt_references_the_structural_backstop_without_relying_on_it() -> None:
    text = load_agent_prompt("teach.v1")
    assert "invites_comprehension_check" in text
    assert "prompt rule alone was already measured being violated" in text.lower()
