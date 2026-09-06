export type StimulationVisualState =
  | 'booting'
  | 'listening'
  | 'thinking'
  | 'working'
  | 'success'
  | 'paused'
  | 'error';

export type StimulationPhase = 'baseline' | 'engage' | 'recover' | 'reward';

export interface StimulationModel {
  readonly level: number;
  readonly phase: StimulationPhase;
  readonly cyclePositionMs: number;
}

/** Voice register selected by the bounded stimulation policy. */
export type StimulationVoiceEmotion = 'calm' | 'gentle' | 'warm' | 'curious' | 'matter_of_fact' | 'upbeat' | 'celebratory';

/** A bounded novelty cycle; this is UI/voice arousal, not medical dopamine dosing. */
export const STIMULATION_CYCLE_MS = 45_000;
export const STIMULATION_ENGAGE_MS = 7_000;
export const STIMULATION_RECOVER_MS = 17_000;

const BASE_LEVEL_BY_STATE: Readonly<Record<StimulationVisualState, number>> = {
  booting: 0.3,
  listening: 0.62,
  thinking: 0.68,
  working: 0.52,
  success: 0.96,
  paused: 0.16,
  error: 0.28,
};

function clamp(value: number): number {
  return Math.max(0, Math.min(value, 1));
}

function positiveModulo(value: number, modulus: number): number {
  return ((value % modulus) + modulus) % modulus;
}

export function stimulationForState(
  state: StimulationVisualState,
  elapsedMs: number,
): StimulationModel {
  const base = BASE_LEVEL_BY_STATE[state];
  const cyclePositionMs = positiveModulo(elapsedMs, STIMULATION_CYCLE_MS);

  if (state === 'success') {
    return { level: base, phase: 'reward', cyclePositionMs };
  }
  if (state === 'booting' || state === 'paused' || state === 'error') {
    return { level: base, phase: 'baseline', cyclePositionMs };
  }
  if (cyclePositionMs < STIMULATION_ENGAGE_MS) {
    return { level: clamp(base + 0.18), phase: 'engage', cyclePositionMs };
  }
  if (cyclePositionMs < STIMULATION_RECOVER_MS) {
    return { level: clamp(base + 0.04), phase: 'recover', cyclePositionMs };
  }
  return { level: base, phase: 'baseline', cyclePositionMs };
}

/**
 * Applies the stimulation phase to a state-derived voice register.
 *
 * The policy deliberately never raises a calm/gentle safety response into an upbeat one. During
 * a short engage window, ordinary task speech gets a brighter Fish prosody; recovery and baseline
 * return to the envelope's normal register. This is arousal shaping, not a claim about changing a
 * user's neurotransmitters.
 */
export function voiceEmotionForStimulation(
  baseEmotion: StimulationVoiceEmotion | string,
  stimulation: StimulationModel,
): string {
  if (baseEmotion === 'calm' || baseEmotion === 'gentle') return baseEmotion;
  if (stimulation.phase === 'reward') return 'celebratory';
  if (stimulation.phase === 'engage') return 'upbeat';
  return baseEmotion;
}
