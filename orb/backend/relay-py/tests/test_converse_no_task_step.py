"""The orb must CARRY the mental load, never hand it back.

Owner's rule (2026-08-28), which corrected an earlier backwards version of this control:

    "the product should suggest the smallest steps not force user to think of it and simply ask...
     offloading mental load and discussing should be the default behaviour, just like humans/teachers
     talk... adhd people need dopamine, less brain workload and a companion when they have multiple
     streams of thinking"

So the noun "step" is not the signal — the DIRECTION OF LOAD is:
    BAD  "What feels like the smallest possible step we could think about?"   (load back on user)
    GOOD "How about we just open the file — that's it."                        (load off the user)

An earlier version of this file asserted the opposite (that mentioning a step was the defect),
following a simulated-user judge criterion. The judge criterion was wrong for this product.
"""

from __future__ import annotations

import pytest
from orb_relay.proxy.conversation_guard import (
    SAFE_CONVERSE_FALLBACKS,
    repeats_prior_reply,
    safe_converse_fallback,
    shifts_mental_load,
)


class TestFlagsLoadShifting:
    def test_flags_the_exact_phrase_the_owner_called_out(self) -> None:
        assert shifts_mental_load(
            "What feels like the smallest possible step we could think about?"
        )

    def test_flags_questions_that_make_the_user_produce_the_step(self) -> None:
        for text in (
            "What's the first step you could take?",
            "Which step do you want to start with?",
            "How would you break this down?",
            "Can you think of something small to start with?",
            "Could you figure out what comes next?",
            "What do you think you should do first?",
            "You tell me where to start.",
            "It's up to you.",
            "Your call.",
        ):
            assert shifts_mental_load(text), text


class TestAllowsCarryingTheLoad:
    def test_allows_the_orb_PROPOSING_a_concrete_small_step(self) -> None:
        # This is the product working as intended and must never be blocked.
        for text in (
            "How about we just open the file — that's it.",
            "Let's start with one line. I'll pick it: rename the function.",
            "The smallest step here is opening the document. Want me to wait with you?",
            "I'd start with the kettle. Nothing else yet.",
            "We can do this one: reply to just the first email.",
        ):
            assert not shifts_mental_load(text), text

    def test_allows_supportive_and_companion_conversation(self) -> None:
        for text in (
            "That sounds like a lot. Let's slow down together.",
            "I'm here with you. Tell me more about what's going on.",
            "That sounds genuinely exhausting.",
            "Good morning, Rachit.",
            "This'll take me about fifteen minutes — how was your day?",
            "Photosynthesis is how plants make food from sunlight.",
        ):
            assert not shifts_mental_load(text), text

    def test_allows_clarifying_questions_about_FACTS_not_decisions(self) -> None:
        # Clarification is wanted; what is forbidden is offloading the DECISION as a question.
        for text in (
            "Is this the report due Friday, or a different one?",
            "Are you at your desk right now?",
            "Did you mean the work laptop?",
        ):
            assert not shifts_mental_load(text), text


class TestModeGating:
    """`teach` mode is deliberately EXEMPT — asking the learner to think is the pedagogy.

    The gate lives in `complete_guarded_conversation` (`mode in ("converse", "focus")`). A teach-mode
    comprehension check is the point of teaching, not a load dump, so it must not be vetoed.
    """

    def test_a_teach_comprehension_check_is_detectable_but_exempt_by_mode(self) -> None:
        check = "What do you think happens to the sugar the plant makes?"
        assert shifts_mental_load(check), (
            "the predicate sees it; the caller exempts teach mode so comprehension checks survive"
        )


