/**
 * Distinguishes the user genuinely interrupting the orb from a noise blip or their own backchannel.
 *
 * `docs/BUILD-DIGEST.md` §3: yield within 100ms, but require 200ms of sustained speech first —
 * those two numbers pull against each other on purpose. Yielding on the first audio frame would
 * make the orb flinch at a cough; waiting for certainty would make it talk over the user. 200ms of
 * voiced audio is the compromise the spec pins.
 *
 * Pure: the caller measures durations (AGENTS.md invariant 6).
 */

import {
  BARGE_IN_MIN_SPEECH_MS,
  BARGE_IN_YIELD_BUDGET_MS,
  VAD_SILENCE_PROBABILITY,
  VAD_SPEECH_PROBABILITY,
} from './contracts';

export interface BargeInInput {
  /** Is the orb currently speaking? No speech, nothing to interrupt. */
  readonly orb_is_speaking: boolean;
  /** Continuous voiced-audio duration detected from the mic, post-AEC. */
  readonly sustained_speech_ms: number;
  /**
   * True when the detected energy is echo of the orb's own output that AEC did not fully cancel.
   * Without this the orb interrupts itself — the failure mode that makes barge-in feel broken.
   */
  readonly is_echo_residue: boolean;
}

export type BargeInDecision =
  | { readonly yield: true }
  | {
      readonly yield: false;
      readonly reason: 'orb_silent' | 'too_brief' | 'echo_residue';
    };

export function decideBargeIn(input: BargeInInput): BargeInDecision {
  if (!input.orb_is_speaking) return { yield: false, reason: 'orb_silent' };
  if (input.is_echo_residue) return { yield: false, reason: 'echo_residue' };
  if (input.sustained_speech_ms < BARGE_IN_MIN_SPEECH_MS) {
    return { yield: false, reason: 'too_brief' };
  }
  return { yield: true };
}

export interface BargeInObservation extends Omit<BargeInInput, 'sustained_speech_ms'> {
  /** Monotonic timestamp from the native mic frame. */
  readonly at_ms: number;
  /** Post-platform-processing VAD confidence for this mic frame. */
  readonly speech_probability: number;
}

export interface BargeInCoordinator {
  observe(input: BargeInObservation): BargeInDecision;
  reset(): void;
}

export interface BargeInEffects {
  /** Retire the relay/provider generation. The implementation must return without awaiting it. */
  cancelProviderCall(budgetMs: number): void;
}

/**
 * Converts post-AEC mic frames into one cancellation edge.
 *
 * The 200ms onset is detection time, not part of the yield budget. Once detected, cancellation is
 * dispatched synchronously; the relay then polls its provider cancel flag every 25ms. Keeping the
 * effect injected makes the production transport and cancellation outcome independently observable.
 */
export function createBargeInCoordinator(effects: BargeInEffects): BargeInCoordinator {
  let candidateStartedAtMs: number | null = null;
  let cancellationSent = false;

  const reset = (): void => {
    candidateStartedAtMs = null;
    cancellationSent = false;
  };

  return {
    observe(input) {
      if (!input.orb_is_speaking) {
        reset();
        return decideBargeIn({ ...input, sustained_speech_ms: 0 });
      }
      if (input.is_echo_residue) {
        reset();
        return decideBargeIn({ ...input, sustained_speech_ms: 0 });
      }
      if (
        !Number.isFinite(input.at_ms) ||
        input.at_ms < 0 ||
        !Number.isFinite(input.speech_probability) ||
        input.speech_probability > 1
      ) {
        reset();
        return { yield: false, reason: 'too_brief' };
      }

      const minimumProbability =
        candidateStartedAtMs === null ? VAD_SPEECH_PROBABILITY : VAD_SILENCE_PROBABILITY;
      if (input.speech_probability < minimumProbability) {
        reset();
        return { yield: false, reason: 'too_brief' };
      }

      if (candidateStartedAtMs === null || input.at_ms < candidateStartedAtMs) {
        candidateStartedAtMs = input.at_ms;
      }
      const decision = decideBargeIn({
        ...input,
        sustained_speech_ms: input.at_ms - candidateStartedAtMs,
      });
      if (decision.yield && !cancellationSent) {
        cancellationSent = true;
        effects.cancelProviderCall(BARGE_IN_YIELD_BUDGET_MS);
      }
      return decision;
    },
    reset,
  };
}
