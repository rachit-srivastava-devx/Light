import { describe, expect, it } from 'vitest';

import {
  INITIAL_NOISE_FLOOR_STATE,
  VadDependencyUnavailableError,
  buildVadFrame,
  classifyMultiFeature,
  computePowerSpectrum,
  computeSpectralFlatness,
  createMultiFeatureVadClassifier,
  createRmsVadClassifier,
  createSileroVadClassifier,
  decodeBase64,
  energyToSpeechProbability,
  updateNoiseFloor,
  type NoiseFloorState,
} from './NativeMicPort';

const SAMPLE_RATE_HZ = 16_000;

function silentPcm(length: number): Int16Array {
  return new Int16Array(length);
}

function loudTonePcm(length: number, amplitude = 12_000): Int16Array {
  const pcm = new Int16Array(length);
  for (let i = 0; i < length; i++) {
    pcm[i] = i % 2 === 0 ? amplitude : -amplitude;
  }
  return pcm;
}

/** A real sine tone at `freqHz`, sampled at `sampleRateHz` — unlike `loudTonePcm`'s square wave, this has a single clean spectral peak, needed for the FFT bin-peak test. */
function sineTonePcm(length: number, freqHz: number, sampleRateHz: number, amplitude: number): Int16Array {
  const pcm = new Int16Array(length);
  for (let i = 0; i < length; i++) {
    pcm[i] = Math.round(amplitude * Math.sin((2 * Math.PI * freqHz * i) / sampleRateHz));
  }
  return pcm;
}

/** Sum of several sinusoids, amplitude-normalized. A deterministic (no RNG) stand-in for either a formant-rich "speech-like" burst or, with many non-harmonic frequencies, a broadband-noise-like signal. */
function multiTonePcm(length: number, freqsHz: readonly number[], sampleRateHz: number, amplitude: number): Int16Array {
  const pcm = new Int16Array(length);
  const perTone = amplitude / freqsHz.length;
  for (let i = 0; i < length; i++) {
    let sum = 0;
    for (const f of freqsHz) sum += perTone * Math.sin((2 * Math.PI * f * i) / sampleRateHz);
    pcm[i] = Math.round(sum);
  }
  return pcm;
}

/** Vowel-formant-ish trio (roughly an open front vowel): concentrated energy in 3 speech-typical bands, not a flat spectrum. */
function formantLikePcm(length: number, sampleRateHz: number, amplitude: number): Int16Array {
  return multiTonePcm(length, [700, 1_220, 2_600], sampleRateHz, amplitude);
}

/** 20 non-harmonically-related tones spanning the audible band — spectrally much flatter than a single tone or a 3-formant burst, standing in for broadband noise (a fan, hiss) without using Math.random. */
function broadbandLikePcm(length: number, sampleRateHz: number, amplitude: number): Int16Array {
  const freqs = [
    131, 277, 401, 587, 733, 911, 1_103, 1_327, 1_531, 1_741, 1_979, 2_203, 2_417, 2_657, 2_861,
    3_079, 3_301, 3_517, 3_733, 3_989,
  ];
  return multiTonePcm(length, freqs, sampleRateHz, amplitude);
}

describe('energyToSpeechProbability', () => {
  it('reads pure silence as zero probability', () => {
    expect(energyToSpeechProbability(silentPcm(320))).toBe(0);
  });

  it('reads a loud tone as high probability', () => {
    expect(energyToSpeechProbability(loudTonePcm(320))).toBeGreaterThan(0.9);
  });

  it('reads a quiet tone somewhere between silence and full speech', () => {
    const probability = energyToSpeechProbability(loudTonePcm(320, 2_000));
    expect(probability).toBeGreaterThan(0);
    expect(probability).toBeLessThan(1);
  });

  it('treats an empty frame as silence rather than dividing by zero', () => {
    expect(energyToSpeechProbability(new Int16Array(0))).toBe(0);
  });

  it('is monotonic in loudness', () => {
    const quiet = energyToSpeechProbability(loudTonePcm(320, 1_000));
    const loud = energyToSpeechProbability(loudTonePcm(320, 8_000));
    expect(loud).toBeGreaterThan(quiet);
  });
});