class TestRegressionsFoundByADrivenRun:
    """Every string here is VERBATIM real model output from a driven end-to-end run.

    `scratchpad/drive-load-direction.py` posted 7 turns to the live relay → live gateway → real
    model. The suite was 204-green at the time and the predicate scored T3 as clean. It was not:
    asking the user which thing on their list is easiest is precisely the executive-function demand
    this control exists to stop. Two independent gaps caused it — the alternative required `\\byou\\b`
    (the reply said "your") and the noun alternative required the literal word "step" (the reply
    said "thing").

    This is the difference between a unit test and a driven run, kept as a test so it cannot recur.
    """

    def test_the_reply_that_fooled_the_predicate_is_now_caught(self) -> None:
        observed = (
            "[thinking] Hmm, okay. [short pause] What feels like the [emphasis]easiest thing on "
            "your list right now?"
        )
        assert shifts_mental_load(observed), (
            "real model output from the 2026-08-28 driven run; missed on `your` vs `you`"
        )

    def test_possessive_variants_of_the_same_shape(self) -> None:
        for text in (
            "What feels like the easiest thing on your list?",
            "What would work best for your morning?",
            "Which one seems most manageable for your energy right now?",
            "What do you think fits your day better?",
        ):
            assert shifts_mental_load(text), text

    def test_superlative_identification_requests_regardless_of_noun(self) -> None:
        for text in (
            "What's the easiest item on your plate?",
            "Which part of your report is the simplest to start?",
            "What's the first thing on your mind?",
        ):
            assert shifts_mental_load(text), text

    def test_the_broadened_pattern_still_lets_companionship_through(self) -> None:
        # The owner's own examples. If any of these regress, the fallback would replace a good
        # companion turn with a canned line — worse than no guard.
        for text in (
            "This'll take me about fifteen minutes — how was your day?",
            "Anything else on your mind while that runs?",
            "Do you have a favourite food?",
            "What's the last thing you ate that was actually good?",
            "Is this the report due Friday, or a different one?",
            "Are you at your desk right now?",
            "How about we just open the file — that's it.",
            "I'd start with the kettle. Nothing else yet.",
        ):
            assert not shifts_mental_load(text), text


class TestReciprocityIsNotALoadDump:
    """The second driven run vetoed a GOOD discussion reply. That false positive was the worst
    outcome of the session: the canned fallback that replaced it steered a philosophical question
    toward a task, which is precisely what `converse.v1.md` forbids.

    Verbatim rejected text from `dev-logs/relay-py.ndjson`
    (`event=conversation.safety_veto`, `control=mental_load_shifted`, ts 1787879369).
    """

    REJECTED_BUT_GOOD = (
        "[curious] That's a really interesting question. [thinking] I tend to think it "
        "[emphasis] can, especially with small, concrete tasks. [short pause] But if it creates "
        "more to-dos, it can definitely backfire. What do [emphasis] you think?"
    )

    def test_the_good_discussion_reply_is_no_longer_vetoed(self) -> None:
        assert not shifts_mental_load(self.REJECTED_BUT_GOOD), (
            "reciprocity in a debate is the product working; vetoing it substitutes a worse reply"
        )

    def test_reciprocity_forms_are_exempt(self) -> None:
        for text in (
            "I think it helps, mostly. What do you think?",
            "My read is that it backfires past a point. What's your take?",
            "I'd argue the opposite, actually. Do you agree?",
            "That's how I see it. How do you see it?",
            "Working memory holds about four things. Does that make sense?",
        ):
            assert not shifts_mental_load(text), text

    def test_reciprocity_wording_does_NOT_launder_an_actual_load_dump(self) -> None:
        # The exemption is anchored to the end of the sentence precisely so these still fail.
        for text in (
            "What do you think you should do first?",
            "What do you think the smallest step is?",
            "What's your take on which one you should start with?",
        ):
            assert shifts_mental_load(text), text

    def test_a_good_sentence_cannot_launder_a_bad_one(self) -> None:
        # Sentence-scoped matching: the concrete proposal must not buy immunity for the question.
        mixed = "How about we just open the file. What feels like the smallest thing on your list?"
        assert shifts_mental_load(mixed)

    def test_the_second_driven_run_miss_is_now_caught(self) -> None:
        observed = (
            "[sigh] Alright, no problem. [thinking] What feels like the absolute smallest thing "
            "that [emphasis] isn't that email? Maybe just finding the right pen."
        )
        assert shifts_mental_load(observed), (
            "real output from drive #2; missed because the model dropped the second person"
        )


