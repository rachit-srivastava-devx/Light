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
import threading
from dataclasses import dataclass, field
from fractions import Fraction

from ..observability.devlog import dev_log

# §3: "per-session hard reservation e.g. Rs 4". The digest gives this as an example, not a pinned
# number, so it is the default and overridable per session rather than a module constant.
DEFAULT_SESSION_RESERVATION_PAISE = 400


class UsageValidationError(ValueError):
    """A usage quantity violated its unit's rule. A programmer error, not a runtime condition."""


class ReservationExceededError(RuntimeError):
    """The spend would exceed the session reservation (INV5). Raised BEFORE the spend occurs."""

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


def _check_discrete(name: str, value: int) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise UsageValidationError(f"{name} must be an int (discrete unit), got {type(value).__name__}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


def _check_continuous(name: str, value: float) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise UsageValidationError(f"{name} must be numeric (continuous unit), got {type(value).__name__}")
    if not math.isfinite(value):
        raise UsageValidationError(f"{name} must be finite, got {value}")
    if value < 0:
        raise UsageValidationError(f"{name} must be >= 0, got {value}")


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


def price_paise(delta: UsageDelta, rates: Rates) -> int:
    """Integer paise, rounded UP once after exact rational accumulation.

    Counts and integer rates become exact fractions directly. The billed-seconds float is first
    converted to its exact integer ratio by ``Fraction``; only then is it combined with a money
    rate. No currency value is represented as float. Rounding once at the paise boundary avoids
    both under-counting and the accumulated over-count that per-line ceiling would introduce.
    """
    total = (
        Fraction(delta.llm_tokens_in * rates.paise_per_1k_llm_tokens_in, 1000)
        + Fraction(delta.llm_tokens_out * rates.paise_per_1k_llm_tokens_out, 1000)
        + Fraction(delta.tts_chars_novel * rates.paise_per_1k_tts_chars, 1000)
        + Fraction(delta.stt_seconds_billed) * rates.paise_per_stt_minute / 60
    )
    return math.ceil(total)


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
    _lock: threading.RLock = field(
        default_factory=threading.RLock, init=False, repr=False, compare=False
    )

    def __post_init__(self) -> None:
        _check_discrete("reservation_paise", self.reservation_paise)

    @property
    def remaining_paise(self) -> int:
        with self._lock:
            return self.reservation_paise - self.spent_paise

    def would_exceed(self, delta: UsageDelta, rates: Rates) -> bool:
        with self._lock:
            return price_paise(delta, rates) > self.remaining_paise

    def charge(self, line: str, delta: UsageDelta, rates: Rates) -> int:
        """Records a spend, or refuses it. Returns the paise charged.

        `line` is the attribution tag from §2's event shape ('voice' or 'llm'), kept as a plain
        string because the eval/cost harness groups by it and a new line (e.g. 'embed') must not
        require a code change here to be metered.
        """
        cost = price_paise(delta, rates)
        with self._lock:
            if cost > self.remaining_paise:
                raise ReservationExceededError(
                    self.session_id, cost, self.remaining_paise, self.tenant_id
                )
            self.spent_paise += cost
            self.ledger.append((line, cost))
            return cost

    def reserve_remaining(self) -> int:
        """Hold the complete remaining reservation before paid work starts.

        The caller settles or releases this hold after the gateway returns. Holding the complete
        balance makes admission atomic for concurrent requests sharing a tenant/session and avoids
        discovering exhaustion only after the atomizer has already spent provider quota.
        """
        with self._lock:
            available = self.remaining_paise - self.reserved_paise
            if available <= 0:
                raise ReservationExceededError(
                    self.session_id, 1, max(available, 0), self.tenant_id
                )
            self.reserved_paise += available
            return available

    def release(self, reserved_paise: int) -> None:
        with self._lock:
            if reserved_paise < 0 or reserved_paise > self.reserved_paise:
                raise ValueError("invalid reservation release")
            self.reserved_paise -= reserved_paise

    def settle(self, line: str, reserved_paise: int, delta: UsageDelta, rates: Rates) -> int:
        """Atomically convert a hold to spend, or reject it and release the hold."""
        cost = price_paise(delta, rates)
        with self._lock:
            if reserved_paise < 0 or reserved_paise > self.reserved_paise:
                raise ValueError("invalid reservation settlement")

            # A settlement owns this hold exactly once. Consume it in the same critical section as
            # either recording spend or rejecting an overrun, so no observer can see the failed
            # post-call state as both unspent and still reserved.
            self.reserved_paise -= reserved_paise
            if cost <= reserved_paise:
                self.spent_paise += cost
                self.ledger.append((line, cost))

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
