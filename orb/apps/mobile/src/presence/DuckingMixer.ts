/**
 * Ramps bed gain when voice plays over it, per `docs/BUILD-DIGEST.md` §2/§3: "Ramps bed gain
 * −18→−30 dBFS over 80–150ms when voice plays; never stops/recreates the bed source node."
 *
 * This module only ever touches the already-built `bedGain.gain` `AudioParam` — it has no
 * reference to the source node, so it structurally cannot stop or recreate it (INV1).
 */

import {
  BED_GAIN_DUCKED_DBFS,
  BED_GAIN_NOMINAL_DBFS,
  DUCK_RAMP_MAX_MS,
  DUCK_RAMP_MIN_MS,
} from './contracts';
import type { AudioTimeSeconds } from './contracts';
import { dbfsToLinearGain } from './AudioGraph';
import type { AudioParamLike } from './AudioGraph';

function clampRampMs(rampMs: number): number {
  return Math.min(DUCK_RAMP_MAX_MS, Math.max(DUCK_RAMP_MIN_MS, rampMs));
}

/**
 * Blueprint 02 §4 names the primitive explicitly: "the bed's `GainNode` is ramped from nominal
 * (−18 dBFS) to a duck level (−30 dBFS) with `setTargetAtTime` over ~80–150 ms — **scheduled**, so
 * the ramp is smooth even if the JS thread hiccups." Time constant is `rampMs/3`, so the move is
 * ~95% complete by `rampMs` (same convention as `ColorMorph.morphTo`).
 *
 * `setTargetAtTime` starts from the param's *actual* current value, which is the reason the
 * blueprint picks it: duck and restore overlap constantly (back-to-back TTS chunks), and any
 * `setValueAtTime(assumedCurrent, …)` before the ramp would snap the gain from wherever the
 * previous ramp really was to the assumed value — an audible click on a bed that must never draw
 * attention to itself (INV1). There is deliberately no `currentDb` parameter: the caller cannot
 * know the in-flight value, and with this primitive it does not need to.
 *
 * Returns the time by which the move is ~complete, for scheduling the voice that follows.
 */
export function duckBed(
  gain: AudioParamLike,
  atSeconds: AudioTimeSeconds,
  rampMs: number = DUCK_RAMP_MAX_MS,
): AudioTimeSeconds {
  const clampedMs = clampRampMs(rampMs);
  gain.setTargetAtTime(dbfsToLinearGain(BED_GAIN_DUCKED_DBFS), atSeconds, clampedMs / 1000 / 3);
  return atSeconds + clampedMs / 1000;
}

/** §4 — restores the bed to nominal (−18 dBFS) once voice stops, same primitive and window. */
export function restoreBed(
  gain: AudioParamLike,
  atSeconds: AudioTimeSeconds,
  rampMs: number = DUCK_RAMP_MAX_MS,
): AudioTimeSeconds {
  const clampedMs = clampRampMs(rampMs);
  gain.setTargetAtTime(dbfsToLinearGain(BED_GAIN_NOMINAL_DBFS), atSeconds, clampedMs / 1000 / 3);
  return atSeconds + clampedMs / 1000;
}
