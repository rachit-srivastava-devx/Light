"""CostMeter: meter -> attribute -> cap -> alert (docs/BUILD-DIGEST.md §2/§3, C12).

Resolves the seam logged as docs/adr/LESSONS.md L4: AGENTS.md invariant 6 ("money is integer
paise, never float") governs *currency* only. Usage quantities get their own rule, applied here:

  - discrete units (tokens, characters) -> integer, >= 0
  - continuous units (billed seconds)   -> float, >= 0, finite

`stt_seconds_billed` is the field L4 named: real STT providers bill fractional seconds, so forcing
it to an integer would either over- or under-count every call. It is validated as continuous.

The cap is a *reservation*, not a post-hoc alert (INV5): a spend that would exceed the session's
reservation is refused before it happens. There is no refund path, because there is nothing to
refund — the spend never occurs.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field

from ..observability.devlog import dev_log

# §3: "per-session hard reservation e.g. Rs 4". The digest gives this as an example, not a pinned
# number, so it is the default and overridable per session rather than a module constant.
DEFAULT_SESSION_RESERVATION_PAISE = 400


from mutmut.mutation.trampoline import wrap_in_trampoline as _mutmut_mutated, MutantDict


class UsageValidationError(ValueError):
    """A usage quantity violated its unit's rule. A programmer error, not a runtime condition."""
mutants_xǁReservationExceededErrorǁ__init____mutmut: MutantDict = {}  # type: ignore


