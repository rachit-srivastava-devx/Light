/**
 * Is the "gap" real silence, or the interim heuristic VAD losing a low-energy phoneme?
 *
 * The endpointing recommendation depends on the answer: a real pause means the 150ms confirmed
 * threshold is too aggressive for this speech; a VAD false-negative means the threshold is fine and
 * the classifier is the thing to fix (ADR 0013 already recommends silero for exactly that reason).
 * Raw RMS is independent of the classifier, so it can arbitrate.
 */
import { readFileSync } from 'node:fs';

import { extractPcmFromWav } from '../../backend/voice-provider-sidecar/src/wav';
import { buildVadFrame, createMultiFeatureVadClassifier } from '../../apps/mobile/src/voice/NativeMicPort';
import { isVoiceFrame } from '../../apps/mobile/src/voice/PauseClock';

const SAMPLE_RATE_HZ = 16_000;
const FRAME_MS = 20;
const SAMPLES_PER_FRAME = (SAMPLE_RATE_HZ * FRAME_MS) / 1_000;

function rms(frame: Int16Array): number {
  let sum = 0;
  for (const sample of frame) sum += (sample / 32_768) ** 2;
  return Math.sqrt(sum / frame.length);
}

for (const file of ['hello-how-are-you.wav', 'open-tax-portal.wav']) {
  const path = `e2e-human-simulator/replay/fixtures/${file}`;
  const { pcm } = extractPcmFromWav(new Uint8Array(readFileSync(path)));
  const samples = new Int16Array(pcm.buffer.slice(pcm.byteOffset, pcm.byteOffset + pcm.byteLength));
  const classifier = createMultiFeatureVadClassifier();
  let state = classifier.initialState;

  const voiceRms: number[] = [];
  const gapRms: number[] = [];
  const trace: string[] = [];
  for (let index = 0; index * SAMPLES_PER_FRAME + SAMPLES_PER_FRAME <= samples.length; index += 1) {
    const frame = samples.subarray(index * SAMPLES_PER_FRAME, (index + 1) * SAMPLES_PER_FRAME);
    const built = buildVadFrame(classifier, state, frame, index * FRAME_MS, SAMPLE_RATE_HZ);
    state = built.nextState;
    const energy = rms(frame);
    (isVoiceFrame(built.frame.speech_probability) ? voiceRms : gapRms).push(energy);
    trace.push(`${isVoiceFrame(built.frame.speech_probability) ? 'V' : '.'}`);
  }
  const mean = (values: number[]) => (values.length === 0 ? 0 : values.reduce((a, b) => a + b, 0) / values.length);
  console.log(`\n${file}`);
  console.log(`  frames        ${trace.length} (20ms each)`);
  console.log(`  VAD trace     ${trace.join('')}`);
  console.log(`  mean RMS      voice=${mean(voiceRms).toFixed(4)}  non-voice=${mean(gapRms).toFixed(4)}`);
  console.log(
    `  ratio         non-voice is ${(mean(gapRms) / Math.max(mean(voiceRms), 1e-9) * 100).toFixed(1)}% of voice energy`,
  );
}
