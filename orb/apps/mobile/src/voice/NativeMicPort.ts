/**
 * JS port onto the native `OrbMic` module (`android/app/src/main/java/com/orbmobile/OrbMicModule.kt`).
 *
 * Owns three things, deliberately kept separate from `VADGate.ts` (the pure state machine) and
 * `RelayClient.ts` (the transport): permission acquisition, the native event subscription, and
 * turning a raw PCM frame into the `VadFrame` shape `VADGate.ts` already defines.
 *
 * VAD classification is behind a small pluggable `VadClassifier` port (see below), not a single
 * hardcoded formula. Three implementations exist:
 *   - `createRmsVadClassifier()` — the original single-band linear-RMS heuristic. Kept, unchanged,
 *     as the explicit, named, always-available fallback (`source: 'heuristic-rms'`).
 *   - `createMultiFeatureVadClassifier()` — the new default. Still a classical DSP heuristic, NOT a
 *     model, but it replaces one feature (RMS against a fixed threshold pair) with three (an
 *     adaptive noise floor, zero-crossing rate, spectral flatness) specifically to stop a constant
 *     loud background (HVAC, a fan) from reading as speech, and to give quiet/far-field speech a
 *     chance via shape rather than loudness alone (`source: 'heuristic-multifeature'`).
 *   - `createSileroVadClassifier()` — the real fix (silero-vad, MIT, see the ADR). Throws a typed,
 *     named error: this tree has neither the `onnxruntime-react-native` dependency nor the native
 *     linking nor the model asset, and none of those are ownable from this file. The factory exists
 *     so the seam is concrete, not just prose — swapping it in later is a one-line change here.
 *
 * Whichever classifier is active, `startCapture` stamps its `source` onto every `VadFrame` it emits
 * (see `VadFrame.vad_source` in `./VADGate`) and logs once, at capture start, which one is active and
 * why. This is the "detectable, never silent" requirement: a fallback that only a code reader could
 * discover is not detectable. Full architecture rationale, latency measurements, and the dependency
 * this still needs: `docs/adr/0013-vad-and-endpointing-architecture.md`.
 */

import { noopDevLogger, type DevLogger } from '../shared/DevLogger';
import type { VadFrame, VadSource } from './VADGate';

/** 16-bit PCM full-scale magnitude, used to normalize RMS into a 0..1 probability. */
const PCM16_FULL_SCALE = 32_768;
/**
 * RMS (as a fraction of full scale) below this reads as silence (probability 0). Chosen well
 * above typical emulator/device electrical noise floor so idle mic input does not register as
 * speech.
 */
const SILENCE_RMS_FLOOR = 0.01;
/** RMS at or above this reads as confident speech (probability 1). */
const SPEECH_RMS_CEILING = 0.12;

/**
 * Maps a PCM frame's loudness to a 0..1 "speech probability" via linear RMS interpolation
 * between `SILENCE_RMS_FLOOR` and `SPEECH_RMS_CEILING`. This is an energy heuristic, not a
 * classifier — it will treat any sufficiently loud non-speech sound as "speech", and it will
 * treat any sufficiently quiet speech as silence. Kept as the explicit, named fallback
 * (`heuristic-rms`, via `createRmsVadClassifier`) rather than deleted: a fallback that
 * disappears from the codebase cannot be selected deliberately when something upstream of it
 * breaks, and this product's rule is that a degradation must be a labelled choice, never a
 * silent one.
 */
export function energyToSpeechProbability(pcm: Int16Array): number {
  if (pcm.length === 0) return 0;
  let sumSquares = 0;
  for (let i = 0; i < pcm.length; i++) {
    const sample = pcm[i] ?? 0;
    sumSquares += sample * sample;
  }
  const rms = Math.sqrt(sumSquares / pcm.length) / PCM16_FULL_SCALE;
  if (rms <= SILENCE_RMS_FLOOR) return 0;
  if (rms >= SPEECH_RMS_CEILING) return 1;
  return (rms - SILENCE_RMS_FLOOR) / (SPEECH_RMS_CEILING - SILENCE_RMS_FLOOR);
}

