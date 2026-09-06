/**
 * Evidence, not a test: drives REAL recorded human speech through the REAL shipped VAD classifier
 * and the REAL endpointing pipeline, then reports when each path would have ended the turn.
 *
 * Nothing here is re-implemented. `createMultiFeatureVadClassifier`, `buildVadFrame`,
 * `reduceVadGate`, `pauseMsSince` and `decideEndpoint` are imported from the app's own source, so
 * a change to any of them changes this number. Run:
 *   npx vite-node scripts/evidence/endpoint-latency.ts
 */
import { readFileSync } from 'node:fs';

import { extractPcmFromWav } from '../../backend/voice-provider-sidecar/src/wav';
import { buildVadFrame, createMultiFeatureVadClassifier } from '../../apps/mobile/src/voice/NativeMicPort';
import { INITIAL_VAD_GATE_STATE, reduceVadGate, type VadGateState } from '../../apps/mobile/src/voice/VADGate';
import { isVoiceFrame, pauseMsSince } from '../../apps/mobile/src/voice/PauseClock';
import { decideEndpoint } from '../../apps/mobile/src/voice/SemanticEndpointer';
import { VAD_POST_ENDPOINT_CLOSE_MS } from '../../apps/mobile/src/voice/contracts';

const SAMPLE_RATE_HZ = 16_000;
const FRAME_MS = 20;
const SAMPLES_PER_FRAME = (SAMPLE_RATE_HZ * FRAME_MS) / 1_000;
/** How far the STT partial trails the audio that produced it. */
const STT_LAG_MS = 200;
/**
 * Trailing quiet appended after the recording ends. These fixtures are tightly trimmed — measured:
 * their first frames are already speech, so there is no room tone in them to borrow — and the
 * repo's only real noise recordings are the ones ADR 0013 measured this same classifier reading as
 * speech 87-99% of the time. So this tail is digital silence: what the VAD sees on a quiet mic,
 * and deliberately the FRIENDLIEST possible tail for the VAD baseline being compared against.
 */
const TRAILING_SILENCE_MS = 2_500;
/** Real STT partials arrive in bursts, not per audio frame; the endpointer only sees what arrived. */
const PARTIAL_INTERVAL_MS = 100;

interface Case {
  readonly file: string;
  readonly transcript: string;
  readonly note: string;
}

const FIXTURE_DIR = 'e2e-human-simulator/replay/fixtures';

/**
 * The denominator, stated: `fixtures/` holds 13 WAVs but only the 4 in `fixtures.json` carry a
 * ground-truth transcript, and one of those is the deliberately-unclear failure path with an empty
 * expected transcript. So 3 real utterances are scoreable for a mid-utterance cutoff — a small
 * denominator, reported as such rather than presented as a rate.
 */
function declaredCases(): readonly Case[] {
  const manifest = JSON.parse(readFileSync(`${FIXTURE_DIR}/../fixtures.json`, 'utf8')) as {
    utterances: { id: string; wav: string; expected_transcript: string; tags: string[] }[];
  };
  const scoreable = manifest.utterances
    .filter((utterance) => utterance.expected_transcript.trim().length > 0)
    .map((utterance) => ({
      file: `${FIXTURE_DIR}/${utterance.wav}`,
      transcript: utterance.expected_transcript,
      note: `${utterance.id} [${utterance.tags.join(',')}]`,
    }));
  return [
    ...scoreable,
    {
      // Synthetic pairing, flagged as such: no Hindi/Hinglish recording exists in this repo, so the
      // bilingual marker is exercised against real audio timing with a substituted transcript.
      file: `${FIXTURE_DIR}/open-tax-portal.wav`,
      transcript: 'mujhe taxes file karni hain aur',
      note: 'SYNTHETIC transcript on real audio: Hinglish trailing conjunction (must NOT endpoint)',
    },
  ];
}

const CASES: readonly Case[] = declaredCases();