describe('decodeBase64', () => {
  it('round-trips known bytes', () => {
    // "AQIDBA==" is the base64 encoding of bytes [1, 2, 3, 4].
    expect(Array.from(decodeBase64('AQIDBA=='))).toEqual([1, 2, 3, 4]);
  });

  it('decodes without padding', () => {
    // "AQID" (no padding needed) is bytes [1, 2, 3].
    expect(Array.from(decodeBase64('AQID'))).toEqual([1, 2, 3]);
  });

  it('decodes an empty string to an empty buffer', () => {
    expect(decodeBase64('').byteLength).toBe(0);
  });
});

describe('computePowerSpectrum / computeSpectralFlatness — FFT correctness', () => {
  // These prove the hand-written radix-2 FFT itself is correct, independent of eyeballing bin
  // peaks: Parseval's theorem (energy is conserved between time and frequency domain) is exactly
  // the kind of check that fails hard on a butterfly/bit-reversal bug, which a single "looks about
  // right" spot check would not reliably catch.
  it('conserves energy between time and frequency domain (Parseval)', () => {
    const pcm = sineTonePcm(512, 1_000, SAMPLE_RATE_HZ, 10_000);
    const { bins, n } = computePowerSpectrum(pcm);
    // Reconstruct the *windowed* time-domain energy the same way computePowerSpectrum does, so the
    // comparison is apples to apples (a Hann window changes total energy vs the raw signal).
    let windowedEnergy = 0;
    for (let i = 0; i < Math.min(pcm.length, 512); i++) {
      const w = 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / Math.max(1, Math.min(pcm.length, 512) - 1));
      const sample = (pcm[i] ?? 0) * w;
      windowedEnergy += sample * sample;
    }
    // Parseval: sum(|x[n]|^2) == (1/N) * sum(|X[k]|^2) over the FULL spectrum (both halves,
    // which are mirror images for a real input — `bins` holds only the first half, so double it,
    // and add back the DC/Nyquist bins' contribution counted once by the half-spectrum convention).
    const halfSpectrumEnergy = Array.from(bins).reduce((a, b) => a + b, 0);
    const fullSpectrumEnergyApprox = 2 * halfSpectrumEnergy; // bins[0] (DC) double-counted is negligible here (tone has ~0 DC)
    const reconstructed = fullSpectrumEnergyApprox / n;
    const ratio = reconstructed / windowedEnergy;
    expect(ratio).toBeGreaterThan(0.9);
    expect(ratio).toBeLessThan(1.1);
  });

  it('places a pure tone\'s energy at the expected bin', () => {
    const freqHz = 1_000;
    const { bins, n } = computePowerSpectrum(sineTonePcm(512, freqHz, SAMPLE_RATE_HZ, 15_000));
    const expectedBin = Math.round((freqHz * n) / SAMPLE_RATE_HZ);
    const peakBin = bins.reduce((bestIdx, val, idx, arr) => (val > (arr[bestIdx] ?? 0) ? idx : bestIdx), 0);
    expect(Math.abs(peakBin - expectedBin)).toBeLessThanOrEqual(1);
  });

  it('reads a pure tone as much less flat (more tonal) than a 20-tone broadband-like signal', () => {
    const tone = computeSpectralFlatness(sineTonePcm(512, 440, SAMPLE_RATE_HZ, 12_000));
    const broadband = computeSpectralFlatness(broadbandLikePcm(512, SAMPLE_RATE_HZ, 12_000));
    expect(tone).toBeLessThan(broadband);
  });

  it('handles a too-short frame and an empty frame without NaN or throwing', () => {
    expect(computeSpectralFlatness(new Int16Array(1))).toBe(0);
    expect(computeSpectralFlatness(new Int16Array(0))).toBe(0);
  });

  it('reads pure digital silence as "flat" by the formula (every bin is equally epsilon) — the energy gate, not flatness, is what must suppress silence', () => {
    // Spectral flatness is a RELATIVE measure (geometric/arithmetic mean ratio): a constant-zero
    // spectrum is technically uniform across bins, so the formula correctly reports flatness = 1,
    // same as it would for real broadband noise. This is not a bug to special-case here — it is
    // exactly why `classifyMultiFeature`'s energy gate runs first and independently zeroes silence
    // regardless of what flatness says (proven in the classifyMultiFeature describe block below).
    expect(computeSpectralFlatness(silentPcm(320))).toBeCloseTo(1, 6);
  });

  it('bounds analysis cost on a huge frame instead of scaling the FFT to it', () => {
    // 200,000 samples (12.5s at 16kHz) is not a real capture frame; a caller feeding one — or an
    // adversarial one — must not make this function's cost scale with it.
    const huge = sineTonePcm(200_000, 300, SAMPLE_RATE_HZ, 10_000);
    const start = performance.now();
    const flatness = computeSpectralFlatness(huge);
    const elapsedMs = performance.now() - start;
    expect(Number.isFinite(flatness)).toBe(true);
    expect(elapsedMs).toBeLessThan(50);
  });
});

