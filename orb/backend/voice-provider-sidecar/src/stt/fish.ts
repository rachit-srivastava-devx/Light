import type { SttBackend, SttResult } from './types.js';
import { PROVIDER_REQUEST_TIMEOUT_MS } from '../provider-timeout.js';
import { SessionAudioBuffer } from '../session-audio-buffer.js';
import { pcm16ToWav } from '../wav.js';

const ENDPOINT = 'https://api.fish.audio/v1/asr';

interface FishAsrResponse {
  readonly text?: unknown;
}

/**
 * Fish Audio ASR is a batch endpoint in the current public API: it accepts multipart/form-data
 * and returns a complete transcript. The relay contract is frame-oriented, so frames are buffered
 * per session and submitted once on end-of-turn; pushAudio remains an honest non-final response.
 */
export class FishSttBackend implements SttBackend {
  readonly label = 'fish';
  private readonly buffer = new SessionAudioBuffer();

  constructor(private readonly apiKey: string, private readonly fetchImpl: typeof fetch = fetch) {}

  async pushAudio(sessionId: string, frame: Uint8Array): Promise<SttResult> {
    this.buffer.push(sessionId, frame);
    return { text: null, is_final: false };
  }

  async endTurn(sessionId: string): Promise<SttResult> {
    const pcm = this.buffer.takeAll(sessionId);
    if (pcm.length === 0) return { text: '', is_final: true };

    const form = new FormData();
    const wav = Buffer.from(pcm16ToWav(pcm));
    form.append('audio', new Blob([wav], { type: 'audio/wav' }), 'audio.wav');
    form.append('language', 'en');
    form.append('ignore_timestamps', 'true');

    const response = await this.fetchImpl(ENDPOINT, {
      method: 'POST',
      headers: { Authorization: `Bearer ${this.apiKey}` },
      body: form,
      signal: AbortSignal.timeout(PROVIDER_REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) {
      throw new Error(`fish audio asr request failed: ${response.status} ${await response.text()}`);
    }
    const body = (await response.json()) as FishAsrResponse;
    if (typeof body.text !== 'string') throw new Error('fish audio asr response omitted text');
    return { text: body.text, is_final: true };
  }
}