class ReservationExceededError(RuntimeError):
    """The spend would exceed the session reservation (INV5). Raised BEFORE the spend occurs."""

    @_mutmut_mutated(mutants_xǁReservationExceededErrorǁ__init____mutmut)
    def __init__(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id or '<unknown>'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.requested_paise = requested_paise
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_orig(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id or '<unknown>'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.requested_paise = requested_paise
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_1(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            None
        )
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.requested_paise = requested_paise
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_2(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id and '<unknown>'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.requested_paise = requested_paise
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_3(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id or 'XX<unknown>XX'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.requested_paise = requested_paise
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_4(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id or '<UNKNOWN>'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.requested_paise = requested_paise
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_5(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id or '<unknown>'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = None
        self.session_id = session_id
        self.requested_paise = requested_paise
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_6(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id or '<unknown>'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = tenant_id
        self.session_id = None
        self.requested_paise = requested_paise
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_7(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id or '<unknown>'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.requested_paise = None
        self.remaining_paise = remaining_paise

    def xǁReservationExceededErrorǁ__init____mutmut_8(
        self,
        session_id: str,
        requested_paise: int,
        remaining_paise: int,
        tenant_id: str | None = None,
    ) -> None:
        super().__init__(
            f"tenant {tenant_id or '<unknown>'}, session {session_id}: spend of {requested_paise} paise exceeds remaining "
            f"reservation of {remaining_paise} paise"
        )
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.requested_paise = requested_paise
        self.remaining_paise = None

mutants_xǁReservationExceededErrorǁ__init____mutmut['_mutmut_orig'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_orig # type: ignore # mutmut generated
mutants_xǁReservationExceededErrorǁ__init____mutmut['xǁReservationExceededErrorǁ__init____mutmut_1'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_1 # type: ignore # mutmut generated
mutants_xǁReservationExceededErrorǁ__init____mutmut['xǁReservationExceededErrorǁ__init____mutmut_2'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_2 # type: ignore # mutmut generated
mutants_xǁReservationExceededErrorǁ__init____mutmut['xǁReservationExceededErrorǁ__init____mutmut_3'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_3 # type: ignore # mutmut generated
mutants_xǁReservationExceededErrorǁ__init____mutmut['xǁReservationExceededErrorǁ__init____mutmut_4'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_4 # type: ignore # mutmut generated
mutants_xǁReservationExceededErrorǁ__init____mutmut['xǁReservationExceededErrorǁ__init____mutmut_5'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_5 # type: ignore # mutmut generated
mutants_xǁReservationExceededErrorǁ__init____mutmut['xǁReservationExceededErrorǁ__init____mutmut_6'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_6 # type: ignore # mutmut generated
mutants_xǁReservationExceededErrorǁ__init____mutmut['xǁReservationExceededErrorǁ__init____mutmut_7'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_7 # type: ignore # mutmut generated
mutants_xǁReservationExceededErrorǁ__init____mutmut['xǁReservationExceededErrorǁ__init____mutmut_8'] = ReservationExceededError.xǁReservationExceededErrorǁ__init____mutmut_8 # type: ignore # mutmut generated
mutants_x__check_discrete__mutmut: MutantDict = {}  # type: ignore


@_mutmut_mutated(mutants_x__check_discrete__mutmut)
def _check_discrete(name: str, value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(value).__name__}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_discrete__mutmut_orig(name: str, value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(value).__name__}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_discrete__mutmut_1(name: str, value: int) -> None:
    if isinstance(value, bool) and not isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(value).__name__}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_discrete__mutmut_2(name: str, value: int) -> None:
    if isinstance(value, bool) or isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(value).__name__}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_discrete__mutmut_3(name: str, value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise UsageValidationError(None)
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_discrete__mutmut_4(name: str, value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(None).__name__}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_discrete__mutmut_5(name: str, value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(value).__name__}")
    if value <= 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_discrete__mutmut_6(name: str, value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(value).__name__}")
    if value < 1:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_discrete__mutmut_7(name: str, value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(value).__name__}")
    if value < 0:
        raise UsageValidationError(None)

mutants_x__check_discrete__mutmut['_mutmut_orig'] = x__check_discrete__mutmut_orig # type: ignore # mutmut generated
mutants_x__check_discrete__mutmut['x__check_discrete__mutmut_1'] = x__check_discrete__mutmut_1 # type: ignore # mutmut generated
mutants_x__check_discrete__mutmut['x__check_discrete__mutmut_2'] = x__check_discrete__mutmut_2 # type: ignore # mutmut generated
mutants_x__check_discrete__mutmut['x__check_discrete__mutmut_3'] = x__check_discrete__mutmut_3 # type: ignore # mutmut generated
mutants_x__check_discrete__mutmut['x__check_discrete__mutmut_4'] = x__check_discrete__mutmut_4 # type: ignore # mutmut generated
mutants_x__check_discrete__mutmut['x__check_discrete__mutmut_5'] = x__check_discrete__mutmut_5 # type: ignore # mutmut generated
mutants_x__check_discrete__mutmut['x__check_discrete__mutmut_6'] = x__check_discrete__mutmut_6 # type: ignore # mutmut generated
mutants_x__check_discrete__mutmut['x__check_discrete__mutmut_7'] = x__check_discrete__mutmut_7 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut: MutantDict = {}  # type: ignore


@_mutmut_mutated(mutants_x__check_continuous__mutmut)
def _check_continuous(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_orig(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_1(name: str, value: float) -> None:
    if isinstance(value, bool) and not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_2(name: str, value: float) -> None:
    if isinstance(value, bool) or isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_3(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(None)
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_4(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(None).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_5(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_6(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(None):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_7(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(None)
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_8(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value <= 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_9(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 1:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def x__check_continuous__mutmut_10(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(None)

mutants_x__check_continuous__mutmut['_mutmut_orig'] = x__check_continuous__mutmut_orig # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_1'] = x__check_continuous__mutmut_1 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_2'] = x__check_continuous__mutmut_2 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_3'] = x__check_continuous__mutmut_3 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_4'] = x__check_continuous__mutmut_4 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_5'] = x__check_continuous__mutmut_5 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_6'] = x__check_continuous__mutmut_6 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_7'] = x__check_continuous__mutmut_7 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_8'] = x__check_continuous__mutmut_8 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_9'] = x__check_continuous__mutmut_9 # type: ignore # mutmut generated
mutants_x__check_continuous__mutmut['x__check_continuous__mutmut_10'] = x__check_continuous__mutmut_10 # type: ignore # mutmut generated


@dataclass(frozen=True)
class UsageDelta:
    """One unit of billable work (§2's cost-metering event shape).

    Field units, per the rule above: the three counts are discrete; billed seconds is continuous.
    """

    llm_tokens_in: int = 0
    llm_tokens_out: int = 0
    tts_chars_novel: int = 0
    stt_seconds_billed: float = 0.0

    def __post_init__(self) -> None:
        _check_discrete("llm_tokens_in", self.llm_tokens_in)
        _check_discrete("llm_tokens_out", self.llm_tokens_out)
        _check_discrete("tts_chars_novel", self.tts_chars_novel)
        _check_continuous("stt_seconds_billed", self.stt_seconds_billed)


@dataclass(frozen=True)
class Rates:
    """Paise per unit. Integers so pricing itself is exact; the *product* of a rate and a
    continuous quantity is rounded up at the point of charge (see `price_paise`).

    Defaults are 0 so a test or a T0 run meters volume without inventing prices — §3's real
    provider prices are USD-denominated and belong in deployment config, not a source constant.
    """

    paise_per_1k_llm_tokens_in: int = 0
    paise_per_1k_llm_tokens_out: int = 0
    paise_per_1k_tts_chars: int = 0
    paise_per_stt_minute: int = 0
mutants_x_price_paise__mutmut: MutantDict = {}  # type: ignore


@_mutmut_mutated(mutants_x_price_paise__mutmut)
def price_paise(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_orig(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_1(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = None
    return math.ceil(total)


def x_price_paise__mutmut_2(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000 - delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_3(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000 - delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_4(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000 - delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_5(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in * 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_6(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in / rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_7(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1001
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_8(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out * 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_9(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out / rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_10(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1001
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_11(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars * 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_12(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel / rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_13(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1001
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_14(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute * 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_15(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed / rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


def x_price_paise__mutmut_16(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 61
    )
    return math.ceil(total)


def x_price_paise__mutmut_17(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, always rounded UP (`math.ceil`).

    Rounding up rather than to-nearest is deliberate: this number gates a spend guard, and a
    guard that systematically under-counts lets a session drift past its reservation by exactly
    the accumulated rounding error. Over-counting by at most 1 paise per charge is the safe
    direction for a cap.
    """
    total = (
        delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in / 1000
        + delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out / 1000
        + delta.tts_chars_novel * rates.paise_per_1k_tts_chars / 1000
        + delta.stt_seconds_billed * rates.paise_per_stt_minute / 60
    )
    return math.ceil(None)

mutants_x_price_paise__mutmut['_mutmut_orig'] = x_price_paise__mutmut_orig # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_1'] = x_price_paise__mutmut_1 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_2'] = x_price_paise__mutmut_2 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_3'] = x_price_paise__mutmut_3 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_4'] = x_price_paise__mutmut_4 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_5'] = x_price_paise__mutmut_5 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_6'] = x_price_paise__mutmut_6 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_7'] = x_price_paise__mutmut_7 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_8'] = x_price_paise__mutmut_8 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_9'] = x_price_paise__mutmut_9 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_10'] = x_price_paise__mutmut_10 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_11'] = x_price_paise__mutmut_11 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_12'] = x_price_paise__mutmut_12 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_13'] = x_price_paise__mutmut_13 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_14'] = x_price_paise__mutmut_14 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_15'] = x_price_paise__mutmut_15 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_16'] = x_price_paise__mutmut_16 # type: ignore # mutmut generated
mutants_x_price_paise__mutmut['x_price_paise__mutmut_17'] = x_price_paise__mutmut_17 # type: ignore # mutmut generated


@dataclass
class SessionMeter:
    """Per-tenant/session reservation ledger. In-memory by design (C7: runs at T0 on Rs 0 infra); the
    aggregate that survives a restart is `@pe/cost-control-plane`'s job via the sidecar, and a
    session that outlives this process has already lost its audio anyway (§5's pause contract).
    """

    tenant_id: str
    session_id: str
    user_id: str
    reservation_paise: int = DEFAULT_SESSION_RESERVATION_PAISE
    spent_paise: int = 0
    reserved_paise: int = 0
    ledger: list[tuple[str, int]] = field(default_factory=list)

    def __post_init__(self) -> None:
        _check_discrete("reservation_paise", self.reservation_paise)

    @property
    def remaining_paise(self) -> int:
        return self.reservation_paise - self.spent_paise

    def would_exceed(self, delta: UsageDelta, rates: Rates) -> bool:
        return price_paise(delta, rates) > self.remaining_paise

    def charge(self, line: str, delta: UsageDelta, rates: Rates) -> int:
        """Records a spend, or refuses it. Returns the paise charged.

        `line` is the attribution tag from §2's event shape ('voice' or 'llm'), kept as a plain
        string because the eval/cost harness groups by it and a new line (e.g. 'embed') must not
        require a code change here to be metered.
        """
        cost = price_paise(delta, rates)
        if cost > self.remaining_paise:
            raise ReservationExceededError(self.session_id, cost, self.remaining_paise)
        self.spent_paise += cost
        self.ledger.append((line, cost))
        return cost

    def reserve_remaining(self) -> int:
        """Hold the complete remaining reservation before paid work starts.

        The caller settles or releases this hold after the gateway returns. Holding the complete
        balance makes admission atomic for concurrent requests sharing a tenant/session and avoids
        discovering exhaustion only after the atomizer has already spent provider quota.
        """
        available = self.remaining_paise - self.reserved_paise
        if available <= 0:
            raise ReservationExceededError(self.session_id, 1, max(available, 0), self.tenant_id)
        self.reserved_paise += available
        return available

    def release(self, reserved_paise: int) -> None:
        if reserved_paise < 0 or reserved_paise > self.reserved_paise:
            raise ValueError("invalid reservation release")
        self.reserved_paise -= reserved_paise

    def settle(self, line: str, reserved_paise: int, delta: UsageDelta, rates: Rates) -> int:
        """Convert a pre-call hold into actual spend and release unused capacity."""
        if reserved_paise < 0 or reserved_paise > self.reserved_paise:
            raise ValueError("invalid reservation settlement")
        cost = price_paise(delta, rates)
        if cost > reserved_paise:
            dev_log(
                "cost.reservation_exceeded",
                level="error",
                tenant_id=self.tenant_id,
                session_id=self.session_id,
                line=line,
                cost_paise=cost,
                reserved_paise=reserved_paise,
            )
            raise ReservationExceededError(self.session_id, cost, reserved_paise, self.tenant_id)
        self.reserved_paise -= reserved_paise
        self.spent_paise += cost
        self.ledger.append((line, cost))
        dev_log(
            "cost.settled",
            tenant_id=self.tenant_id,
            session_id=self.session_id,
            line=line,
            cost_paise=cost,
            spent_paise=self.spent_paise,
            remaining_paise=self.remaining_paise,
        )
        return cost