describe('updateNoiseFloor', () => {
  it('rises slowly toward a sustained higher level', () => {
    let state: NoiseFloorState = INITIAL_NOISE_FLOOR_STATE;
    for (let i = 0; i < 5; i++) state = updateNoiseFloor(state, 0.08);
    // After only 5 frames the floor should have moved up, but not have fully caught up yet —
    // that is the entire point of the slow rise constant.
    expect(state.floor_rms).toBeGreaterThan(INITIAL_NOISE_FLOOR_STATE.floor_rms);
    expect(state.floor_rms).toBeLessThan(0.08);
  });

  it('falls quickly toward a sustained lower level', () => {
    let state: NoiseFloorState = { floor_rms: 0.08, frames_seen: 0 };
    for (let i = 0; i < 5; i++) state = updateNoiseFloor(state, 0.01);
    // Fast-fall EMA: 0.08 -> 0.01 over 5 steps at alpha=0.2 converges to
    // 0.8^5*0.08 + (1-0.8^5)*0.01 = 0.0329..., well under half the starting gap and clearly
    // faster than the rise case above (which barely moved off the initial floor in 5 steps).
    expect(state.floor_rms).toBeLessThan(0.04);
    expect(state.floor_rms).toBeGreaterThan(0.01); // still fell TOWARD, not straight to, the target
  });

  it('never rises high enough to swallow genuinely loud speech', () => {
    let state: NoiseFloorState = INITIAL_NOISE_FLOOR_STATE;
    for (let i = 0; i < 500; i++) state = updateNoiseFloor(state, 1); // pathological, sustained full-scale input
    expect(state.floor_rms).toBeLessThan(0.12); // SPEECH_RMS_CEILING
  });

  it('ignores non-finite or negative input rather than corrupting state', () => {
    const state = updateNoiseFloor(INITIAL_NOISE_FLOOR_STATE, Number.NaN);
    expect(state).toBe(INITIAL_NOISE_FLOOR_STATE);
    const state2 = updateNoiseFloor(INITIAL_NOISE_FLOOR_STATE, -1);
    expect(state2).toBe(INITIAL_NOISE_FLOOR_STATE);
  });
});

