/**
 * CostReservation — per-session hard token/char budget (INV5: per-session cost reservation, cannot
 * exceed).
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §3 ("per-session hard reservation e.g. ₹4") and §2 (the
 * cost-metering event shape: `{llm_tokens_in, llm_tokens_out, tts_chars_novel,
 * stt_seconds_billed}`, tagged `{user, session, line}`, "checked against a hard per-session
 * reservation before any spend").
 *
 * AGENTS.md invariant 6: money is integer paise, never float. All arithmetic below is on integers;
 * a non-integer paise amount is a caller bug and throws rather than silently rounding (rounding
 * would be exactly the kind of float-adjacent fudge invariant 6 exists to rule out).
 */

/** §3 — "per-session hard reservation e.g. ₹4". 1 rupee = 100 paise. */
export const SESSION_RESERVATION_PAISE = 400;

/** §2 — the cost-metering event's four billable quantities, before any per-unit pricing is applied. */
export interface UsageDelta {
  readonly llm_tokens_in: number;
  readonly llm_tokens_out: number;
  readonly tts_chars_novel: number;
  readonly stt_seconds_billed: number;
}

/** §2 — `tagged {user, session, line: 'voice'|'llm'}`. */
export type CostLine = 'voice' | 'llm';

/** §2 cost-metering event shape, emitted on every accepted spend. */
export interface CostMeterEvent {
  readonly usage: UsageDelta;
  readonly tag: { readonly user: string; readonly session: string; readonly line: CostLine };
  /** Integer paise actually charged for this event. */
  readonly cost_paise: number;
  /** Integer paise remaining in the reservation after this event. */
  readonly remaining_paise: number;
}

/** Thrown by the constructor when given a non-integer or negative reservation. */
export class InvalidReservationError extends Error {
  constructor(detail: string) {
    super(`CostReservation: invalid reservation — ${detail}`);
    this.name = 'InvalidReservationError';
  }
}

/** Thrown by `spend()` when a caller passes a non-integer paise amount — never silently rounded. */
export class NonIntegerPaiseError extends Error {
  constructor(detail: string) {
    super(`CostReservation: paise amounts must be integers — ${detail}`);
    this.name = 'NonIntegerPaiseError';
  }
}

/**
 * A spend that would exceed the remaining reservation, rejected BEFORE it happens (fails closed —
 * INV5). The caller must not have already performed the underlying spend (billed the provider,
 * synthesized the audio, etc.) when it calls `spend()`; this class has no refund path by design.
 */
export class ReservationExceededError extends Error {
  constructor(
    readonly requested_paise: number,
    readonly remaining_paise: number,
  ) {
    super(
      `CostReservation: spend of ${requested_paise} paise exceeds remaining ${remaining_paise} paise — rejected before spend`,
    );
    this.name = 'ReservationExceededError';
  }
}

function assertIntegerPaise(value: number, label: string): void {
  if (!Number.isInteger(value)) {
    throw new NonIntegerPaiseError(`${label} = ${value} is not an integer`);
  }
  if (value < 0) {
    throw new NonIntegerPaiseError(`${label} = ${value} is negative`);
  }
}

/**
 * Per-session hard budget, integer paise only. `spend(cost_paise, usage, line)` fails closed: if
 * the requested spend would exceed what's left, it throws and the caller must not perform the
 * underlying spend at all. There is no `refund`/`rollback` method — fail-closed means the check
 * happens before the spend, not a post-hoc reversal after an over-spend already happened.
 *
 * KNOWN SEAM (Opus review 2026-08-04, Tier-7 `CostMeter.ts`): `UsageDelta.stt_seconds_billed` is
 * currently asserted integer by `assertIntegerPaise`, which is the *money* rule (AGENTS.md
 * invariant 6) applied to a duration. STT providers bill fractional seconds, so Tier 7 must either
 * relax this field to non-negative-finite or define the rounding policy explicitly — and fix the
 * error message, which says "paise" about a field that is not paise.
 */
export class CostReservation {
  private spent_paise: number;
  readonly session_id: string;
  readonly user_id: string;
  readonly reservation_paise: number;

  constructor(session_id: string, user_id: string, reservation_paise: number = SESSION_RESERVATION_PAISE) {
    if (!Number.isInteger(reservation_paise)) {
      throw new InvalidReservationError(
        `reservation_paise must be an integer (paise, never float), got ${reservation_paise}`,
      );
    }
    if (reservation_paise <= 0) {
      throw new InvalidReservationError(`reservation_paise must be > 0, got ${reservation_paise}`);
    }
    this.session_id = session_id;
    this.user_id = user_id;
    this.reservation_paise = reservation_paise;
    this.spent_paise = 0;
  }

  get remaining_paise(): number {
    return this.reservation_paise - this.spent_paise;
  }

  /**
   * Attempt to reserve `cost_paise` against the budget for a usage delta already priced by the
   * caller. Throws `ReservationExceededError` and leaves the reservation untouched if the spend
   * would exceed `remaining_paise` — call this BEFORE performing the billable action (the provider
   * call, the TTS synthesis), never after.
   */
  spend(cost_paise: number, usage: UsageDelta, line: CostLine): CostMeterEvent {
    assertIntegerPaise(cost_paise, 'cost_paise');
    for (const [key, value] of Object.entries(usage)) {
      assertIntegerPaise(value, `usage.${key}`);
    }

    if (cost_paise > this.remaining_paise) {
      throw new ReservationExceededError(cost_paise, this.remaining_paise);
    }

    this.spent_paise += cost_paise;

    return {
      usage,
      tag: { user: this.user_id, session: this.session_id, line },
      cost_paise,
      remaining_paise: this.remaining_paise,
    };
  }

  /** `true` iff a spend of exactly `cost_paise` would be admitted right now — a side-effect-free
   * check callers can use to decide whether to attempt the underlying billable action at all. */
  canSpend(cost_paise: number): boolean {
    assertIntegerPaise(cost_paise, 'cost_paise');
    return cost_paise <= this.remaining_paise;
  }
}