// ---------------------------------------------------------------------------------------------
// Pluggable classifier port.
// ---------------------------------------------------------------------------------------------

export interface VadClassification {
  readonly speech_probability: number;
  readonly source: VadSource;
}

/**
 * A VAD classifier reduces one PCM frame (and its own running state, if any) to a probability.
 * Deterministic by construction: every implementation here is a pure fold over its inputs (no
 * wall-clock, no RNG), so replaying the same frame sequence reproduces the same probabilities —
 * required for the control-path determinism rule this product holds everywhere else.
 */
export interface VadClassifier<S> {
  readonly source: VadSource;
  readonly initialState: S;
  classify(state: S, pcm: Int16Array, sampleRateHz: number): { readonly state: S; readonly result: VadClassification };
}

/** Stateless wrapper around the original RMS heuristic, conforming to the same port. */
export function createRmsVadClassifier(): VadClassifier<null> {
  return {
    source: 'heuristic-rms',
    initialState: null,
    classify(state, pcm) {
      return {
        state,
        result: { speech_probability: energyToSpeechProbability(pcm), source: 'heuristic-rms' },
      };
    },
  };
}

// ---------------------------------------------------------------------------------------------
// Multi-feature classifier: adaptive noise floor + zero-crossing rate + spectral flatness.
//
// None of this is a neural VAD. It is a classical, deterministic, dependency-free signal-processing
// heuristic — a real, measurable improvement on a single linear RMS threshold, but explicitly not
// the G-R2-recommended fix (silero-vad). See the module doc and the ADR for why silero could not be
// wired in this session, and what precisely is missing.
// ---------------------------------------------------------------------------------------------

export interface NoiseFloorState {
  /** Adaptive estimate of the ambient/background RMS level, same units as `energyToSpeechProbability`. */
  readonly floor_rms: number;
  readonly frames_seen: number;
}

export const INITIAL_NOISE_FLOOR_STATE: NoiseFloorState = {
  floor_rms: SILENCE_RMS_FLOOR,
  frames_seen: 0,
};

/** Slow: a sustained loud background needs ~1-2s of consistently elevated RMS before the floor rises to meet it — long enough that a normal utterance cannot itself drag the floor up mid-sentence. */
const NOISE_FLOOR_RISE_ALPHA = 0.02;
/** Fast: once the room quiets back down, the floor should recover within a handful of frames, not seconds. */
const NOISE_FLOOR_FALL_ALPHA = 0.2;
/** The floor may adapt below the static default for a genuinely quiet room, but never to (near) zero — a guard band against pure digital-silence quantization noise. */
const NOISE_FLOOR_MIN = SILENCE_RMS_FLOOR * 0.25;
/** The floor may never rise so high it would make genuinely loud speech unreachable. */
const NOISE_FLOOR_MAX = SPEECH_RMS_CEILING * 0.9;

/**
 * Asymmetric EMA noise-floor tracker (a simplified "minimum statistics" estimator): rises slowly
 * toward a new, sustained higher ambient level (absorbing a constant HVAC hum/fan after a couple of
 * seconds), falls quickly toward a new lower one (recovering fast when the room quiets). Pure
 * reducer — deterministic given the RMS sequence, no clock read.
 */
export function updateNoiseFloor(state: NoiseFloorState, rms: number): NoiseFloorState {
  if (!Number.isFinite(rms) || rms < 0) return state;
  const alpha = rms > state.floor_rms ? NOISE_FLOOR_RISE_ALPHA : NOISE_FLOOR_FALL_ALPHA;
  const next = state.floor_rms + alpha * (rms - state.floor_rms);
  const clamped = Math.min(NOISE_FLOOR_MAX, Math.max(NOISE_FLOOR_MIN, next));
  return { floor_rms: clamped, frames_seen: state.frames_seen + 1 };
}