function framesFrom(path: string): Int16Array[] {
  const { pcm } = extractPcmFromWav(new Uint8Array(readFileSync(path)));
  const samples = new Int16Array(pcm.buffer.slice(pcm.byteOffset, pcm.byteOffset + pcm.byteLength));
  const frames: Int16Array[] = [];
  for (let offset = 0; offset + SAMPLES_PER_FRAME <= samples.length; offset += SAMPLES_PER_FRAME) {
    frames.push(samples.subarray(offset, offset + SAMPLES_PER_FRAME));
  }
  return frames;
}

interface Outcome {
  readonly semanticMs: number | null;
  readonly semanticTranscript: string | null;
  readonly vadMs: number | null;
  readonly firstVoiceMs: number | null;
  readonly lastVoiceMs: number | null;
  readonly longestGapMs: number;
  readonly longestGapAtMs: number | null;
  readonly frames: number;
}

/**
 * What the STT has emitted by now, as a real provider would: words appear as they are recognised,
 * not all at once. Feeding the finished sentence from the first partial would both hide the
 * continuation markers that protect a mid-sentence pause and invent complete-looking text that did
 * not exist yet — the difference between simulating STT and assuming the answer.
 */
function partialAt(transcript: string, atMs: number, firstVoiceMs: number, lastVoiceMs: number): string {
  const words = transcript.split(' ');
  if (atMs <= firstVoiceMs) return '';
  const progress = Math.min(1, (atMs - firstVoiceMs) / Math.max(1, lastVoiceMs - firstVoiceMs));
  return words.slice(0, Math.max(1, Math.ceil(progress * words.length))).join(' ');
}

function voiceSpan(speechFrames: readonly Int16Array[]): { first: number | null; last: number | null } {
  const classifier = createMultiFeatureVadClassifier();
  let state = classifier.initialState;
  let first: number | null = null;
  let last: number | null = null;
  for (let index = 0; index < speechFrames.length; index += 1) {
    const built = buildVadFrame(classifier, state, speechFrames[index], index * FRAME_MS, SAMPLE_RATE_HZ);
    state = built.nextState;
    if (isVoiceFrame(built.frame.speech_probability)) {
      if (first === null) first = index * FRAME_MS;
      last = index * FRAME_MS;
    }
  }
  return { first, last };
}

function run(speechFrames: readonly Int16Array[], transcript: string): Outcome {
  const silence = new Int16Array(SAMPLES_PER_FRAME);
  const timeline = [
    ...speechFrames,
    ...Array.from({ length: TRAILING_SILENCE_MS / FRAME_MS }, () => silence),
  ];

  const classifier = createMultiFeatureVadClassifier();
  let classifierState = classifier.initialState;
  let vadState: VadGateState = INITIAL_VAD_GATE_STATE;
  let lastVoiceAtMs: number | null = null;
  let semanticMs: number | null = null;
  let semanticTranscript: string | null = null;
  let vadMs: number | null = null;
  let firstVoiceSeenMs: number | null = null;
  let lastVoiceSeenMs: number | null = null;
  let longestGapMs = 0;
  let longestGapAtMs: number | null = null;
  const span = voiceSpan(speechFrames);

  for (let index = 0; index < timeline.length; index += 1) {
    const atMs = index * FRAME_MS;
    const built = buildVadFrame(classifier, classifierState, timeline[index], atMs, SAMPLE_RATE_HZ);
    classifierState = built.nextState;

    // App.tsx's mic callback, in order: record the pause clock, then hand the frame to the gate.
    if (isVoiceFrame(built.frame.speech_probability)) {
      // Longest run of non-voice BETWEEN two voice frames — the intra-utterance gap that decides
      // whether a 150ms confirmed threshold cuts this speaker off mid-sentence.
      if (lastVoiceAtMs !== null && span.last !== null && atMs <= span.last) {
        const gap = atMs - lastVoiceAtMs;
        if (gap > longestGapMs) {
          longestGapMs = gap;
          longestGapAtMs = lastVoiceAtMs;
        }
      }
      lastVoiceAtMs = atMs;
      lastVoiceSeenMs = atMs;
      if (firstVoiceSeenMs === null) firstVoiceSeenMs = atMs;
    }
    const decision = reduceVadGate(vadState, built.frame);
    vadState = decision.state;
    if (vadMs === null && decision.decision.kind === 'end_of_turn') vadMs = atMs;

    // App.tsx's onTranscript, on the STT partials that would actually have arrived by now.
    const partialDue = atMs >= STT_LAG_MS && (atMs - STT_LAG_MS) % PARTIAL_INTERVAL_MS === 0;
    if (semanticMs === null && vadState.phase === 'listening' && partialDue) {
      const pauseMs = pauseMsSince(lastVoiceAtMs, atMs);
      const partial = partialAt(transcript, atMs, span.first ?? 0, span.last ?? 0);
      if (partial.length > 0 && decideEndpoint(partial, pauseMs, atMs).kind === 'endpoint') {
        semanticMs = atMs;
        semanticTranscript = partial;
      }
    }
  }
  return {
    semanticMs,
    semanticTranscript,
    vadMs,
    firstVoiceMs: firstVoiceSeenMs,
    lastVoiceMs: lastVoiceSeenMs,
    longestGapMs,
    longestGapAtMs,
    frames: timeline.length,
  };
}

