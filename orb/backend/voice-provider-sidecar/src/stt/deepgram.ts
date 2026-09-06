import type { SttBackend, SttResult } from './types.js';
import { PROVIDER_REQUEST_TIMEOUT_MS } from '../provider-timeout.js';
import { SessionAudioBuffer } from '../session-audio-buffer.js';
import { pcm16ToWav } from '../wav.js';

const ENDPOINT = 'https://api.deepgram.com/v1/listen?model=nova-3&smart_format=true';
const FIRST_PARTIAL_AUDIO_BYTES = 16_000 * 2; // 1.0s, mono PCM16 at 16 kHz.

interface DeepgramResponse {
  results?: {
    channels?: Array<{ alternatives?: Array<{ transcript?: string }> }>;
  };
}

/**
 * Deepgram Nova-3 STT. The first second is submitted during `pushAudio` to emit a bounded early
 * partial; `endTurn` submits the full utterance once for the authoritative final.
 *
 * CONFIDENCE NOTE: moderate confidence. The `/v1/listen` prerecorded shape and response envelope
 * (`results.channels[0].alternatives[0].transcript`) are stable across Deepgram's docs history;
 * the part most likely to need correction against a real key is whether `model=nova-3` is the
 * exact query param value live on the account being used (Deepgram renames/aliases model ids).
 */
export class DeepgramSttBackend implements SttBackend {
  readonly label = 'deepgram';
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
    const response = await this.fetchImpl(ENDPOINT, {
      method: 'POST',
      headers: {
        Authorization: `Token ${this.apiKey}`,
        'Content-Type': 'audio/wav',
      },
      body: wav,
      signal: AbortSignal.timeout(PROVIDER_REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) {
      throw new Error(`deepgram stt request failed: ${response.status} ${await response.text()}`);
    }
    const body = (await response.json()) as DeepgramResponse;
    const transcript = body.results?.channels?.[0]?.alternatives?.[0]?.transcript ?? '';
    return { text: transcript || (isFinal ? '' : null), is_final: isFinal };
  }
}
