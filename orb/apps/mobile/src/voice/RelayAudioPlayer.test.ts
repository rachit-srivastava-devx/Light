import { describe, expect, it, vi } from 'vitest';

import {
  pcmChunkDurationSeconds,
  pcmToAudioBuffer,
  RelayAudioPlayer,
  RELAY_AUDIO_SAMPLE_RATE_HZ,
  resampleSamples,
} from './RelayAudioPlayer';
import type { AudioBufferSourceNodeLike, AudioNodeLike, GainNodeLike } from '../presence/AudioGraph';
import type { AudioBufferLike, PresenceAudioContext } from '../presence/PresenceBoot';
import type { SpeechPort } from './SpeechPort';
import { SPEECH_16K_MONO_WAV } from './fixtures/speech-16k-mono-wav';

class FakeParam {
  value = 0;
  setValueAtTime(): void {}
  setTargetAtTime(): void {}
  linearRampToValueAtTime(): void {}
}

class FakeNode implements AudioNodeLike {
  connectedTo: AudioNodeLike[] = [];
  connect(destination: AudioNodeLike): void {
    this.connectedTo.push(destination);
  }
}

class FakeGain extends FakeNode implements GainNodeLike {
  readonly gain = new FakeParam();
}

class FakeSource extends FakeNode implements AudioBufferSourceNodeLike {
  buffer: unknown = null;
  loop = false;
  startedAt: number[] = [];
  start(when: number): void {
    this.startedAt.push(when);
  }
}

class FakeContext implements PresenceAudioContext {
  readonly destination = new FakeNode();
  currentTime = 0;
  sampleRate = 16_000;
  createdBuffers: { channels: number; length: number; sampleRate: number }[] = [];
  createdSources: FakeSource[] = [];

  createBuffer(channels: number, length: number, sampleRate: number): AudioBufferLike {
    this.createdBuffers.push({ channels, length, sampleRate });
    const data = new Float32Array(length);
    return { getChannelData: () => data };
  }
  createBufferSource(): FakeSource {
    const source = new FakeSource();
    this.createdSources.push(source);
    return source;
  }
  createGain(): FakeGain {
    return new FakeGain();
  }
  createBiquadFilter(): never {
    throw new Error('not used by RelayAudioPlayer');
  }
}

/** Builds a raw 16-bit PCM ArrayBuffer from plain sample values, mirroring mic-side fixtures. */
function pcm16(samples: number[]): ArrayBuffer {
  const view = new Int16Array(samples);
  return view.buffer;
}

function voicedPcm(sampleCount: number): ArrayBuffer {
  return pcm16(Array.from({ length: sampleCount }, (_, index) => (index % 2 === 0 ? 8_000 : -8_000)));
}

function speechPort(): SpeechPort {
  return { speak: vi.fn(), stop: vi.fn() };
}

describe('pcmChunkDurationSeconds', () => {
  it('converts byte length at the 24 kHz TTS relay rate to seconds', () => {
    // 24000 samples * 2 bytes/sample = 48000 bytes = exactly 1 second at 24kHz.
    expect(pcmChunkDurationSeconds(48_000)).toBeCloseTo(1, 10);
    expect(pcmChunkDurationSeconds(24_000)).toBeCloseTo(0.5, 10);
  });
});

describe('pcmToAudioBuffer', () => {
  it('builds a mono buffer at the given sample rate and normalizes 16-bit PCM to -1..1 float', () => {
    const ctx = new FakeContext();
    const pcm = pcm16([32_767, -32_768, 0]);
    const buffer = pcmToAudioBuffer(ctx, pcm, RELAY_AUDIO_SAMPLE_RATE_HZ);

    expect(ctx.createdBuffers).toEqual([{ channels: 1, length: 3, sampleRate: RELAY_AUDIO_SAMPLE_RATE_HZ }]);
    const channel = buffer.getChannelData(0);
    expect(channel[0]).toBeCloseTo(32_767 / 32_768, 5);
    expect(channel[1]).toBeCloseTo(-1, 5);
    expect(channel[2]).toBe(0);
  });
});

describe('resampleSamples', () => {
  it('preserves duration when converting relay audio to a native output rate', () => {
    const source = new Int16Array(16_000);
    const target = resampleSamples(source, 16_000, 24_000);

    expect(target).toHaveLength(24_000);
  });

  it('does not allocate or alter samples when rates already match', () => {
    const source = new Int16Array([1, -2, 3]);

    expect(resampleSamples(source, 16_000, 16_000)).toBe(source);
  });
});

