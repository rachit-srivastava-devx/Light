/**
 * Decides when to play a cached "mm-hmm" while the user is still talking.
 *
 * `docs/BUILD-DIGEST.md` §1: 100–200ms cached clips on 250–600ms mid-utterance pauses, at most 2
 * per utterance, never consecutive, never in the first 3s, and never at high Emotional Load.
 *
 * The last rule is the one that matters most and is easiest to get wrong: backchannelling at
 * someone who is distressed reads as dismissive, not attentive. It is a hard suppression, not a
 * weighting.
 *
 * Pure decision logic — playback is the audio graph's job.
 */

import {
  BACKCHANNEL_MAX_PER_UTTERANCE,
  BACKCHANNEL_PAUSE_MAX_MS,
  BACKCHANNEL_PAUSE_MIN_MS,
  BACKCHANNEL_SUPPRESS_FIRST_MS,
} from './contracts';

/** §2's belief threshold above which backchannelling is suppressed outright. */
export const EMOTIONAL_LOAD_SUPPRESS_ABOVE = 0.7;

export interface BackchannelState {
  /** How many have fired in this utterance. Reset on utterance start. */
  readonly played_this_utterance: number;
  /** True if the immediately preceding decision was to play (blocks consecutive fires). */
  readonly last_decision_was_play: boolean;
}

export interface BackchannelInput {
  readonly pause_ms: number;
  readonly utterance_elapsed_ms: number;
  /** 0..1, from the belief vector's EmotionalLoad. */
  readonly emotional_load: number;
  readonly state: BackchannelState;
}

export type BackchannelDecision =
  | { readonly play: true }
  | {
      readonly play: false;
      readonly reason:
        | 'pause_too_short'
        | 'pause_too_long'
        | 'too_early_in_utterance'
        | 'budget_spent'
        | 'would_be_consecutive'
        | 'emotional_load_high';
    };

export function decideBackchannel(input: BackchannelInput): BackchannelDecision {
  const { pause_ms, utterance_elapsed_ms, emotional_load, state } = input;

  // Checked first: no other condition can override it (§1 — hard suppression).
  if (emotional_load > EMOTIONAL_LOAD_SUPPRESS_ABOVE) {
    return { play: false, reason: 'emotional_load_high' };
  }
  if (utterance_elapsed_ms < BACKCHANNEL_SUPPRESS_FIRST_MS) {
    return { play: false, reason: 'too_early_in_utterance' };
  }
  if (state.played_this_utterance >= BACKCHANNEL_MAX_PER_UTTERANCE) {
    return { play: false, reason: 'budget_spent' };
  }
  if (state.last_decision_was_play) {
    return { play: false, reason: 'would_be_consecutive' };
  }
  if (pause_ms < BACKCHANNEL_PAUSE_MIN_MS) {
    return { play: false, reason: 'pause_too_short' };
  }
  // A pause longer than the window is not a thinking pause — it is an endpoint, and the
  // endpointer owns it. Backchannelling into it would talk over the orb's own reply.
  if (pause_ms > BACKCHANNEL_PAUSE_MAX_MS) {
    return { play: false, reason: 'pause_too_long' };
  }
  return { play: true };
}

/** Applies a decision to the state. Separate from `decideBackchannel` so the decision stays pure. */
export function advanceState(
  state: BackchannelState,
  decision: BackchannelDecision,
): BackchannelState {
  return {
    played_this_utterance: state.played_this_utterance + (decision.play ? 1 : 0),
    last_decision_was_play: decision.play,
  };
}

export const INITIAL_BACKCHANNEL_STATE: BackchannelState = {
  played_this_utterance: 0,
  last_decision_was_play: false,
};
