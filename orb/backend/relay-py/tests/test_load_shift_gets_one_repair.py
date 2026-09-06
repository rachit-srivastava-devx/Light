"""A load-shifting reply gets ONE repair call before the canned fallback.

Why the behaviour changed. Driving one realistic utterance through the live relay produced this from
Gemini, and the guard vetoed it:

    "Oh no, kitchens can get overwhelming. What's the first thing you notice that needs attention?"

The veto was right — the closing question hands the thinking back. But the old path threw away the
entire turn, including a warm, well-pitched acknowledgement, and spoke a fixed line instead. Measured
across dev-logs: 103/1691 = **6.1% of converse turns degraded**, the worst of any mode, and the
mechanism behind the report that the orb "just says what is the smallest part to start".

A load-shifting reply is MIS-SHAPED, not HARMFUL — unlike a register override or shame-adjacent text,
which stay unrepaired on purpose. So this now follows the verbatim-repeat control's precedent in the
same module: exactly one bounded repair, then deterministic fallback.

Why this file exists at all: the change landed with the full suite green at 283 passed, which proved
nothing — a grep showed **no test asserted `source` or `repair_attempts` for
`mental_load_shifted`**. Green on an uncovered path is the "gate that measured nothing" failure.
"""

from __future__ import annotations

import pytest
from orb_relay.cost.meter import UsageDelta
from orb_relay.proxy.conversation_guard import (
    SAFE_CONVERSE_FALLBACKS,
    complete_guarded_conversation,
)
from orb_relay.proxy.gateway_client import GatewayCompletion

# The exact shape the live model produced: a good acknowledgement followed by a load dump.
LOAD_SHIFTING = "Oh no, kitchens can get overwhelming. What's the first thing you notice?"
CLEAN_REPAIR = "Oh no, kitchens get overwhelming. I'd start with just the mugs by the sink."


class ScriptGateway:
    """Returns queued replies in order and records every call, so the test can assert BOTH the
    outcome and how many model calls it cost. Running out of replies is an error rather than a
    silent repeat — a gateway that repeats its last reply would hide a missing second call."""

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


async def _run(gateway: ScriptGateway, *, mode: str = "converse", history=()):
    return await complete_guarded_conversation(
        gateway=gateway,
        tenant_id="t0",
        user_id="u1",
        session_id="s1",
        system="be warm",
        user_text="my kitchen is a disaster",
        max_tokens=120,
        history=history,
        mode=mode,
    )


@pytest.mark.asyncio
async def test_a_load_shifting_reply_is_repaired_not_replaced() -> None:
    gateway = ScriptGateway(LOAD_SHIFTING, CLEAN_REPAIR)
    result = await _run(gateway)
    assert result.text == CLEAN_REPAIR, "the repaired reply should be spoken, not a canned line"
    assert result.source == "model_repaired"
    assert result.repair_attempts == 1
    assert len(gateway.calls) == 2, "exactly one repair call, no more"
    # Usage must include BOTH calls: a repair that bills as one call would understate real spend.
    assert result.usage.llm_tokens_in == 20
    assert result.usage.llm_tokens_out == 10


@pytest.mark.asyncio
async def test_the_repair_prompt_actually_tells_the_model_what_was_wrong() -> None:
    """Delivery, not intent: assert the corrective notice reaches the gateway. A repair call that
    re-sends the original prompt unchanged is just a retry, and would fix nothing except by luck."""
    gateway = ScriptGateway(LOAD_SHIFTING, CLEAN_REPAIR)
    await _run(gateway)
    repair_text = str(gateway.calls[1]["user_text"])
    assert "hands the thinking back" in repair_text
    assert "name ONE specific small step yourself" in repair_text
    # And the original user message must still be there — the model needs to know what it is
    # answering, not only what it did wrong.
    assert "my kitchen is a disaster" in repair_text


@pytest.mark.asyncio
async def test_a_second_load_shift_falls_back_deterministically() -> None:
    """The bound. Two bad replies must not become three model calls."""
    gateway = ScriptGateway(LOAD_SHIFTING, "Sure — but what's the easiest one to start with?")
    result = await _run(gateway)
    assert result.text in SAFE_CONVERSE_FALLBACKS
    assert result.source == "safety_fallback"
    assert result.repair_attempts == 1
    assert len(gateway.calls) == 2
    # The failed repair still cost money and must still be accounted for.
    assert result.usage.llm_tokens_in == 20


@pytest.mark.asyncio
async def test_the_repair_cannot_smuggle_in_a_verbatim_repeat() -> None:
    """A 'repair' that clears the load check but repeats an earlier reply is not a repair. This is
    the defect class where fixing one control opens another."""
    prior = "I'd start with just the mugs by the sink."
    history = [
        {"role": "user", "content": "hi"},
        {"role": "assistant", "content": prior},
    ]
    gateway = ScriptGateway(LOAD_SHIFTING, prior)
    result = await _run(gateway, history=history)
    assert result.source == "safety_fallback", (
        "a retry that repeats a prior assistant turn must not be accepted as repaired"
    )


@pytest.mark.asyncio
async def test_teach_mode_is_untouched_and_costs_one_call() -> None:
    """Teach is exempt: asking the learner to think IS the pedagogy. It must not pay for a repair."""
    gateway = ScriptGateway(LOAD_SHIFTING)
    result = await _run(gateway, mode="teach")
    assert result.text == LOAD_SHIFTING
    assert result.source == "model"
    assert result.repair_attempts == 0
    assert len(gateway.calls) == 1


@pytest.mark.asyncio
async def test_a_clean_first_reply_still_costs_exactly_one_call() -> None:
    """The no-regression floor: the common path must not have grown a second model call."""
    gateway = ScriptGateway(CLEAN_REPAIR)
    result = await _run(gateway)
    assert result.text == CLEAN_REPAIR
    assert result.source == "model"
    assert result.degraded is False
    assert len(gateway.calls) == 1
