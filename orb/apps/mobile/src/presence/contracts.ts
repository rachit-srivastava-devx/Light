/**
 * Presence plane contract — the audio graph's node config and the interruption classification.
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §1 (NoiseEngine / AudioGraph / DuckingMixer /
 * InterruptionHandler), §2 ("Audio graph node config"), §3 (audio invariant numbers) and §5 (the
 * pause contract). This file imports nothing: the presence plane has zero dependency on cognitive
 * or voice (AGENTS.md invariant 1, lint-enforced by `tooling/boundary-lint.mjs`) — silence is the
 * one true outage (§5), so the bed must not be able to block on anything.
 */

/**
 * Seconds on the `AudioContext` clock, as passed to `start(when)` / `setTargetAtTime`. Deliberately
 * distinct from the session plane's wall-clock `EpochMs`: §1 requires all bed scheduling to run on
 * the audio clock because JS never touches the render loop.
 */
export type AudioTimeSeconds = number;

/** §3 — "30s noise buffer, 48kHz, crossfaded"; §1 — tail↔head crossfade ~250ms built at startup. */
export const BED_BUFFER_SECONDS = 30;
export const BED_SAMPLE_RATE_HZ = 48_000;
export const BED_CROSSFADE_MS = 250;

/** §1 DuckingMixer — bed gain ramps −18 → −30 dBFS (the −12dB duck of §3) over 80–150ms. */
export const BED_GAIN_NOMINAL_DBFS = -18;
export const BED_GAIN_DUCKED_DBFS = -30;
export const DUCK_RAMP_MIN_MS = 80;
export const DUCK_RAMP_MAX_MS = 150;

/** §3/§5 — a transient interruption must have the bed back within 300ms, behind a cover. */
export const TRANSIENT_RECOVERY_BUDGET_MS = 300;
/** §5 pause contract step 1 — the bed stops with a ~250ms fade, never abruptly. */
export const BED_FADE_MS = 250;

/** §2 — `AudioBufferSourceNode(noiseBuffer, loop=true)`. Never stopped or recreated (§1, INV1). */
export interface NoiseSourceConfig {
  readonly loop: true;
  readonly buffer_seconds: number;
  readonly sample_rate_hz: typeof BED_SAMPLE_RATE_HZ;
  readonly crossfade_ms: number;
}

export interface GainNodeConfig {
  /** dBFS. The mixer ramps this value; it never gates the source (INV1). */
  readonly gain_db: number;
}

/**
 * §2 — `BiquadFilterNode(lowpass, sweepable)`. The sweep is `ColorMorph.ts`'s brown↔pink status
 * channel (§1), so `type` is fixed at lowpass and only `frequency_hz` moves.
 */
export interface BiquadFilterConfig {
  readonly type: 'lowpass';
  readonly frequency_hz: number;
  readonly q: number;
}

/**
 * §2 — the whole graph:
 *   AudioBufferSourceNode(noise, loop) → Gain → Biquad(lowpass) → destination
 *   AudioBufferSourceNode(voice/cue)   → Gain → destination        [mixed over the bed]
 * The voice branch has no filter: it is mixed over the bed, not coloured with it.
 */
export interface AudioGraphConfig {
  readonly bed: {
    readonly source: NoiseSourceConfig;
    readonly gain: GainNodeConfig;
    readonly filter: BiquadFilterConfig;
  };
  readonly voice: { readonly gain: GainNodeConfig };
}

/**
 * §5 — the classification `InterruptionHandler.ts` exists to make. It decides between two
 * completely different recoveries, so it is the highest-stakes boolean in the presence plane:
 * a user-intent event misread as transient keeps the mic hot after the user locked the screen.
 */
export type InterruptionKind = 'transient' | 'user_intent';

/** §5 — "Transient interruptions (call, Siri, audio-stack reset)". */
export type TransientCause = 'incoming_call' | 'siri' | 'audio_stack_reset';

/**
 * §5 — "triggered by backgrounding, screen lock, Bluetooth/headphone disconnect, or another app
 * playing media (all 'user-intent')".
 */
export type UserIntentCause =
  | 'backgrounded'
  | 'manual_pause'
  | 'screen_locked'
  | 'bluetooth_disconnected'
  | 'headphone_disconnected'
  | 'other_app_media';

export type InterruptionEvent =
  | { readonly kind: 'transient'; readonly cause: TransientCause; readonly at: AudioTimeSeconds }
  | { readonly kind: 'user_intent'; readonly cause: UserIntentCause; readonly at: AudioTimeSeconds };

/**
 * §5 — the pause contract's five steps, in order. Ordered because the order is the contract: the
 * mic must be released (2) before the UI claims it is not listening (4).
 */
export const PAUSE_CONTRACT_SEQUENCE = [
  'bed_fade_out',
  'release_mic_at_os_level',
  'close_stt_socket',
  'show_paused_not_listening',
  'capture_nothing',
] as const;
export type PauseContractStep = (typeof PAUSE_CONTRACT_SEQUENCE)[number];

/**
 * §5 — "Resume is explicit (user taps) or automatic-with-cover for transient interruptions only;
 * never silently resumes on foreground return." The mapping is fixed: `user_intent` ⇒
 * `explicit_user_tap`, `transient` ⇒ `automatic_with_cover`.
 */
export type ResumeMode = 'explicit_user_tap' | 'automatic_with_cover';

/**
 * Signature only — `InterruptionHandler.ts` owns the body. Kept in the contract so the audio suite
 * (§6) can be written against the classification before the handler exists.
 */
export type InterruptionClassifier = (event: InterruptionEvent) => {
  readonly kind: InterruptionKind;
  readonly resume: ResumeMode;
  /**
   * §5 — an audio-stack death may exceed the 300ms budget and then needs a spoken cover line
   * rather than a silent recovery. True only for transient events.
   */
  readonly cover_required: boolean;
};
