/**
 * What confirmed-pause threshold ends turns early WITHOUT cutting real speakers off mid-sentence?
 *
 * `ENDPOINT_COMPLETE_PAUSE_MS` is 150ms today. Measured over this repo's real recordings, the
 * inter-word pauses inside a single natural utterance run to 320-400ms (verified as real silence,
 * not VAD error: gap-attribution.ts puts non-voice frames at ~13% of voice RMS). This sweep is the
 * evidence behind any decision to change that number — it is NOT a change to it.
 */
import { readFileSync } from 'node:fs';

import { extractPcmFromWav } from '../../backend/voice-provider-sidecar/src/wav';
import { buildVadFrame, createMultiFeatureVadClassifier } from '../../apps/mobile/src/voice/NativeMicPort';
import { INITIAL_VAD_GATE_STATE, reduceVadGate, type VadGateState } from '../../apps/mobile/src/voice/VADGate';
import { isVoiceFrame, pauseMsSince } from '../../apps/mobile/src/voice/PauseClock';
import { decideEndpoint } from '../../apps/mobile/src/voice/SemanticEndpointer';
import { ENDPOINT_COMPLETE_PAUSE_MS, VAD_POST_ENDPOINT_CLOSE_MS } from '../../apps/mobile/src/voice/contracts';

const SAMPLE_RATE_HZ = 16_000;
const FRAME_MS = 20;
const SAMPLES_PER_FRAME = (SAMPLE_RATE_HZ * FRAME_MS) / 1_000;
const STT_LAG_MS = 200;
const PARTIAL_INTERVAL_MS = 100;
const TRAILING_SILENCE_MS = 2_500;
const DIR = 'e2e-human-simulator/replay/fixtures';

const manifest = JSON.parse(readFileSync(`${DIR}/../fixtures.json`, 'utf8')) as {
  utterances: { id: string; wav: string; expected_transcript: string }[];
};
const cases = manifest.utterances.filter((u) => u.expected_transcript.trim().length > 0);

function framesOf(wav: string): Int16Array[] {
  const { pcm } = extractPcmFromWav(new Uint8Array(readFileSync(`${DIR}/${wav}`)));
  const samples = new Int16Array(pcm.buffer.slice(pcm.byteOffset, pcm.byteOffset + pcm.byteLength));
  const frames: Int16Array[] = [];
  for (let o = 0; o + SAMPLES_PER_FRAME <= samples.length; o += SAMPLES_PER_FRAME) {
    frames.push(samples.subarray(o, o + SAMPLES_PER_FRAME));
  }
  const silence = new Int16Array(SAMPLES_PER_FRAME);
  for (let i = 0; i < TRAILING_SILENCE_MS / FRAME_MS; i += 1) frames.push(silence);
  return frames;
}

function evaluate(wav: string, transcript: string, completePauseMs: number) {
  const timeline = framesOf(wav);
  const classifier = createMultiFeatureVadClassifier();
  let cState = classifier.initialState;
  let vadState: VadGateState = INITIAL_VAD_GATE_STATE;
  let lastVoice: number | null = null;
  let firstVoice: number | null = null;
  let lastVoiceOverall: number | null = null;
  let semantic: number | null = null;
  let vad: number | null = null;

  // Pre-pass for the true voice span, so partial growth is aligned to the real audio.
  {
    const c2 = createMultiFeatureVadClassifier();
    let s2 = c2.initialState;
    for (let i = 0; i < timeline.length; i += 1) {
      const b = buildVadFrame(c2, s2, timeline[i], i * FRAME_MS, SAMPLE_RATE_HZ);
      s2 = b.nextState;
      if (isVoiceFrame(b.frame.speech_probability)) {
        if (firstVoice === null) firstVoice = i * FRAME_MS;
        lastVoiceOverall = i * FRAME_MS;
      }
    }
  }
  const words = transcript.split(' ');
  for (let i = 0; i < timeline.length; i += 1) {
    const atMs = i * FRAME_MS;
    const built = buildVadFrame(classifier, cState, timeline[i], atMs, SAMPLE_RATE_HZ);
    cState = built.nextState;
    if (isVoiceFrame(built.frame.speech_probability)) lastVoice = atMs;
    const d = reduceVadGate(vadState, built.frame);
    vadState = d.state;
    if (vad === null && d.decision.kind === 'end_of_turn') vad = atMs;
    if (semantic === null && vadState.phase === 'listening' && atMs >= STT_LAG_MS && (atMs - STT_LAG_MS) % PARTIAL_INTERVAL_MS === 0) {
      const progress = Math.min(1, (atMs - (firstVoice ?? 0)) / Math.max(1, (lastVoiceOverall ?? 0) - (firstVoice ?? 0)));
      const partial = words.slice(0, Math.max(1, Math.ceil(progress * words.length))).join(' ');
      if (atMs > (firstVoice ?? 0) && decideEndpoint(partial, pauseMsSince(lastVoice, atMs), atMs, { completePauseMs }).kind === 'endpoint') {
        semantic = atMs;
      }
    }
  }
  return { semantic, vad, lastVoice: lastVoiceOverall };
}

console.log(`threshold sweep over ${cases.length} real utterances with ground-truth transcripts`);
console.log(`(shipped ENDPOINT_COMPLETE_PAUSE_MS = ${ENDPOINT_COMPLETE_PAUSE_MS}ms, VAD hangover = ${VAD_POST_ENDPOINT_CLOSE_MS}ms)\n`);
console.log('  threshold |  cutoffs | mean saved vs VAD | detail');
console.log('  ----------+----------+-------------------+--------');
for (const completePauseMs of [150, 200, 250, 300, 350, 400, 450, 500, 600]) {
  let cutoffs = 0;
  const saved: number[] = [];
  const detail: string[] = [];
  for (const c of cases) {
    const r = evaluate(c.wav, c.expected_transcript, completePauseMs);
    const cut = r.semantic !== null && r.lastVoice !== null && r.semantic < r.lastVoice;
    if (cut) cutoffs += 1;
    if (r.semantic !== null && r.vad !== null) saved.push(r.vad - r.semantic);
    detail.push(`${c.id}:${r.semantic === null ? 'none' : `${r.semantic}ms`}${cut ? '!' : ''}`);
  }
  const meanSaved = saved.length === 0 ? 0 : Math.round(saved.reduce((a, b) => a + b, 0) / saved.length);
  console.log(
    `  ${String(completePauseMs).padStart(6)}ms  |  ${String(cutoffs).padStart(1)}/${cases.length}     |  ${String(meanSaved).padStart(9)}ms      | ${detail.join('  ')}`,
  );
}
console.log('\n"!" marks a turn ended while the speaker was still talking. Denominator is 3 —');
console.log('small, and 3 English utterances from one speaker; it bounds a decision, it does not settle one.');