function computeRms(pcm: Int16Array): number {
  if (pcm.length === 0) return 0;
  let sumSquares = 0;
  for (let i = 0; i < pcm.length; i++) {
    const sample = pcm[i] ?? 0;
    sumSquares += sample * sample;
  }
  return Math.sqrt(sumSquares / pcm.length) / PCM16_FULL_SCALE;
}

/** Fraction of adjacent-sample sign changes, 0..1 (1.0 = alternating sign every sample = Nyquist). */
function computeZeroCrossingRate(pcm: Int16Array): number {
  if (pcm.length < 2) return 0;
  let crossings = 0;
  for (let i = 1; i < pcm.length; i++) {
    const prev = pcm[i - 1] ?? 0;
    const curr = pcm[i] ?? 0;
    if (prev >= 0 !== curr >= 0) crossings++;
  }
  return crossings / (pcm.length - 1);
}

function nextPowerOfTwo(n: number): number {
  let p = 1;
  while (p < n) p *= 2;
  return p;
}

/**
 * Adversarial-input guard: a huge frame must not blow up FFT cost. Analysis (ZCR, spectral
 * flatness) runs on at most this many samples (energy/RMS still runs over the full frame, which is
 * cheap and O(n)); at 16kHz this is 128ms — comfortably wider than any real 20-40ms capture frame,
 * so real frames are never truncated, only pathological/synthetic ones.
 */
const MAX_ANALYSIS_SAMPLES = 2_048;

/**
 * Standard iterative radix-2 Cooley-Tukey FFT, in place, on same-length real/imaginary arrays whose
 * length is a power of two. No external dependency: G-R2 flags hand-rolled *VAD/silence detection*
 * and hand-rolled *WAV parsing* as defects with a named library fix, but does not cover a textbook
 * FFT, and no pure-JS FFT is already a dependency of this tree. Correctness is asserted in
 * `NativeMicPort.test.ts` via a Parseval energy-conservation check, not just spot-checked bins.
 */
function fftInPlace(re: Float64Array, im: Float64Array): void {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; (j & bit) !== 0; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) {
      const tr = re[i] ?? 0;
      re[i] = re[j] ?? 0;
      re[j] = tr;
      const ti = im[i] ?? 0;
      im[i] = im[j] ?? 0;
      im[j] = ti;
    }
  }
  for (let len = 2; len <= n; len <<= 1) {
    const half = len / 2;
    const ang = (-2 * Math.PI) / len;
    const wr = Math.cos(ang);
    const wi = Math.sin(ang);
    for (let i = 0; i < n; i += len) {
      let curWr = 1;
      let curWi = 0;
      for (let k = 0; k < half; k++) {
        const evenRe = re[i + k] ?? 0;
        const evenIm = im[i + k] ?? 0;
        const oddRe = re[i + k + half] ?? 0;
        const oddIm = im[i + k + half] ?? 0;
        const vRe = oddRe * curWr - oddIm * curWi;
        const vIm = oddRe * curWi + oddIm * curWr;
        re[i + k] = evenRe + vRe;
        im[i + k] = evenIm + vIm;
        re[i + k + half] = evenRe - vRe;
        im[i + k + half] = evenIm - vIm;
        const nextWr = curWr * wr - curWi * wi;
        const nextWi = curWr * wi + curWi * wr;
        curWr = nextWr;
        curWi = nextWi;
      }
    }
  }
}

export interface PowerSpectrum {
  /** Power at each bin 0..n/2-1 (bin 0 is DC). Windowed (Hann), zero-padded to a power of two. */
  readonly bins: Float64Array;
  /** FFT length used (a power of two >= the analysed sample count). */
  readonly n: number;
  /** Samples actually windowed before the FFT (<= `MAX_ANALYSIS_SAMPLES`, <= `pcm.length`). */
  readonly analysisLength: number;
}