describe('RelayAudioPlayer', () => {
  it('schedules the first chunk of a turn at the current audio-clock time', () => {
    const ctx = new FakeContext();
    ctx.currentTime = 1.5;
    const voiceGain = new FakeGain();
    const player = new RelayAudioPlayer(ctx, voiceGain);

    player.enqueueChunk(voicedPcm(4));

    expect(ctx.createdSources[0]?.startedAt).toEqual([1.5]);
    expect(ctx.createdSources[0]?.connectedTo).toEqual([voiceGain]);
  });

  it('chains subsequent chunks back-to-back after the previous chunk ends, not overlapping', () => {
    const ctx = new FakeContext();
    ctx.currentTime = 0;
    const voiceGain = new FakeGain();
    const player = new RelayAudioPlayer(ctx, voiceGain, 16_000);

    // 16000 bytes = 8000 samples = 0.5s at 16kHz.
    player.enqueueChunk(voicedPcm(8_000));
    // Clock has not advanced past the scheduled end yet (still mid-first-chunk).
    ctx.currentTime = 0.1;
    player.enqueueChunk(voicedPcm(8_000));

    expect(ctx.createdSources[0]?.startedAt).toEqual([0]);
    expect(ctx.createdSources[1]?.startedAt).toEqual([0.5]);
  });

  it('schedules from "now" instead of the stale queue tail once the clock has caught up', () => {
    const ctx = new FakeContext();
    ctx.currentTime = 0;
    const voiceGain = new FakeGain();
    const player = new RelayAudioPlayer(ctx, voiceGain, 16_000);

    player.enqueueChunk(voicedPcm(8_000)); // ends at 0.5s
    ctx.currentTime = 2.0; // well past the end of the first chunk
    player.enqueueChunk(voicedPcm(8_000));

    expect(ctx.createdSources[1]?.startedAt).toEqual([2.0]);
  });

  it('drops empty chunks without creating a source', () => {
    const ctx = new FakeContext();
    const voiceGain = new FakeGain();
    const player = new RelayAudioPlayer(ctx, voiceGain);

    player.enqueueChunk(new ArrayBuffer(0));

    expect(ctx.createdSources).toHaveLength(0);
  });

  it('completeTurn resets scheduling so the next turn starts fresh from "now"', () => {
    const ctx = new FakeContext();
    ctx.currentTime = 0;
    const voiceGain = new FakeGain();
    const player = new RelayAudioPlayer(ctx, voiceGain, 16_000);

    player.enqueueChunk(voicedPcm(8_000)); // queued to end at 0.5s
    player.completeTurn();
    ctx.currentTime = 0.1; // still "inside" where the old chunk would have been playing
    player.enqueueChunk(voicedPcm(8_000));

    expect(ctx.createdSources[1]?.startedAt).toEqual([0.1]);
  });

  it('reports one playback summary and keeps the mic guard active through scheduled audio', () => {
    const ctx = new FakeContext();
    const log = vi.fn();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), 16_000, undefined, { log });

    player.enqueueChunk(voicedPcm(8_000));
    expect(player.isPlaying()).toBe(true);
    player.completeTurn();
    ctx.currentTime = 0.49;
    expect(player.isPlaying()).toBe(true);
    ctx.currentTime = 0.5;
    expect(player.isPlaying()).toBe(false);
    expect(log).toHaveBeenCalledWith(
      'relay_audio.playback_summary',
      expect.objectContaining({ chunks: 1, bytes: 16_000, silent_chunks: 0, duration_ms: 500 }),
    );
    expect(log.mock.calls.filter(([event]) => event === 'relay_audio.playback_chunk')).toHaveLength(0);
  });

  it('decodes a WAV fixture before creating the native-rate playback buffer', () => {
    const ctx = new FakeContext();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), 16_000);

    player.enqueueChunk(SPEECH_16K_MONO_WAV.buffer.slice(0));

    expect(ctx.createdBuffers[0]).toEqual({ channels: 1, length: 640, sampleRate: 16_000 });
    expect(ctx.createdSources).toHaveLength(1);
  });

  it('resamples 16 kHz TTS to the native audio context rate before playback', () => {
    const ctx = new FakeContext();
    ctx.sampleRate = 24_000;
    const player = new RelayAudioPlayer(ctx, new FakeGain(), 16_000);

    player.enqueueChunk(voicedPcm(160));

    expect(ctx.createdBuffers[0]).toEqual({ channels: 1, length: 240, sampleRate: 24_000 });
    expect(ctx.createdSources).toHaveLength(1);
  });

  it('drops fake/non-speech bytes and speaks a typed failure through the injected port', async () => {
    const ctx = new FakeContext();
    const port = speechPort();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), 16_000, port);

    player.enqueueChunk(new Uint8Array(320).fill(1).buffer);
    player.enqueueChunk(new Uint8Array(320).fill(1).buffer);
    player.completeTurn();
    await Promise.resolve();

    expect(ctx.createdSources).toHaveLength(0);
    expect(port.speak).toHaveBeenCalledTimes(1);
    expect(port.speak).toHaveBeenCalledWith(
      'I couldn’t play that response. Please try again.',
      'orb.warm.v1',
      'supportive',
    );
  });

  it('speaks provider failure but stays silent for a user pause', async () => {
    const ctx = new FakeContext();
    const port = speechPort();
    const player = new RelayAudioPlayer(ctx, new FakeGain(), 16_000, port);

    player.handleClosing('provider_failure');
    player.handleClosing('provider_failure');
    player.handleClosing('user_pause');
    await Promise.resolve();

    expect(port.speak).toHaveBeenCalledTimes(1);
    expect(port.speak).toHaveBeenCalledWith(
      'I’m having trouble with my voice connection, but I’m still here. Please try again.',
      'orb.warm.v1',
      'supportive',
    );
  });
});