console.log(
  `real recorded speech | ${SAMPLE_RATE_HZ / 1000}kHz mono | ${FRAME_MS}ms frames | classifier=${createMultiFeatureVadClassifier().source}`,
);
console.log(
  `tail = ${TRAILING_SILENCE_MS}ms digital silence | STT partials every ${PARTIAL_INTERVAL_MS}ms after a ${STT_LAG_MS}ms lag\n`,
);
let scoreable = 0;
let cutoffs = 0;
for (const testCase of CASES) {
  const outcome = run(framesFrom(testCase.file), testCase.transcript);
  if (!testCase.note.startsWith('SYNTHETIC')) {
    scoreable += 1;
    if (outcome.semanticMs !== null && outcome.lastVoiceMs !== null && outcome.semanticMs < outcome.lastVoiceMs) {
      cutoffs += 1;
    }
  }
  const saved =
    outcome.semanticMs !== null && outcome.vadMs !== null ? `${outcome.vadMs - outcome.semanticMs}ms` : 'n/a';
  console.log(`${testCase.note}`);
  console.log(`  file            ${testCase.file}`);
  console.log(`  transcript      "${testCase.transcript}"`);
  console.log(`  voice span      ${outcome.firstVoiceMs}ms -> ${outcome.lastVoiceMs}ms   (of ${outcome.frames * FRAME_MS}ms timeline)`);
  console.log(`  longest gap     ${outcome.longestGapMs}ms mid-utterance, starting at ${outcome.longestGapAtMs}ms`);
  const cutoff =
    outcome.semanticMs !== null && outcome.lastVoiceMs !== null && outcome.semanticMs < outcome.lastVoiceMs;
  console.log(
    `  semantic ends   ${outcome.semanticMs ?? 'never'}${outcome.semanticMs !== null ? 'ms' : ''}` +
      (outcome.semanticTranscript !== null ? `  on partial "${outcome.semanticTranscript}"` : '') +
      (cutoff ? '   <-- MID-UTTERANCE CUTOFF' : ''),
  );
  console.log(`  VAD ends        ${outcome.vadMs ?? 'never'}${outcome.vadMs !== null ? 'ms' : ''}   (hangover ${VAD_POST_ENDPOINT_CLOSE_MS}ms)`);
  console.log(`  saved           ${saved}\n`);
}
console.log(
  `mid-utterance cutoffs: ${cutoffs} of ${scoreable} scoreable real utterances ` +
    `(fixtures/ holds 13 WAVs; ${scoreable} carry a non-empty ground-truth transcript in fixtures.json)`,
);