/**
 * Windows (Hann), zero-pads to the next power of two, and FFTs one frame. Exported separately from
 * `computeSpectralFlatness` so the FFT itself has an independently testable surface: correctness is
 * asserted in `NativeMicPort.test.ts` via a Parseval energy-conservation check and a known-bin-peak
 * check on a pure tone, not just spot-checked through the derived flatness scalar.
 */
export function computePowerSpectrum(pcm: Int16Array): PowerSpectrum {
  const analysisLength = Math.min(pcm.length, MAX_ANALYSIS_SAMPLES);
  const n = nextPowerOfTwo(Math.max(analysisLength, 1));
  const re = new Float64Array(n);
  const im = new Float64Array(n);
  for (let i = 0; i < analysisLength; i++) {
    const w = 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / Math.max(1, analysisLength - 1));
    re[i] = (pcm[i] ?? 0) * w;
  }
  fftInPlace(re, im);
  const half = n / 2;
  const bins = new Float64Array(half);
  for (let k = 0; k < half; k++) {
    const r = re[k] ?? 0;
    const imagPart = im[k] ?? 0;
    bins[k] = r * r + imagPart * imagPart;
  }
  return { bins, n, analysisLength };
}

/**
 * Spectral flatness (Wiener entropy): geometric mean / arithmetic mean of the power spectrum, in
 * (0, 1]. Near 1 = flat/broadband (fan hiss, white-noise-like); near 0 = peaky (a tone/hum, or
 * speech's formant structure) — see `zcrPlausibilityFactor` for how the tonal-hum case is told
 * apart from speech. The DC bin is excluded.
 */
export function computeSpectralFlatness(pcm: Int16Array): number {
  const { bins, analysisLength } = computePowerSpectrum(pcm);
  if (analysisLength < 2 || bins.length < 2) return 0;
  let logSum = 0;
  let sum = 0;
  let count = 0;
  for (let k = 1; k < bins.length; k++) {
    const power = (bins[k] ?? 0) + 1e-12;
    logSum += Math.log(power);
    sum += power;
    count++;
  }
  if (count === 0 || sum <= 0) return 0;
  const geoMean = Math.exp(logSum / count);
  const arithMean = sum / count;
  const flatness = geoMean / arithMean;
  return Number.isFinite(flatness) ? Math.min(1, Math.max(0, flatness)) : 0;
}

const DEFAULT_SAMPLE_RATE_HZ = 16_000;

function energyScore(rms: number, floorRms: number): number {
  const effectiveFloor = Math.max(SILENCE_RMS_FLOOR * 0.5, floorRms);
  const above = rms - effectiveFloor;
  if (above <= 0) return 0;
  const span = Math.max(1e-6, SPEECH_RMS_CEILING - effectiveFloor);
  return Math.min(1, above / span);
}

/**
 * A fully flat spectrum (broadband noise) multiplies the energy score down to 30%, never to zero —
 * heavy clipping can also flatten a spectrum, and this is a soft prior, not a hard gate. Hand-set,
 * not fit to data: the same honesty this file's neighbours already use for their thresholds.
 */
function noiseSuppressionFactor(flatness: number): number {
  const clamped = Math.min(1, Math.max(0, flatness));
  return 1 - 0.7 * clamped;
}

/**
 * Converts the fractional zero-crossing rate to an approximate dominant frequency (zcr=1.0 <=>
 * alternating every sample <=> Nyquist) and down-weights the two bands least consistent with human
 * speech: at/below mains-hum range (a tone/hum), and above the bulk of speech energy (hiss/whine).
 * A soft prior (floor 0.4-0.5), not a gate, for the same reason as `noiseSuppressionFactor`.
 */
function zcrPlausibilityFactor(zcr: number, sampleRateHz: number): number {
  const approxDominantHz = (zcr * sampleRateHz) / 2;
  if (approxDominantHz < 70) return 0.4;
  if (approxDominantHz > 4_000) return 0.5;
  return 1;
}

