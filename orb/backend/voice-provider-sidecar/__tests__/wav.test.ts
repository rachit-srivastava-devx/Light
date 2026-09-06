import { describe, expect, it } from 'vitest';
import { extractPcmFromWav, pcm16ToWav } from '../src/wav.js';

describe('extractPcmFromWav', () => {
  it('normalizes a 16 kHz WAV into the 24 kHz TTS relay format', () => {
    const pcm = Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    const wav = pcm16ToWav(pcm);
    const { pcm: extracted, wasWav } = extractPcmFromWav(wav);
    expect(wasWav).toBe(true);
    expect(extracted.length).toBe(16);
  });

  it('finds the data chunk even when a metadata chunk (e.g. LIST) comes first', () => {
    const pcm = Uint8Array.from([9, 9, 9, 9]);
    // RIFF header (12) + a fake 6-byte "LIST" chunk (8 + 6, padded to even = 14) + fmt-less
    // minimal data chunk. This repo's own pcm16ToWav never emits LIST, so this fixture is
    // hand-built to prove the walker doesn't assume `data` is always the first chunk.
    const listBody = Uint8Array.from([1, 2, 3, 4, 5, 6]);
    const header = new Uint8Array(12 + 8 + listBody.length + 8 + pcm.length);
    const view = new DataView(header.buffer);
    const writeAscii = (offset: number, text: string) => {
      for (let i = 0; i < text.length; i++) view.setUint8(offset + i, text.charCodeAt(i));
    };
    writeAscii(0, 'RIFF');
    view.setUint32(4, header.length - 8, true);
    writeAscii(8, 'WAVE');
    writeAscii(12, 'LIST');
    view.setUint32(16, listBody.length, true);
    header.set(listBody, 20);
    const dataOffset = 20 + listBody.length;
    writeAscii(dataOffset, 'data');
    view.setUint32(dataOffset + 4, pcm.length, true);
    header.set(pcm, dataOffset + 8);

    const { pcm: extracted, wasWav } = extractPcmFromWav(header);
    expect(wasWav).toBe(true);
    expect(Array.from(extracted)).toEqual(Array.from(pcm));
  });

  it('returns the input unchanged when it is not a RIFF/WAVE container at all', () => {
    // e.g. a provider that ignored the requested "wav" format and sent MP3/raw bytes instead.
    const notWav = Uint8Array.from([0xff, 0xfb, 0x90, 0x00, 1, 2, 3]);
    const { pcm, wasWav } = extractPcmFromWav(notWav);
    expect(wasWav).toBe(false);
    expect(Array.from(pcm)).toEqual(Array.from(notWav));
  });

  it('returns the input unchanged on a too-short buffer instead of throwing', () => {
    const tiny = Uint8Array.from([1, 2, 3]);
    const { pcm, wasWav } = extractPcmFromWav(tiny);
    expect(wasWav).toBe(false);
    expect(Array.from(pcm)).toEqual(Array.from(tiny));
  });

  it('returns the input unchanged when RIFF/WAVE is well-formed but has no data subchunk', () => {
    const header = new Uint8Array(12);
    const view = new DataView(header.buffer);
    const writeAscii = (offset: number, text: string) => {
      for (let i = 0; i < text.length; i++) view.setUint8(offset + i, text.charCodeAt(i));
    };
    writeAscii(0, 'RIFF');
    view.setUint32(4, 4, true);
    writeAscii(8, 'WAVE');
    const { pcm, wasWav } = extractPcmFromWav(header);
    expect(wasWav).toBe(false);
    expect(Array.from(pcm)).toEqual(Array.from(header));
  });

  it('leaves fake-backend fixture bytes (no RIFF magic) untouched', () => {
    // Matches relay-rs's FakeProvider fixture: vec![0u8; 320], vec![1u8; 320] — plain bytes, not
    // a WAV container. Must pass through unmodified so the fake/T0 path stays byte-identical.
    const fixture = new Uint8Array(320).fill(1);
    const { pcm, wasWav } = extractPcmFromWav(fixture);
    expect(wasWav).toBe(false);
    expect(Array.from(pcm)).toEqual(Array.from(fixture));
  });

  it('normalizes a valid non-16k mono PCM WAV before it reaches the headerless relay contract', () => {
    const sourceRate = 8_000;
    const sourceSamples = new Int16Array(sourceRate / 2);
    for (let index = 0; index < sourceSamples.length; index += 1) sourceSamples[index] = index;
    const pcm = new Uint8Array(sourceSamples.length * 2);
    const pcmView = new DataView(pcm.buffer);
    sourceSamples.forEach((sample, index) => pcmView.setInt16(index * 2, sample, true));
    const wav = new Uint8Array(44 + pcm.length);
    const view = new DataView(wav.buffer);
    const ascii = (offset: number, text: string) => {
      for (let index = 0; index < text.length; index += 1) view.setUint8(offset + index, text.charCodeAt(index));
    };
    ascii(0, 'RIFF');
    view.setUint32(4, wav.length - 8, true);
    ascii(8, 'WAVE');
    ascii(12, 'fmt ');
    view.setUint32(16, 16, true);
    view.setUint16(20, 1, true);
    view.setUint16(22, 1, true);
    view.setUint32(24, sourceRate, true);
    view.setUint32(28, sourceRate * 2, true);
    view.setUint16(32, 2, true);
    view.setUint16(34, 16, true);
    ascii(36, 'data');
    view.setUint32(40, pcm.length, true);
    wav.set(pcm, 44);

    const result = extractPcmFromWav(wav);
    expect(result.wasWav).toBe(true);
    expect(result.pcm.length).toBe(sourceSamples.length * 6);
  });
});
