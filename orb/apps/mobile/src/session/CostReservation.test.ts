import { describe, expect, it } from 'vitest';
import {
  CostReservation,
  InvalidReservationError,
  NonIntegerPaiseError,
  ReservationExceededError,
  SESSION_RESERVATION_PAISE,
} from './CostReservation';

const ZERO_USAGE = { llm_tokens_in: 0, llm_tokens_out: 0, tts_chars_novel: 0, stt_seconds_billed: 0 };

describe('CostReservation construction', () => {
  it('defaults to SESSION_RESERVATION_PAISE (₹4 = 400 paise)', () => {
    const r = new CostReservation('s1', 'u1');
    expect(r.reservation_paise).toBe(SESSION_RESERVATION_PAISE);
    expect(SESSION_RESERVATION_PAISE).toBe(400);
    expect(r.remaining_paise).toBe(400);
  });

  it('rejects a non-integer reservation', () => {
    expect(() => new CostReservation('s1', 'u1', 100.5)).toThrow(InvalidReservationError);
  });

  it('rejects a zero or negative reservation', () => {
    expect(() => new CostReservation('s1', 'u1', 0)).toThrow(InvalidReservationError);
    expect(() => new CostReservation('s1', 'u1', -50)).toThrow(InvalidReservationError);
  });
});

describe('CostReservation.spend — integer paise only', () => {
  it('accepts an in-budget integer spend and decrements remaining_paise', () => {
    const r = new CostReservation('s1', 'u1', 400);
    const event = r.spend(150, { ...ZERO_USAGE, llm_tokens_in: 100 }, 'llm');
    expect(r.remaining_paise).toBe(250);
    expect(event.cost_paise).toBe(150);
    expect(event.remaining_paise).toBe(250);
    expect(event.tag).toEqual({ user: 'u1', session: 's1', line: 'llm' });
  });

  it('emits the §2 cost-metering event shape verbatim', () => {
    const r = new CostReservation('s1', 'u1', 400);
    const usage = {
      llm_tokens_in: 10,
      llm_tokens_out: 20,
      tts_chars_novel: 30,
      stt_seconds_billed: 5,
    };
    const event = r.spend(40, usage, 'voice');
    expect(event.usage).toEqual(usage);
    expect(event.tag.line).toBe('voice');
  });

  it('rejects a non-integer cost_paise', () => {
    const r = new CostReservation('s1', 'u1', 400);
    expect(() => r.spend(10.5, ZERO_USAGE, 'llm')).toThrow(NonIntegerPaiseError);
  });

  it('rejects a non-integer usage field', () => {
    const r = new CostReservation('s1', 'u1', 400);
    expect(() =>
      r.spend(10, { ...ZERO_USAGE, stt_seconds_billed: 1.2 }, 'voice'),
    ).toThrow(NonIntegerPaiseError);
  });

  it('rejects a negative cost_paise', () => {
    const r = new CostReservation('s1', 'u1', 400);
    expect(() => r.spend(-1, ZERO_USAGE, 'llm')).toThrow(NonIntegerPaiseError);
  });
});

describe('CostReservation — fails closed at the boundary (INV5)', () => {
  it('a spend exactly equal to remaining_paise is admitted', () => {
    const r = new CostReservation('s1', 'u1', 100);
    expect(() => r.spend(100, ZERO_USAGE, 'llm')).not.toThrow();
    expect(r.remaining_paise).toBe(0);
  });

  it('a spend exceeding remaining_paise is rejected BEFORE mutating state, not refunded after', () => {
    const r = new CostReservation('s1', 'u1', 100);
    r.spend(80, ZERO_USAGE, 'llm');
    expect(r.remaining_paise).toBe(20);

    expect(() => r.spend(21, ZERO_USAGE, 'llm')).toThrow(ReservationExceededError);
    // The rejected spend must not have been applied — remaining_paise is unchanged.
    expect(r.remaining_paise).toBe(20);
  });

  it('canSpend reports admissibility without mutating state', () => {
    const r = new CostReservation('s1', 'u1', 100);
    expect(r.canSpend(100)).toBe(true);
    expect(r.canSpend(101)).toBe(false);
    expect(r.remaining_paise).toBe(100); // canSpend is side-effect-free
  });

  it('repeated in-budget spends never push remaining_paise negative', () => {
    const r = new CostReservation('s1', 'u1', 100);
    r.spend(40, ZERO_USAGE, 'llm');
    r.spend(40, ZERO_USAGE, 'voice');
    expect(r.remaining_paise).toBe(20);
    expect(() => r.spend(30, ZERO_USAGE, 'llm')).toThrow(ReservationExceededError);
    expect(r.remaining_paise).toBe(20);
  });
});