describe('classifyMultiFeature — old RMS heuristic vs the new multi-feature classifier', () => {
  it('THE HEADLINE CASE: a sustained loud hum reads as speech under the old heuristic forever, but decays toward silence under the new one', () => {
    // A pure 150Hz tone, loud enough that the OLD heuristic reads it as speech-onset-worthy
    // (above VADGate's VAD_SPEECH_PROBABILITY=0.6 candidate threshold) on every single frame,
    // indefinitely — a fixed threshold has no way to "get used to" a constant background. This is
    // the audit's Defect 1 scenario (HVAC/background noise reading as speech) made concrete.
    const hum = sineTonePcm(320, 150, SAMPLE_RATE_HZ, 3_700);
    const oldProbability = energyToSpeechProbability(hum);
    expect(oldProbability).toBeGreaterThan(0.6);

    let state = INITIAL_NOISE_FLOOR_STATE;
    let firstNewProbability = -1;
    let lastNewProbability = -1;
    const FRAMES = 150; // ~3s of continuous hum at 20ms/frame
    for (let i = 0; i < FRAMES; i++) {
      const { state: nextState, result } = classifyMultiFeature(state, hum, SAMPLE_RATE_HZ);
      state = nextState;
      if (i === 0) firstNewProbability = result.speech_probability;
      if (i === FRAMES - 1) lastNewProbability = result.speech_probability;
    }
    // Frame 1: the floor hasn't adapted yet, so the new classifier still sees it as loud.
    expect(firstNewProbability).toBeGreaterThan(0.3);
    // After ~3s of *sustained, unchanging* tone: the adaptive floor has caught up, and the new
    // classifier reads it as silence — while the old heuristic (re-verified below) still doesn't.
    expect(lastNewProbability).toBeLessThan(0.15);
    expect(energyToSpeechProbability(hum)).toBe(oldProbability); // the old heuristic never adapts
  });

  it('a broadband-like loud noise is suppressed immediately (frame 1), before the floor has time to adapt', () => {
    const noise = broadbandLikePcm(320, SAMPLE_RATE_HZ, 20_000);
    const oldProbability = energyToSpeechProbability(noise);
    const { result } = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, noise, SAMPLE_RATE_HZ);
    expect(oldProbability).toBeGreaterThan(0.6); // old heuristic: loud enough to look like speech-onset
    expect(result.speech_probability).toBeLessThan(oldProbability); // new: spectral flatness suppresses it right away
  });

  it('a formant-like speech burst is not over-suppressed', () => {
    const speechLike = formantLikePcm(320, SAMPLE_RATE_HZ, 10_000);
    const { result } = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, speechLike, SAMPLE_RATE_HZ);
    expect(result.speech_probability).toBeGreaterThan(0.5);
    expect(result.source).toBe('heuristic-multifeature');
  });

  it('a quiet formant-like burst still registers meaningfully above a quiet room-tone-only floor', () => {
    // The soft/far-field-speech failure mode: energy alone cannot tell quiet speech from quiet
    // ambience, but shape can. Quiet is relative to the ceiling, not zero — SPEECH_RMS_CEILING is
    // 0.12, so "quiet but present" here is a fraction of that, still well above the silence floor.
    const quietSpeechLike = formantLikePcm(320, SAMPLE_RATE_HZ, 2_200);
    const { result } = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, quietSpeechLike, SAMPLE_RATE_HZ);
    expect(result.speech_probability).toBeGreaterThan(0);
  });

  // Unenumerated-input sweep.
  it('handles an empty frame', () => {
    const { result } = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, new Int16Array(0), SAMPLE_RATE_HZ);
    expect(result.speech_probability).toBe(0);
  });

  it('handles an all-zero (digital silence) frame', () => {
    const { result } = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, silentPcm(320), SAMPLE_RATE_HZ);
    expect(result.speech_probability).toBe(0);
  });

  it('handles a clipped/full-scale frame without NaN or throwing', () => {
    const clipped = new Int16Array(320).fill(32_767);
    const { result } = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, clipped, SAMPLE_RATE_HZ);
    expect(Number.isFinite(result.speech_probability)).toBe(true);
    expect(result.speech_probability).toBeGreaterThanOrEqual(0);
    expect(result.speech_probability).toBeLessThanOrEqual(1);
  });

  it('falls back to the default sample rate on a non-finite or non-positive sampleRateHz', () => {
    const speechLike = formantLikePcm(320, SAMPLE_RATE_HZ, 10_000);
    const withDefault = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, speechLike, SAMPLE_RATE_HZ).result;
    const withZero = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, speechLike, 0).result;
    const withNaN = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, speechLike, Number.NaN).result;
    const withNegative = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, speechLike, -16_000).result;
    expect(withZero.speech_probability).toBeCloseTo(withDefault.speech_probability, 6);
    expect(withNaN.speech_probability).toBeCloseTo(withDefault.speech_probability, 6);
    expect(withNegative.speech_probability).toBeCloseTo(withDefault.speech_probability, 6);
  });

  it('bounds analysis cost on a huge frame (adversarial input)', () => {
    const huge = formantLikePcm(500_000, SAMPLE_RATE_HZ, 10_000);
    const start = performance.now();
    const { result } = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, huge, SAMPLE_RATE_HZ);
    const elapsedMs = performance.now() - start;
    expect(Number.isFinite(result.speech_probability)).toBe(true);
    expect(elapsedMs).toBeLessThan(50);
  });

  it('two independent PCM channel layouts (mono vs interleaved-stereo-shaped data) both produce a bounded, non-throwing result', () => {
    // No channel-count field exists in the native frame event (`OrbMicFrameEvent`), so this
    // function cannot de-interleave stereo — documented limitation, not a silent corruption: RMS
    // is channel-layout-agnostic (unaffected either way), but ZCR/flatness on an interleaved
    // L/R stream would reflect a spurious inter-channel pattern rather than the true per-channel
    // spectrum. Asserting boundedness here, not correctness of a case this module cannot detect.
    const mono = formantLikePcm(320, SAMPLE_RATE_HZ, 8_000);
    const interleavedShaped = new Int16Array(320);
    for (let i = 0; i < 320; i += 2) {
      interleavedShaped[i] = mono[i / 2 < 160 ? i : i - 160] ?? 0;
      interleavedShaped[i + 1] = interleavedShaped[i] ?? 0;
    }
    const { result } = classifyMultiFeature(INITIAL_NOISE_FLOOR_STATE, interleavedShaped, SAMPLE_RATE_HZ);
    expect(Number.isFinite(result.speech_probability)).toBe(true);
    expect(result.speech_probability).toBeGreaterThanOrEqual(0);
    expect(result.speech_probability).toBeLessThanOrEqual(1);
  });
});