export interface MultiFeatureFeatures {
  readonly rms: number;
  readonly zero_crossing_rate: number;
  readonly spectral_flatness: number;
  readonly floor_rms: number;
}

export interface MultiFeatureClassification extends VadClassification {
  readonly source: 'heuristic-multifeature';
  readonly features: MultiFeatureFeatures;
}

/**
 * The new default classifier. Combines an adaptive-floor energy score with a bounded "shape trust"
 * factor (average, not product, of the flatness and ZCR priors — two imperfect independent cues
 * should not compound into over-aggressive joint suppression when they disagree) so no single
 * feature can unilaterally zero out a loud, clearly-present sound; the adaptive floor remains the
 * primary mechanism for a *sustained* background, and the shape terms catch a non-stationary loud
 * burst the floor has not adapted to yet.
 *
 * Explicitly NOT a claim of parity with silero-vad or any learned model — see the module doc.
 */
export function classifyMultiFeature(
  state: NoiseFloorState,
  pcm: Int16Array,
  sampleRateHz: number = DEFAULT_SAMPLE_RATE_HZ,
): { readonly state: NoiseFloorState; readonly result: MultiFeatureClassification } {
  const safeSampleRateHz = sampleRateHz > 0 && Number.isFinite(sampleRateHz) ? sampleRateHz : DEFAULT_SAMPLE_RATE_HZ;
  const rms = computeRms(pcm);
  const nextFloorState = updateNoiseFloor(state, rms);
  if (pcm.length === 0) {
    return {
      state: nextFloorState,
      result: {
        speech_probability: 0,
        source: 'heuristic-multifeature',
        features: { rms: 0, zero_crossing_rate: 0, spectral_flatness: 0, floor_rms: nextFloorState.floor_rms },
      },
    };
  }
  const zcr = computeZeroCrossingRate(pcm);
  const flatness = computeSpectralFlatness(pcm);
  const eScore = energyScore(rms, nextFloorState.floor_rms);
  const shapeTrust = (noiseSuppressionFactor(flatness) + zcrPlausibilityFactor(zcr, safeSampleRateHz)) / 2;
  const probabilityRaw = eScore * shapeTrust;
  const probability = Number.isFinite(probabilityRaw) ? Math.min(1, Math.max(0, probabilityRaw)) : 0;
  return {
    state: nextFloorState,
    result: {
      speech_probability: probability,
      source: 'heuristic-multifeature',
      features: { rms, zero_crossing_rate: zcr, spectral_flatness: flatness, floor_rms: nextFloorState.floor_rms },
    },
  };
}

export function createMultiFeatureVadClassifier(): VadClassifier<NoiseFloorState> {
  return {
    source: 'heuristic-multifeature',
    initialState: INITIAL_NOISE_FLOOR_STATE,
    classify(state, pcm, sampleRateHz) {
      return classifyMultiFeature(state, pcm, sampleRateHz);
    },
  };
}

/**
 * Thrown by `createSileroVadClassifier`. Named so a caller that selects `'silero-onnx'` before the
 * dependency lands fails loudly and specifically, rather than silently falling back to a heuristic —
 * this product's defining scar is exactly a silent quality fallback (native TTS standing in for the
 * real voice with nothing surfacing it). See `docs/adr/0013-vad-and-endpointing-architecture.md`.
 */
export class VadDependencyUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'VadDependencyUnavailableError';
  }
}

/**
 * The real fix: silero-vad's ONNX model, run on-device via `onnxruntime-react-native`. Not
 * implemented in this file — this repo has none of: the npm dependency (`package.json` is Track
 * A's, not this track's owned scope), the native iOS/Android linking, or the bundled model asset.
 * This factory exists purely so the port stays swappable and concrete: once those three land, the
 * classifier this returns replaces `createMultiFeatureVadClassifier()` in `startCapture`'s default
 * with no change to `VADGate.ts` or any caller — that is the entire point of the `VadClassifier` port.
 */