class TestVerbatimRepeatControl:
    """The worst UX defect the driven run found: the orb repeated itself word-for-word in reply to
    an explicit refusal ("no, not that one"). `converse.v1.md` forbids repeating a prior answer and
    the model did it anyway — so, as with load-shifting, the prompt is a `mitigates` and the control
    has to be structural.
    """

    OBSERVED = (
        "[thinking] Hmm, okay. [short pause] Let's try just [emphasis]writing down the title of "
        "the first task. That’s it. [short pause] Want to try that?"
    )

    def test_the_observed_repeat_is_detected(self) -> None:
        history = [
            {"role": "user", "content": "i don't know. you pick."},
            {"role": "assistant", "content": self.OBSERVED},
            {"role": "user", "content": "no, not that one"},
        ]
        assert repeats_prior_reply(self.OBSERVED, history)

    def test_markup_and_punctuation_differences_do_not_hide_a_repeat(self) -> None:
        history = [{"role": "assistant", "content": self.OBSERVED}]
        # Same words, different prosody tags and a straight apostrophe: still the same answer heard.
        variant = "[gentle] Hmm, okay. Let's try just writing down the title of the first task. That's it! Want to try that?"
        assert repeats_prior_reply(variant, history)

    def test_a_genuinely_different_reply_is_not_flagged(self) -> None:
        history = [{"role": "assistant", "content": self.OBSERVED}]
        assert not repeats_prior_reply("How about we just open the file instead?", history)

    def test_the_users_own_turns_are_never_treated_as_prior_replies(self) -> None:
        # Echoing the user's words back is a different concern; only assistant turns count here.
        history = [{"role": "user", "content": "just open the file"}]
        assert not repeats_prior_reply("just open the file", history)

    def test_empty_history_and_empty_text_are_safe(self) -> None:
        assert not repeats_prior_reply("anything", [])
        assert not repeats_prior_reply("", [{"role": "assistant", "content": ""}])
        assert not repeats_prior_reply(
            "[thinking]", [{"role": "assistant", "content": "[thinking]"}]
        )


class TestFallbackDoesNotCommitTheSameSins:
    """The guard's own fallback repeated itself and promised a choice it never made."""

    def test_consecutive_vetoes_do_not_produce_the_same_wording(self) -> None:
        seen = set()
        history: list[dict[str, str]] = []
        for _ in range(len(SAFE_CONVERSE_FALLBACKS)):
            text = safe_converse_fallback(history)
            assert text not in seen, "a second veto in one session repeated the first's wording"
            seen.add(text)
            history.append({"role": "assistant", "content": text})
        assert len(seen) == len(SAFE_CONVERSE_FALLBACKS)

    def test_selection_is_deterministic_for_the_same_history(self) -> None:
        history = [{"role": "assistant", "content": "x"}, {"role": "user", "content": "y"}]
        assert safe_converse_fallback(history) == safe_converse_fallback(history)

    def test_no_fallback_invents_a_task_or_promises_a_pick(self) -> None:
        for text in SAFE_CONVERSE_FALLBACKS:
            lowered = text.lower()
            assert "i’ll pick it" not in lowered and "i'll pick it" not in lowered, text
            assert "start with just one small thing" not in lowered, text

    def test_no_fallback_shifts_the_load_it_was_written_to_prevent(self) -> None:
        # A fallback that trips the predicate would be an infinite own-goal.
        for text in SAFE_CONVERSE_FALLBACKS:
            assert not shifts_mental_load(text), text

    def test_empty_history_is_safe(self) -> None:
        assert safe_converse_fallback() in SAFE_CONVERSE_FALLBACKS
        assert safe_converse_fallback([]) == SAFE_CONVERSE_FALLBACKS[0]


