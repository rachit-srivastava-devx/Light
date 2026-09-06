import type { SttBackend, SttResult } from './types.js';
import { PROVIDER_REQUEST_TIMEOUT_MS } from '../provider-timeout.js';
import { SessionAudioBuffer } from '../session-audio-buffer.js';
import { pcm16ToWav } from '../wav.js';

const ENDPOINT = 'https://api.meta.ai/v1/asr/transcribe';
const MODEL = 'muse-voice-transcribe-1.0';
const FIRST_PARTIAL_AUDIO_BYTES = 16_000 * 2; // 1.0s, mono PCM16 at 16 kHz.

interface MuseTranscribeResponse {
  readonly transcript?: unknown;
}

/**
 * Meta Muse Voice Transcribe, via its REST file endpoint (`POST /v1/asr/transcribe`), not the
 * `wss://.../asr/realtime` streaming endpoint — this sidecar's `SttBackend` contract is
 * frame-buffer-then-submit (see `sarvam.ts`/`deepgram.ts`), so the batch endpoint is the natural
 * fit and keeps this adapter's shape identical to its siblings. The realtime endpoint stays
 * available as a follow-up if turn-level batch latency proves too slow for barge-in.
 *
 * CONFIDENCE NOTE: unverified against a real key — built from https://dev.meta.ai/docs docs only,
 * no live-confirmed field/model-name drift check yet (compare `sarvam.ts`'s note about
 * `saarika:v2` deprecating on contact with the real API). Treat the request/response shape here as
 * best-effort until run once against a real Muse key.
 *
 * GAP: Muse returns turn-level timestamps only, no word-level timing — the blueprint's <50ms
 * word-level-partial requirement (03-VOICE-LATENCY-PIPELINE.md) is not met by this endpoint at all
 * (batch, not streaming). Do not treat this backend as barge-in-latency-equivalent to Deepgram
 * without a real evidence run.
 */
export class MuseSttBackend implements SttBackend {
  readonly label = 'muse';
  private readonly buffer = new SessionAudioBuffer();

  constructor(private readonly apiKey: string, private readonly fetchImpl: typeof fetch = fetch) {}

  async pushAudio(sessionId: string, frame: Uint8Array): Promise<SttResult> {
    this.buffer.push(sessionId, frame);
    if (this.buffer.claimFirstPartial(sessionId, FIRST_PARTIAL_AUDIO_BYTES)) {
      return this.transcribe(sessionId, this.buffer.snapshot(sessionId), false);
    }
    return { text: null, is_final: false };
  }

  async endTurn(sessionId: string): Promise<SttResult> {
    const pcm = this.buffer.takeAll(sessionId);
    if (pcm.length === 0) {
      return { text: '', is_final: true };
    }
    return this.transcribe(sessionId, pcm, true);
  }

  private async transcribe(sessionId: string, pcm: Uint8Array, isFinal: boolean): Promise<SttResult> {
    const wav = Buffer.from(pcm16ToWav(pcm));
    const form = new FormData();
    form.append(
      'request',
      new Blob([JSON.stringify({ model: MODEL, mode: 'PUSH_TO_TALK', audioEncoding: 'WAV' })], {
        type: 'application/json',
      }),
    );
    form.append('audio', new Blob([wav], { type: 'audio/wav' }), 'audio.wav');

    const url = `${ENDPOINT}?sessionId=${encodeURIComponent(sessionId)}`;
    const response = await this.fetchImpl(url, {
      method: 'POST',
      headers: { Authorization: `Bearer ${this.apiKey}` },
      body: form,
      signal: AbortSignal.timeout(PROVIDER_REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) {
      throw new Error(`muse asr request failed: ${response.status} ${await response.text()}`);
    }
    const body = (await response.json()) as MuseTranscribeResponse;
    if (typeof body.transcript !== 'string') throw new Error('muse asr response omitted transcript');
    return { text: body.transcript || (isFinal ? '' : null), is_final: isFinal };
  }
}
