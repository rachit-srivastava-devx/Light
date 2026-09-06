import type { SttBackend, SttResult } from './types.js';
import { PROVIDER_REQUEST_TIMEOUT_MS } from '../provider-timeout.js';
import { SessionAudioBuffer } from '../session-audio-buffer.js';
import { pcm16ToWav } from '../wav.js';

const ENDPOINT = 'https://api.sarvam.ai/speech-to-text';
const FIRST_PARTIAL_AUDIO_BYTES = 16_000 * 2; // 1.0s, mono PCM16 at 16 kHz.

/** Live-confirmed 2026-08-06: `saarika:v2` is deprecated — Sarvam's API returns a 400
 * ("Model 'saarika:v2' has been deprecated. Please use 'saarika:v2.5' instead.") on real use. */
const MODEL = 'saarika:v2.5';

/**
 * Sarvam Saarika STT. The first second is submitted during `pushAudio` to emit a bounded early
 * partial; `endTurn` submits the full utterance once for the authoritative final.
 *
 * CONFIDENCE NOTE: field names and response envelope shape (multipart `file`/`model`/
 * `language_code`, JSON `{transcript}` response) are live-confirmed working against a real key as
 * of 2026-08-06 — the model version was the one thing that had actually drifted from the docs.
 */
export class SarvamSttBackend implements SttBackend {
  readonly label = 'sarvam';
  private readonly buffer = new SessionAudioBuffer();

  constructor(private readonly apiKey: string, private readonly fetchImpl: typeof fetch = fetch) {}

  async pushAudio(sessionId: string, frame: Uint8Array): Promise<SttResult> {
    this.buffer.push(sessionId, frame);
    if (this.buffer.claimFirstPartial(sessionId, FIRST_PARTIAL_AUDIO_BYTES)) {
      return this.transcribe(this.buffer.snapshot(sessionId), false);
    }
    return { text: null, is_final: false };
  }

  async endTurn(sessionId: string): Promise<SttResult> {
    const pcm = this.buffer.takeAll(sessionId);
    if (pcm.length === 0) {
      return { text: '', is_final: true };
    }
    return this.transcribe(pcm, true);
  }

  private async transcribe(pcm: Uint8Array, isFinal: boolean): Promise<SttResult> {
    const wav = Buffer.from(pcm16ToWav(pcm));
    const form = new FormData();
    form.append('file', new Blob([wav], { type: 'audio/wav' }), 'audio.wav');
    form.append('model', MODEL);
    form.append('language_code', 'unknown');

    const response = await this.fetchImpl(ENDPOINT, {
      method: 'POST',
      headers: { 'api-subscription-key': this.apiKey },
      body: form,
      signal: AbortSignal.timeout(PROVIDER_REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) {
      throw new Error(`sarvam stt request failed: ${response.status} ${await response.text()}`);
    }
    const body = (await response.json()) as { transcript?: string };
    const transcript = body.transcript ?? '';
    return { text: transcript || (isFinal ? '' : null), is_final: isFinal };
  }
}
