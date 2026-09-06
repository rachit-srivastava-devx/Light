/**
 * Session state + intervention -> (emotion, rate, energy). A deterministic lookup table.
 *
 * `docs/BUILD-DIGEST.md` §4: "prosody selection (state-derived lookup, model never picks tone)".
 * This is the product's top-priority surface — empathy here is a *system*, not a TTS flag.
 *
 * INV7's structural half lives here: shame-adjacent registers are not "avoided by the prompt",
 * they are **unrepresentable** — `OrbEmotion` does not contain them, so no code path, model
 * output, or config value can select one. That is the difference between a guarantee and a hope.
 */

import type { Intervention } from '../cognitive/contracts';
import type { SessionState } from '../session/contracts';

/**
 * The allowed emotional registers. Deliberately closed and deliberately small.
 *
 * Excluded by construction (INV7): disappointed, stern, urgent, admonishing, exasperated,
 * concerned-about-you. Every one of those reads as a parent or a boss to a user who has spent
 * their life being told they are lazy — which is the exact failure this product exists to avoid.
 * Adding one would require editing this union, which is a reviewable act, not an accident.
 */
export type OrbEmotion =
  | 'warm'
  | 'calm'
  | 'curious'
  | 'celebratory'
  | 'gentle'
  | 'matter_of_fact';

/** Speaking-rate multiplier against the voice's neutral baseline. */
export type SpeechRate = number;
export type Energy = 'low' | 'mid_low' | 'mid' | 'mid_high';

export interface ProsodyDirective {
  readonly emotion: OrbEmotion;
  readonly rate: SpeechRate;
  readonly energy: Energy;
}

/**
 * Per-state defaults. Rates come from blueprint doc 12 §5's table where it names them (CLARIFY is
 * 0.95x, "a question, not an interrogation"); the rest sit within ±10% of neutral, because a voice
 * that varies more than that stops sounding like one person.
 */
const BY_STATE: Readonly<Record<SessionState, ProsodyDirective>> = {
  IDLE_PRESENT: { emotion: 'calm', rate: 1.0, energy: 'low' },
  INTAKE: { emotion: 'warm', rate: 1.0, energy: 'mid' },
  CLARIFY: { emotion: 'curious', rate: 0.95, energy: 'mid_low' },
  STEP_PRESENT: { emotion: 'matter_of_fact', rate: 1.0, energy: 'mid' },
  WORKING: { emotion: 'warm', rate: 1.04, energy: 'mid' },
  CHECK_IN: { emotion: 'gentle', rate: 0.95, energy: 'mid_low' },
  STEP_DONE: { emotion: 'celebratory', rate: 1.05, energy: 'mid_high' },
  INTERRUPTED: { emotion: 'calm', rate: 1.0, energy: 'low' },
  SESSION_DONE: { emotion: 'warm', rate: 0.98, energy: 'mid_low' },
};

/**
 * Interventions that override the state's default, because the intervention *is* the message.
 * Interventions not listed keep the state's tone — a Suggest during WORKING should sound like
 * WORKING, not like an announcement.
 */
const BY_INTERVENTION: Partial<Record<Intervention, ProsodyDirective>> = {
  Celebrate: { emotion: 'celebratory', rate: 1.05, energy: 'mid_high' },
  // Pause and Escalate are the only interventions reachable at high Dysregulated (INV7), so their
  // tone is the one that matters most: gentle and slow, never urgent.
  Pause: { emotion: 'gentle', rate: 0.93, energy: 'low' },
  Escalate: { emotion: 'gentle', rate: 0.93, energy: 'low' },
  Redirect: { emotion: 'gentle', rate: 0.97, energy: 'mid_low' },
  Clarify: { emotion: 'curious', rate: 0.95, energy: 'mid_low' },
};

/**
 * @param state the session state the utterance is spoken in
 * @param intervention the intervention producing it, if any
 */
export function selectProsody(
  state: SessionState,
  intervention: Intervention | null,
): ProsodyDirective {
  if (intervention !== null) {
    const override = BY_INTERVENTION[intervention];
    if (override !== undefined) return override;
  }
  return BY_STATE[state];
}
