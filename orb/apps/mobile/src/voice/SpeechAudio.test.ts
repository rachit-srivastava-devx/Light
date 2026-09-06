import { describe, expect, it } from 'vitest';

import {
  SPEECH_BITS_PER_SAMPLE,
  SPEECH_CHANNELS,
  SPEECH_SAMPLE_RATE_HZ,
  SpeechAudioError,
  decodeRemoteTtsAudio,
} from './SpeechAudio';
import { SPEECH_16K_MONO_WAV } from './fixtures/speech-16k-mono-wav';

describe('decodeRemoteTtsAudio', () => {
  it('decodes the WAV smoke fixture into playable 16-bit mono 24 kHz TTS audio', () => {
    const audio = decodeRemoteTtsAudio(SPEECH_16K_MONO_WAV);

    expect(audio.sample_rate_hz).toBe(SPEECH_SAMPLE_RATE_HZ);
    expect(audio.channels).toBe(SPEECH_CHANNELS);
    expect(audio.bits_per_sample).toBe(SPEECH_BITS_PER_SAMPLE);
    expect(audio.duration_ms).toBe(40);
    expect(audio.samples.length).toBe(960);
    expect(Math.max(...audio.samples)).toBeGreaterThan(7_000);
  });

  it('normalizes explicitly described stereo 8 kHz PCM to mono 24 kHz', () => {
    const source = new Int16Array([0, 0, 4_000, 4_000, -4_000, -4_000, 8_000, 8_000]);
    const audio = decodeRemoteTtsAudio(source.buffer, {
      kind: 'pcm_s16le',
      sample_rate_hz: 8_000,
      channels: 2,
    });

    expect(audio.sample_rate_hz).toBe(SPEECH_SAMPLE_RATE_HZ);
    expect(audio.channels).toBe(1);
    expect(audio.samples.length).toBe(12);
    expect(audio.samples[0]).toBe(0);
    expect(audio.samples[6]).toBe(-4_000);
    expect(audio.samples[11]).toBe(8_000);
  });

  it('accepts Fish streaming WAV size sentinels instead of rejecting valid audio', () => {
    const fishWav = SPEECH_16K_MONO_WAV.slice();
    const view = new DataView(fishWav.buffer);
    view.setUint32(4, 0xffffffff, true);
    view.setUint32(40, 0xffffffff, true);

    const audio = decodeRemoteTtsAudio(fishWav);

    expect(audio.sample_rate_hz).toBe(SPEECH_SAMPLE_RATE_HZ);
    expect(audio.samples.length).toBe(960);
  });

  it('refuses arbitrary binary chunks when no format is declared', () => {
    expect(() => decodeRemoteTtsAudio(new Uint8Array(320))).toThrowError(
      expect.objectContaining<Partial<SpeechAudioError>>({ code: 'format_required' }),
    );
  });

  it('refuses zero/one-filled fake chunks even when labelled as PCM', () => {
    expect(() =>
      decodeRemoteTtsAudio(new Uint8Array(320).fill(1), {
        kind: 'pcm_s16le',
        sample_rate_hz: SPEECH_SAMPLE_RATE_HZ,
        channels: 1,
      }),
    ).toThrowError(expect.objectContaining<Partial<SpeechAudioError>>({ code: 'non_speech_audio' }));
  });

  it('fails closed on compressed/non-PCM WAV instead of passing bytes to playback', () => {
    const compressedHeader = SPEECH_16K_MONO_WAV.slice();
    new DataView(compressedHeader.buffer).setUint16(20, 3, true);
    expect(() => decodeRemoteTtsAudio(compressedHeader)).toThrowError(
      expect.objectContaining<Partial<SpeechAudioError>>({ code: 'unsupported_wav' }),
    );
  });
});
