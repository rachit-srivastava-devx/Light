/**
 * Procedural noise generation — pure signal processing, no audio runtime.
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §1 (leaky-integrator brown, Paul Kellet 7-coefficient
 * pink cascade, 30s buffer with ~250ms tail↔head crossfade) and §3 ("30s noise buffer, 48kHz,
 * crossfaded"). Deliberately free of any `react-native-audio-api` import: this file takes numbers
 * in and returns a number array out, so it is unit-testable without a native/audio runtime.
 *
 * No `Math.random`: white noise needs a source of randomness, but AGENTS.md invariant 6 requires
 * injected, replayable inputs everywhere in this codebase. `seed` stands in for "now" here — a
 * fixed seed must reproduce a bit-identical buffer.
 */

import { BED_BUFFER_SECONDS, BED_CROSSFADE_MS, BED_SAMPLE_RATE_HZ } from './contracts';

export type NoiseColor = 'brown' | 'pink';

export interface NoiseBuffer {
  readonly samples: Float64Array;
  readonly sample_rate_hz: number;
}

/** mulberry32 — small, seeded, deterministic PRNG. Not cryptographic; just replayable. */
export function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** White noise in [-1, 1], length samples, deterministic for a given seed. */
export function generateWhiteNoise(length: number, seed: number): Float64Array {
  const rand = seededRandom(seed);
  const out = new Float64Array(length);
  for (let i = 0; i < length; i++) {
    out[i] = rand() * 2 - 1;
  }
  return out;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * §1 — "leaky-integrator brown (`y = clamp(y·0.998 + w·0.02)`)". Constants are the digest's own
 * formula, quoted verbatim; clamp bound is [-1, 1], the buffer's amplitude domain.
 */
const BROWN_LEAK = 0.998;
const BROWN_WHITE_WEIGHT = 0.02;

export function brownNoise(white: Float64Array): Float64Array {
  const out = new Float64Array(white.length);
  let y = 0;
  for (let i = 0; i < white.length; i++) {
    y = clamp(y * BROWN_LEAK + (white[i] as number) * BROWN_WHITE_WEIGHT, -1, 1);
    out[i] = y;
  }
  return out;
}

/**
 * §1 — "Paul Kellet 7-coefficient pink cascade". Coefficients are the published constants of that
 * named algorithm (industry-standard pink noise filter via 7 leaky integrators, `b0`..`b6`), not
 * independently derived — kept as a single block rather than a config so the well-known algorithm
 * stays recognizable and checkable against its reference source.
 *
 * Output is clamped to [-1, 1] like brown. The cascade has no inherent bound: measured over 60
 * seeds × 300k samples the worst peak was 0.976 — inside range, but only ~0.2 dB of headroom, and
 * the 30 s bed buffer is 4.8× longer than that probe. An unclamped sample >1 would hard-clip in the
 * `AudioBuffer` as a click on a bed that is never allowed to draw attention to itself (INV1).
 */
export function pinkNoise(white: Float64Array): Float64Array {
  const out = new Float64Array(white.length);
  let b0 = 0;
  let b1 = 0;
  let b2 = 0;
  let b3 = 0;
  let b4 = 0;
  let b5 = 0;
  let b6 = 0;
  for (let i = 0; i < white.length; i++) {
    const w = white[i] as number;
    b0 = 0.99886 * b0 + w * 0.0555179;
    b1 = 0.99332 * b1 + w * 0.0750759;
    b2 = 0.969 * b2 + w * 0.153852;
    b3 = 0.8665 * b3 + w * 0.3104856;
    b4 = 0.55 * b4 + w * 0.5329522;
    b5 = -0.7616 * b5 - w * 0.016898;
    const pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + w * 0.5362;
    b6 = w * 0.115926;
    out[i] = clamp(pink * 0.11, -1, 1); // 0.11 = published normalization constant for this cascade
  }
  return out;
}

/**
 * §1 / blueprint 02 §2 — "the buffer's last ~250 ms is crossfaded into its first, so the wrap is
 * continuous in both amplitude and (for a stochastic signal) spectrum."
 *
 * Standard loop-crossfade construction, and the two properties it has to deliver are *amplitude*
 * and *spectrum* continuity — not merely `out[N-1] === out[0]`, which is a scalar that a reversed
 * or otherwise-degenerate blend can satisfy while leaving an audible cusp (see LESSONS #1).
 *
 * `raw` must be `outputLength + crossfadeSamples` long: the extra `crossfadeSamples` are the
 * generator's *natural continuation past the loop point*. We fold that overhang back onto the head:
 *
 *     out[j]  = raw[j]·wIn(t) + raw[outputLength + j]·wOut(t)   for j < crossfadeSamples
 *     out[j]  = raw[j]                                          otherwise
 *
 * At the wrap, `out[outputLength-1] = raw[outputLength-1]` is followed by
 * `out[0] = raw[outputLength]` — consecutive samples of one unbroken generator run, so value *and*
 * slope are continuous by construction rather than by assertion.
 *
 * Weights are equal-power (sin/cos), not linear: the head and the overhang are uncorrelated noise,
 * so a linear fade sums to ~0.707 amplitude mid-window and dips the bed ~3 dB every loop period —
 * exactly the 30 s periodicity blueprint 02 §11.4's soak FFT is written to catch.
 */
export function crossfadeLoop(raw: Float64Array, crossfadeSamples: number): Float64Array {
  const outputLength = raw.length - Math.max(0, crossfadeSamples);
  if (crossfadeSamples <= 0 || crossfadeSamples >= outputLength) {
    return Float64Array.from(raw.subarray(0, outputLength));
  }
  const out = Float64Array.from(raw.subarray(0, outputLength));
  const lastIndex = crossfadeSamples - 1;
  for (let j = 0; j < crossfadeSamples; j++) {
    const t = lastIndex > 0 ? j / lastIndex : 1;
    const wIn = Math.sin((Math.PI / 2) * t);
    const wOut = Math.cos((Math.PI / 2) * t);
    out[j] = (raw[j] as number) * wIn + (raw[outputLength + j] as number) * wOut;
  }
  return out;
}

/**
 * §1/§3 — builds the full 30s@48kHz crossfaded bed buffer for one colour. `seconds`/`sampleRateHz`/
 * `crossfadeMs` default to the contract constants; overridable only for tests.
 *
 * Generates `length + crossfadeSamples` samples so `crossfadeLoop` has the real continuation past
 * the loop point to fold back (see above). The returned buffer is exactly `length` samples — the
 * overhang is scratch, never played. Blueprint 02 §10 budgets "<15 MFLOP once at startup"; the
 * extra 250 ms of generation is ~0.8% on top of 30 s and stays well inside that.
 */
export function buildNoiseBuffer(
  color: NoiseColor,
  seed: number,
  seconds: number = BED_BUFFER_SECONDS,
  sampleRateHz: number = BED_SAMPLE_RATE_HZ,
  crossfadeMs: number = BED_CROSSFADE_MS,
): NoiseBuffer {
  const length = Math.round(seconds * sampleRateHz);
  const crossfadeSamples = Math.round((crossfadeMs / 1000) * sampleRateHz);
  const white = generateWhiteNoise(length + crossfadeSamples, seed);
  const raw = color === 'brown' ? brownNoise(white) : pinkNoise(white);
  const samples = crossfadeLoop(raw, crossfadeSamples);
  return { samples, sample_rate_hz: sampleRateHz };
}
