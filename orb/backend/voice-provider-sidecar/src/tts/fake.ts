import type { TtsBackend } from './types.js';

/** Deterministic, no-network TTS backend: same fixed two-chunk shape `FakeProvider` in relay-rs returns. */
export class FakeTtsBackend implements TtsBackend {
  readonly label = 'fake';

  async synthesize(_text: string, _voiceId: string, _emotion: string): Promise<Uint8Array> {
    const bytes = new Uint8Array(640);
    bytes.fill(0, 0, 320);
    bytes.fill(1, 320, 640);
    return bytes;
  }
}
