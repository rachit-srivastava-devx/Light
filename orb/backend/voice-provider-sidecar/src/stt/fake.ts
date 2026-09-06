import type { SttBackend, SttResult } from './types.js';

/**
 * Deterministic, no-network STT backend. Mirrors `backend/relay-rs/src/provider.rs`'s
 * `FakeProvider` behaviour exactly (partial every 3rd frame, fixed final transcript) so the two
 * fakes agree when the socket layer and this sidecar are both run at T0.
 */
export class FakeSttBackend implements SttBackend {
  readonly label = 'fake';
  private readonly frameCounts = new Map<string, number>();

  async pushAudio(sessionId: string, frame: Uint8Array): Promise<SttResult> {
    if (frame.length === 0) {
      throw new Error('empty audio frame');
    }
    const count = (this.frameCounts.get(sessionId) ?? 0) + 1;
    this.frameCounts.set(sessionId, count);
    if (count % 3 === 0) {
      return { text: `partial after ${count} frames`, is_final: false };
    }
    return { text: null, is_final: false };
  }

  async endTurn(sessionId: string): Promise<SttResult> {
    this.frameCounts.delete(sessionId);
    return { text: 'final transcript', is_final: true };
  }
}