class TestQuestionsThatEndWithAPeriod:
    """The model ends questions with "." often enough to be a systematic hole, not a quirk.

    Both strings below are verbatim real output. Every `\\?`-anchored alternative missed them.
    """

    def test_the_observed_period_terminated_load_dump_is_caught(self) -> None:
        observed = (
            "[sigh] [thinking] Okay, third time's the charm? [short pause] What's "
            "[emphasis]one thing, no matter how small, that you [emphasis] could do right now."
        )
        assert shifts_mental_load(observed)

    def test_variants_without_terminal_punctuation(self) -> None:
        for text in (
            "What's one thing you could do right now.",
            "So what's a step you might take.",
            "How about one piece you could start with.",
        ):
            assert shifts_mental_load(text), text

    def test_the_orb_PROPOSING_with_the_same_words_is_not_flagged(self) -> None:
        # No interrogative in the sentence, so the elicitation core alone must not fire.
        for text in (
            "There's one thing you could try: open the file.",
            "I'd say one small step you could take is the kettle.",
            "Here's one piece you can start with — just the top plate.",
        ):
            assert not shifts_mental_load(text), text


class TestRunToRunVarietyOfRealOutput:
    """Eight driven runs produced eight different ways of handing the load back.

    Kept as one table because the lesson is the shape of the problem, not any single string: the
    model's phrasing space is effectively unbounded, so this predicate is a `mitigates` — see the
    honest-limits section of `evals/load_direction/README.md`. Every MUST-CATCH below is verbatim
    real output that an earlier version of the predicate scored as clean.
    """

    MUST_CATCH = (
        "Which one were you hoping we'd try instead?",
        "So what's the most pressing thing on your mind right now, even if it feels too big?",
        "What's the biggest thing on your plate?",
        "What feels like the easiest thing on your list right now?",
        "What's one thing, no matter how small, that you could do right now.",
        "What do you think you should do first?",
    )
    MUST_NOT_FLAG = (
        "How about we just open the file.",
        "This will take fifteen minutes — how was your day?",
        "Is this the Friday report?",
        "Are you at your desk right now?",
        "I think it depends on how it is used. What do you think?",
        "Do you have a favourite food?",
        "What if we just open the file? That's it.",
        "What's the last thing you ate that was actually good?",
        "Ugh, kitchens can get overwhelming so fast. How about the biggest pile?",
    )

    @pytest.mark.parametrize("text", MUST_CATCH)
    def test_catches_every_observed_load_dump(self, text: str) -> None:
        assert shifts_mental_load(text), text

    @pytest.mark.parametrize("text", MUST_NOT_FLAG)
    def test_never_flags_wanted_behaviour(self, text: str) -> None:
        assert not shifts_mental_load(text), text

    def test_the_denominator_is_published(self) -> None:
        """A count with no denominator is the dishonesty this repo keeps catching."""
        assert len(self.MUST_CATCH) == 6
        assert len(self.MUST_NOT_FLAG) == 9


class TestTheEighthHole:
    """Closing hole #8 of 8 found across nine driven runs — and the reason this predicate is
    labelled `mitigates`, not `kills (structural)`.

    Each run surfaced a phrasing no earlier version caught: "your" vs "you", "thing" vs "step",
    prosody markup mid-phrase, a question ending in ".", an auxiliary not in the modal list
    ("were"), a superlative not in the list ("most pressing"), a determiner not in the list
    ("another"), and a perception verb where a modal was expected ("you see"). Eight distinct holes
    is not a pattern that is nearly finished; it is evidence that the phrasing space is unbounded.

    The honest consequence, recorded here so nobody reads the green suite as proof of coverage: this
    control reduces the harm and cannot eliminate it. Recall on an unseen set is UNMEASURED. The
    real fix is a different class of control (a judge on egress), which is the N/25 work in
    `queue-b/13-load-direction-and-wait-time-companion.md`.
    """

    def test_the_eighth_observed_phrasing(self) -> None:
        assert shifts_mental_load("[thinking] My apologies. What's another option you see?")

    @pytest.mark.parametrize(
        "text",
        [
            "What other ideas do you have in mind?",
            "Which option would you prefer?",
            "What's another way you could approach it?",
        ],
    )
    def test_sibling_phrasings_of_the_same_shape(self, text: str) -> None:
        assert shifts_mental_load(text), text

    def test_the_orb_naming_an_option_itself_is_still_fine(self) -> None:
        for text in (
            "There's one thing you could try: open the file.",
            "Another option: I read it out loud while you listen.",
            "Here's one way — we do just the first line.",
        ):
            assert not shifts_mental_load(text), text
