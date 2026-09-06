import { buildAudioGraph, type AudioContextLike, type BuiltAudioGraph } from './AudioGraph';
import {
  BED_BUFFER_SECONDS,
  BED_CROSSFADE_MS,
  BED_GAIN_NOMINAL_DBFS,
  BED_SAMPLE_RATE_HZ,
  type AudioGraphConfig,
} from './contracts';
import { buildNoiseBuffer, type NoiseColor } from './NoiseEngine';

export interface AudioBufferLike {
  getChannelData(channel: number): Float32Array;
}

export interface PresenceAudioContext extends AudioContextLike {
  readonly currentTime: number;
  /** Native output rate selected by the platform audio session. */
  readonly sampleRate: number;
  createBuffer(channels: number, length: number, sampleRate: number): AudioBufferLike;
}

const runtimeBeds = new WeakMap<object, BuiltAudioGraph>();

export const DEFAULT_AUDIO_GRAPH_CONFIG: AudioGraphConfig = {
  bed: {
    source: {
      loop: true,
      buffer_seconds: BED_BUFFER_SECONDS,
      sample_rate_hz: BED_SAMPLE_RATE_HZ,
      crossfade_ms: BED_CROSSFADE_MS,
    },
    gain: { gain_db: BED_GAIN_NOMINAL_DBFS },
    filter: { type: 'lowpass', frequency_hz: 850, q: 0.7 },
  },
  voice: { gain: { gain_db: 0 } },
};

export function bootPresenceBed(
  ctx: PresenceAudioContext,
  color: NoiseColor = 'brown',
  seed: number = 42,
): BuiltAudioGraph {
  const noise = buildNoiseBuffer(
    color,
    seed,
    DEFAULT_AUDIO_GRAPH_CONFIG.bed.source.buffer_seconds,
    DEFAULT_AUDIO_GRAPH_CONFIG.bed.source.sample_rate_hz,
    DEFAULT_AUDIO_GRAPH_CONFIG.bed.source.crossfade_ms,
  );
  const buffer = ctx.createBuffer(1, noise.samples.length, noise.sample_rate_hz);
  const channel = buffer.getChannelData(0);
  for (let i = 0; i < noise.samples.length; i++) {
    channel[i] = noise.samples[i] as number;
  }
  return buildAudioGraph(ctx, DEFAULT_AUDIO_GRAPH_CONFIG, buffer, ctx.currentTime);
}

/** Boots at most one bed for the lifetime of an app runtime. */
export function getOrBootPresenceBed(
  runtime: object,
  createContext: () => PresenceAudioContext,
  color: NoiseColor = 'brown',
  seed: number = 42,
): BuiltAudioGraph {
  const existing = runtimeBeds.get(runtime);
  if (existing) return existing;
  const graph = bootPresenceBed(createContext(), color, seed);
  runtimeBeds.set(runtime, graph);
  return graph;
}

/**
 * Returns the already-running bed without creating an AudioContext.
 *
 * Turn-control code uses this to schedule an ambient status morph on the same graph that app boot
 * created. A missing graph is observable as `undefined`; the hot path must never cold-boot audio
 * after the user stops speaking because buffer generation would make the 250 ms guarantee depend
 * on JS work.
 */
export function getBootedPresenceBed(runtime: object): BuiltAudioGraph | undefined {
  return runtimeBeds.get(runtime);
}