export function createSileroVadClassifier(): VadClassifier<unknown> {
  throw new VadDependencyUnavailableError(
    "silero-onnx VAD requires 'onnxruntime-react-native' (npm dependency, not installed in this " +
      'tree), the silero-vad v5 ONNX model asset bundled into the app, and native iOS/Android ' +
      'linking (Podfile / build.gradle) — none of which are present or in this track\'s owned ' +
      'files. See docs/adr/0013-vad-and-endpointing-architecture.md for the exact integration plan.',
  );
}

const BASE64_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

/**
 * Standard base64 decode to raw bytes. Written locally rather than relying on a global `atob`
 * because Hermes's availability of it has varied across RN versions, and this keeps the function
 * pure and unit-testable without React Native.
 */
export function decodeBase64(base64: string): Uint8Array {
  const clean = base64.replace(/=+$/u, '');
  const bytes: number[] = [];
  let buffer = 0;
  let bitsCollected = 0;
  for (const char of clean) {
    const value = BASE64_ALPHABET.indexOf(char);
    if (value === -1) continue;
    buffer = (buffer << 6) | value;
    bitsCollected += 6;
    if (bitsCollected >= 8) {
      bitsCollected -= 8;
      bytes.push((buffer >> bitsCollected) & 0xff);
    }
  }
  return Uint8Array.from(bytes);
}

interface OrbMicFrameEvent {
  readonly audioBase64: string;
  readonly atMs: number;
  readonly byteLength: number;
}

interface NativeMicModule {
  readonly start: () => Promise<void>;
  readonly stop: () => Promise<void>;
}

function loadNativeMic(): { readonly module: NativeMicModule; readonly emitter: NativeEventEmitterLike } {
  const { NativeModules, NativeEventEmitter } = require('react-native') as {
    readonly NativeModules?: { readonly OrbMic?: NativeMicModule };
    readonly NativeEventEmitter: new (module: unknown) => NativeEventEmitterLike;
  };
  const nativeMic = NativeModules?.OrbMic;
  if (!nativeMic) throw new Error('NativeModules.OrbMic is not registered');
  return { module: nativeMic, emitter: new NativeEventEmitter(nativeMic) };
}

interface NativeEventEmitterLike {
  addListener(eventType: string, listener: (event: OrbMicFrameEvent) => void): { remove: () => void };
}

/**
 * Requests mic access — every app start, not just first install (this must always run, per the
 * product requirement: check on every launch and prompt whenever it isn't already granted).
 * `PermissionsAndroid.request()` and iOS's `OrbMic.requestPermission()` both already no-op to an
 * immediate `granted` resolve if the user already said yes, and both re-prompt with the OS dialog
 * when the answer is still undecided — calling this unconditionally on every start is correct on
 * both platforms, not just safe. Resolves `false` (never throws) on denial so callers can no-op
 * the mic side rather than crash.
 */
export async function requestMicPermission(): Promise<boolean> {
  const { Platform, PermissionsAndroid, NativeModules } = require('react-native') as {
    readonly Platform: { readonly OS: string };
    readonly PermissionsAndroid?: {
      readonly PERMISSIONS: { readonly RECORD_AUDIO: string };
      readonly RESULTS: { readonly GRANTED: string };
      readonly request: (permission: string) => Promise<string>;
    };
    readonly NativeModules?: { readonly OrbMic?: { requestPermission: () => Promise<boolean> } };
  };
  try {
    if (Platform.OS === 'ios') {
      const granted = await NativeModules?.OrbMic?.requestPermission();
      if (granted === undefined) throw new Error('NativeModules.OrbMic is not registered');
      return granted;
    }
    if (!PermissionsAndroid) throw new Error('PermissionsAndroid is not available on this platform');
    const result = await PermissionsAndroid.request(PermissionsAndroid.PERMISSIONS.RECORD_AUDIO);
    return result === PermissionsAndroid.RESULTS.GRANTED;
  } catch (error) {
    console.error(`focus-orb:mic-permission-error ${error instanceof Error ? error.message : String(error)}`);
    return false;
  }
}

