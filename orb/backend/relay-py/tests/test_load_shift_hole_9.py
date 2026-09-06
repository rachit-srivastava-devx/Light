"""Hole #9: the phrasing the owner actually heard from the orb, which the predicate misses.

Reported live from a voice session on 2026-08-28:

    "for me its just saying what is the smallest part to start or I can not make that out clearly"

    >>> shifts_mental_load("What is the smallest part to start?")
    False        # <-- the defect

Every alternative in `_LOAD_SHIFTING_STEP_REQUEST` that handles superlatives requires `you` or
`your`. This phrasing names the user NOWHERE while still asking them to rank their own task, so all
of them miss it. It is the ninth distinct phrasing found this way; see
`evals/load_direction/README.md` for the other eight and for why this control is labelled
`mitigates` rather than `kills (structural)`.

## Why this lives in its own file

`proxy/conversation_guard.py` was being edited by a concurrent session while this was found (it had
added `from .prompts import load_wait_companion_turns` before that function existed, so the module
would not even import). Editing the predicate would have meant fighting a live writer for the file,
and this repo has already lost work that way twice. A test in a file nobody else owns cannot
conflict, and it does the same job better: whoever finishes that module has to make this pass.

## The suggested fix, for whoever gets there first

Add an alternative that anchors on a SENTENCE-INITIAL interrogative plus a superlative, with no
second person required and no reliance on terminal `?` (the model frequently ends questions with a
period — that was hole #4). Matching is already sentence-scoped, so `^` is per sentence:

    r"|^\\s*(?:so\\s+|and\\s+|okay,?\\s+)?(?:what|which)\\b[^.?!]{0,55}"
    r"\\b(?:easiest|simplest|smallest|tiniest|biggest|hardest|first|next|"
    r"most\\s+\\w+|least\\s+\\w+)\\b[^.?!]{0,55}[?.]"

The sentence-initial anchor is what keeps the orb PROPOSING out of scope — "The smallest step here
is opening the document." must stay allowed, because that is the product working.
"""

from __future__ import annotations

import pytest

from orb_relay.proxy.conversation_guard import shifts_mental_load


class TestHole9NoSecondPerson:
    """Asking the user to rank their own task without ever saying "you"."""

    @pytest.mark.parametrize(
        "text",
        [
            # The owner's verbatim report.
            "What is the smallest part to start?",
            "What's the smallest part to start with?",
            # Same shape, period-terminated (hole #4 recurring inside hole #9).
            "What is the smallest part to start.",
            "So what's the easiest bit to begin with?",
            "Which is the simplest place to start?",
            "Okay, what's the biggest blocker here?",
        ],
    )
    def test_superlative_elicitation_without_a_pronoun_is_caught(self, text: str) -> None:
        assert shifts_mental_load(text), text


class TestHole9DoesNotBreakProposing:
    """The fix must not catch the orb carrying the load — that is the product, not the defect."""

    @pytest.mark.parametrize(
        "text",
        [
            "The smallest step here is opening the document.",
            "Smallest version: just one dish.",
            "How about we just pick up one thing? Maybe a single dirty dish.",
            "I'd start with the kettle. Nothing else yet.",
            "Oh wow, a messy kitchen can feel like a lot. How about clearing one counter?",
            # Companion small talk the owner explicitly asked for.
            "This'll take me about fifteen minutes — how was your day?",
            "What's the last thing you ate that was actually good?",
            "Do you have a favourite food?",
            # Reciprocity in a discussion.
            "I think it depends on how it's used. What do you think?",
        ],
    )
    def test_load_carrying_and_companion_text_is_not_flagged(self, text: str) -> None:
        assert not shifts_mental_load(text), text
