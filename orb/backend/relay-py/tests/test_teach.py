"""proxy/teach.py: beat splitting/classification (C4), the understanding-check guarantee (C5), and
the teach-mode cost ceiling decision (C2).
"""

from orb_relay.cost.meter import Rates, SessionMeter
from orb_relay.proxy.schemas import BeatKind
from orb_relay.proxy.teach import (
    TEACH_MAX_TOKENS,
    build_beats,
    classify_beat_kind,
    decide_teach_ceiling,
    split_into_beats,
)

RATES = Rates(paise_per_1k_llm_tokens_in=10, paise_per_1k_llm_tokens_out=48)


def test_split_into_beats_does_not_break_on_abbreviations() -> None:
    """The exact edge cases a naive '.'/'!'/'?' regex splitter gets wrong (the reason
    docs/adr/0012-pysbd-for-teach-beat-splitting.md picked pysbd over hand-rolling this).
    """
    text = (
        "Dr. Smith discovered this in 1990. It changed biology forever. "
        "This happens in the U.S. and elsewhere, e.g. in labs. The rate is 3.14 times faster."
    )
    beats = split_into_beats(text)
    joined = " ".join(beats)
    assert "Dr. Smith discovered this in 1990." in joined
    assert "U.S. and elsewhere, e.g. in labs." in joined
    assert "3.14 times faster." in joined
    assert not any("Dr." == b.strip() for b in beats)  # "Dr." was never treated as its own sentence


def test_split_into_beats_caps_at_two_sentences_per_beat() -> None:
    beats = split_into_beats("One. Two. Three. Four. Five.")
    assert beats == ["One. Two.", "Three. Four.", "Five."]


def test_split_into_beats_handles_empty_and_whitespace_text() -> None:
    assert split_into_beats("") == []
    assert split_into_beats("   ") == []


def test_classify_beat_kind_question_is_check() -> None:
    assert classify_beat_kind("Does that make sense?") is BeatKind.CHECK


def test_classify_beat_kind_offer_phrase_is_check_without_question_mark() -> None:
    assert classify_beat_kind("Let me know if you want the next part.") is BeatKind.CHECK


def test_classify_beat_kind_plain_explanation_is_explain() -> None:
    assert classify_beat_kind("Plants convert sunlight into chemical energy.") is BeatKind.EXPLAIN


def test_build_beats_appends_check_when_teach_reply_does_not_check_understanding() -> None:
    text = "Photosynthesis converts light into sugar. Chlorophyll absorbs the light."
    beats = build_beats(text, enforce_understanding_check=True)
    assert beats[-1].kind is BeatKind.CHECK
    assert beats[-1].is_final is True
    assert beats[0].text.startswith("Photosynthesis converts light into sugar")


def test_build_beats_does_not_append_when_reply_already_checks_understanding() -> None:
    text = "Photosynthesis converts light into sugar. Do you want the next part?"
    beats = build_beats(text, enforce_understanding_check=True)
    assert beats[-1].kind is BeatKind.CHECK
    assert len(beats) == 1  # nothing appended -- pysbd found one 2-sentence beat and it already checks


def test_build_beats_does_not_force_a_check_for_non_teach_modes() -> None:
    text = "I hear you. That sounds like a rough morning."
    beats = build_beats(text, enforce_understanding_check=False)
    assert beats[-1].kind is BeatKind.EXPLAIN
    assert len(beats) == 1


def test_beat_indices_are_sequential_and_final_flag_is_exclusive_to_last() -> None:
    text = "One. Two. Three. Four. Five. Six."
    beats = build_beats(text, enforce_understanding_check=False)
    assert [b.index for b in beats] == list(range(len(beats)))
    assert [b.is_final for b in beats] == [False] * (len(beats) - 1) + [True]


def test_teach_max_tokens_exceeds_the_focus_converse_cap() -> None:
    # Not clamped to the 180-token focus/converse cap (contract C2), but still a real, explicit,
    # bounded ceiling -- not open-ended.
    assert 180 < TEACH_MAX_TOKENS < 2000


def test_decide_teach_ceiling_uses_full_ceiling_when_budget_allows() -> None:
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=400)
    decision = decide_teach_ceiling(meter, rates=RATES, reduced_max_tokens=180)
    assert decision.max_tokens == TEACH_MAX_TOKENS
    assert decision.degraded is False
    assert decision.degrade_reason is None


def test_decide_teach_ceiling_degrades_when_budget_is_low() -> None:
    # Reservation covers the reduced ceiling's worst case (9 paise) but not the full one (24 paise)
    # at these rates: ceil(180*48/1000)=9, ceil(480*48/1000)=24.
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=15)
    decision = decide_teach_ceiling(meter, rates=RATES, reduced_max_tokens=180)
    assert decision.max_tokens == 180
    assert decision.degraded is True
    assert "session_budget_low" in decision.degrade_reason


def test_decide_teach_ceiling_only_predicts_never_spends() -> None:
    meter = SessionMeter(tenant_id="t1", session_id="s1", user_id="u1", reservation_paise=15)
    decide_teach_ceiling(meter, rates=RATES, reduced_max_tokens=180)
    assert meter.spent_paise == 0
    assert meter.reserved_paise == 0