let activeSubscription: { remove: () => void } | null = null;

/**
 * Classifies one raw PCM frame and builds the `VadFrame` `startCapture` hands to its caller,
 * tagging it with the classifier's `source` (the "detectable, never silent" requirement). Pulled
 * out of `startCapture` as a pure function specifically so this — the part that actually matters —
 * is unit-testable without mocking React Native's native module bridge; `startCapture` itself is
 * thin plumbing around this and the native event subscription, matching how the rest of this
 * codebase already draws the line between pure logic and native wiring.
 */
export function buildVadFrame<S>(
  classifier: VadClassifier<S>,
  classifierState: S,
  pcm: Int16Array,
  atMs: number,
  sampleRateHz: number,
): { readonly frame: VadFrame; readonly nextState: S } {
  const classified = classifier.classify(classifierState, pcm, sampleRateHz);
  return {
    frame: {
      at_ms: atMs,
      speech_probability: classified.result.speech_probability,
      vad_source: classified.result.source,
    },
    nextState: classified.state,
  };
}

export interface StartCaptureOptions {
  /**
   * Which classifier to run. Defaults to the multi-feature heuristic. Pass
   * `createRmsVadClassifier()` to force the old fallback, or a real `createSileroVadClassifier()`
   * once its dependency is wired — `startCapture` never chooses this silently on its own.
   */
  readonly classifier?: VadClassifier<unknown>;
  /** Mic sample rate in Hz, forwarded to the classifier. Defaults to 16kHz (matches RelayClient's mic path). */
  readonly sampleRateHz?: number;
  readonly devLogger?: DevLogger;
}

/**
 * Starts native capture and subscribes to `OrbMicFrame` events, converting each to a
 * (`VadFrame`, `ArrayBuffer`) pair for the caller (`App.tsx` feeds these straight into
 * `voiceLoop.handleAudioFrame`). Each call gets fresh classifier state (a device switch that
 * restarts capture gets a fresh noise-floor adaptation window rather than an inherited one) and
 * logs, once, which VAD source is active — never silently.
 */
export async function startCapture(
  onFrame: (frame: VadFrame, audio: ArrayBuffer) => void,
  options: StartCaptureOptions = {},
): Promise<void> {
  const classifier = options.classifier ?? createMultiFeatureVadClassifier();
  const sampleRateHz = options.sampleRateHz ?? DEFAULT_SAMPLE_RATE_HZ;
  const devLogger = options.devLogger ?? noopDevLogger;
  let classifierState = classifier.initialState;

  devLogger.log('focus-orb.vad_classifier_active', {
    source: classifier.source,
    is_fallback: classifier.source !== 'silero-onnx',
    sample_rate_hz: sampleRateHz,
  }, classifier.source === 'silero-onnx' ? 'info' : 'warn');

  const { module, emitter } = loadNativeMic();
  activeSubscription?.remove();
  activeSubscription = emitter.addListener('OrbMicFrame', (event) => {
    const bytes = decodeBase64(event.audioBase64);
    const audio = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
    const pcm = new Int16Array(audio);
    const { frame, nextState } = buildVadFrame(classifier, classifierState, pcm, event.atMs, sampleRateHz);
    classifierState = nextState;
    onFrame(frame, audio);
  });
  await module.start();
}

export async function stopCapture(): Promise<void> {
  activeSubscription?.remove();
  activeSubscription = null;
  try {
    const { module } = loadNativeMic();
    await module.stop();
  } catch (error) {
    console.error(`focus-orb:mic-stop-error ${error instanceof Error ? error.message : String(error)}`);
  }
}