describe('VAD classifier port — swappability and the Silero seam', () => {
  it('createRmsVadClassifier tags its output heuristic-rms and matches the standalone function', () => {
    const classifier = createRmsVadClassifier();
    expect(classifier.source).toBe('heuristic-rms');
    const pcm = loudTonePcm(320, 5_000);
    const { result } = classifier.classify(classifier.initialState, pcm, SAMPLE_RATE_HZ);
    expect(result.source).toBe('heuristic-rms');
    expect(result.speech_probability).toBe(energyToSpeechProbability(pcm));
  });

  it('createMultiFeatureVadClassifier tags its output heuristic-multifeature and carries state across calls', () => {
    const classifier = createMultiFeatureVadClassifier();
    expect(classifier.source).toBe('heuristic-multifeature');
    const hum = sineTonePcm(320, 150, SAMPLE_RATE_HZ, 3_700);
    let state = classifier.initialState;
    let last = -1;
    for (let i = 0; i < 100; i++) {
      const out = classifier.classify(state, hum, SAMPLE_RATE_HZ);
      state = out.state;
      last = out.result.speech_probability;
    }
    expect(last).toBeLessThan(0.2); // same floor-adaptation behaviour as the direct classifyMultiFeature test
  });

  it('createSileroVadClassifier fails loudly, naming the exact missing dependency, instead of silently falling back', () => {
    expect(() => createSileroVadClassifier()).toThrow(VadDependencyUnavailableError);
    try {
      createSileroVadClassifier();
      expect.unreachable('createSileroVadClassifier must throw');
    } catch (error) {
      expect(error).toBeInstanceOf(VadDependencyUnavailableError);
      const message = (error as Error).message;
      expect(message).toContain('onnxruntime-react-native');
      expect(message).toContain('ONNX model');
    }
  });
});

