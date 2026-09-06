import { describe, expect, it } from 'vitest';
import { FakeSttBackend } from '../src/stt/fake';
import { FakeTtsBackend } from '../src/tts/fake';

/**
 * Proves the fake STT backend matches `backend/relay-rs/src/provider.rs`'s `FakeProvider`
 * byte-for-byte in behaviour (partial every 3rd frame, fixed final transcript, rejects empty
 * frames) — that parity is what makes T0 fake/fake mode consistent whether the Rust relay talks
 * to its own in-process fake or to this sidecar's HTTP fake.
 */
describe('FakeSttBackend', () => {
  it('returns null for frames 1 and 2, a partial on frame 3', async () => {
    const stt = new FakeSttBackend();
    await expect(stt.pushAudio('s1', new Uint8Array([1]))).resolves.toEqual({ text: null, is_final: false });
    await expect(stt.pushAudio('s1', new Uint8Array([1]))).resolves.toEqual({ text: null, is_final: false });
    await expect(stt.pushAudio('s1', new Uint8Array([1]))).resolves.toEqual({
      text: 'partial after 3 frames',
      is_final: false,
    });
  });

  it('rejects an empty frame instead of silently counting it', async () => {
    const stt = new FakeSttBackend();
    await expect(stt.pushAudio('s1', new Uint8Array([]))).rejects.toThrow(/empty audio frame/);
  });

  it('returns a fixed final transcript and resets the session counter', async () => {
    const stt = new FakeSttBackend();
    await stt.pushAudio('s1', new Uint8Array([1]));
    await expect(stt.endTurn('s1')).resolves.toEqual({ text: 'final transcript', is_final: true });
    // A fresh turn on the same session starts counting from 1 again.
    await expect(stt.pushAudio('s1', new Uint8Array([1]))).resolves.toEqual({ text: null, is_final: false });
  });

  it('tracks frame counts per session independently', async () => {
    const stt = new FakeSttBackend();
    await stt.pushAudio('a', new Uint8Array([1]));
    await stt.pushAudio('a', new Uint8Array([1]));
    await expect(stt.pushAudio('b', new Uint8Array([1]))).resolves.toEqual({ text: null, is_final: false });
    await expect(stt.pushAudio('a', new Uint8Array([1]))).resolves.toEqual({
      text: 'partial after 3 frames',
      is_final: false,
    });
  });
});

describe('FakeTtsBackend', () => {
  it('returns the same fixed two-region byte layout every call (deterministic)', async () => {
    const tts = new FakeTtsBackend();
    const a = await tts.synthesize('hello', 'orb.warm.v1', 'calm');
    const b = await tts.synthesize('different text', 'other.voice', 'urgent');
    expect(a).toEqual(b);
    expect(a.length).toBe(640);
    expect(Array.from(a.subarray(0, 320))).toEqual(new Array(320).fill(0));
    expect(Array.from(a.subarray(320, 640))).toEqual(new Array(320).fill(1));
  });
});