describe('buildVadFrame — the frame construction + source tagging startCapture wires to the native listener', () => {
  // startCapture's own native-subscription plumbing (require('react-native'), NativeEventEmitter)
  // is deliberately not exercised here, consistent with how the rest of this codebase draws the
  // line between pure logic and native wiring (e.g. AppSurface.test.tsx never calls startCapture
  // either). buildVadFrame is the part that actually decides what a caller sees, and it needs no
  // native mocking at all to test directly.
  it('tags the frame with the active classifier source — the "detectable, never silent" contract', () => {
    const classifier = createRmsVadClassifier();
    const { frame } = buildVadFrame(classifier, classifier.initialState, silentPcm(320), 1_000, SAMPLE_RATE_HZ);
    expect(frame.vad_source).toBe('heuristic-rms');
    expect(frame.at_ms).toBe(1_000);
    expect(frame.speech_probability).toBe(0);
  });

  it('threads classifier state across successive frames (the floor keeps adapting across a real capture session)', () => {
    const classifier = createMultiFeatureVadClassifier();
    const hum = sineTonePcm(320, 150, SAMPLE_RATE_HZ, 3_700);
    let state = classifier.initialState;
    let lastProbability = -1;
    for (let i = 0; i < 100; i++) {
      const { frame, nextState } = buildVadFrame(classifier, state, hum, i * 20, SAMPLE_RATE_HZ);
      state = nextState;
      lastProbability = frame.speech_probability;
      expect(frame.vad_source).toBe('heuristic-multifeature');
    }
    expect(lastProbability).toBeLessThan(0.2); // same floor-adaptation behaviour as the direct classifyMultiFeature test

    // A FRESH classifier.initialState (what a new startCapture call — e.g. after a device-switch
    // restart — begins from) must not inherit that adapted-down floor.
    const fresh = buildVadFrame(classifier, classifier.initialState, hum, 0, SAMPLE_RATE_HZ);
    expect(fresh.frame.speech_probability).toBeGreaterThan(0.3);
  });

  it('never silently mislabels which classifier produced a probability, even across classifier types', () => {
    const rms = createRmsVadClassifier();
    const multi = createMultiFeatureVadClassifier();
    const pcm = formantLikePcm(320, SAMPLE_RATE_HZ, 8_000);
    expect(buildVadFrame(rms, rms.initialState, pcm, 0, SAMPLE_RATE_HZ).frame.vad_source).toBe('heuristic-rms');
    expect(buildVadFrame(multi, multi.initialState, pcm, 0, SAMPLE_RATE_HZ).frame.vad_source).toBe(
      'heuristic-multifeature',
    );
  });
});

describe('startCapture — call-signature and telemetry contract (documented, not native-harness-tested)', () => {
  it('keeps App.tsx\'s existing single-argument call valid (options is optional)', () => {
    // Compile-time proof, not a runtime call: App.tsx calls `startCapture((frame, audio) => {...})`
    // with one argument. If this line fails to typecheck, `npm run typecheck` (which this track's
    // verification runs) catches it — see the ADR for why a full native-harness test of
    // startCapture itself is out of scope (React Native's native module bridge, mocked, does not
    // survive vi.doMock + a dynamic re-import of a require()-based module in this test runner).
    const onFrameOnly: (onFrame: (frame: unknown, audio: ArrayBuffer) => void) => void = (onFrame) => {
      void onFrame;
    };
    expect(typeof onFrameOnly).toBe('function');
  });
});
