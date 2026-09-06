/**
 * Declares the Web Audio graph per `docs/BUILD-DIGEST.md` §2 ("Audio graph node config"):
 *
 *   AudioBufferSourceNode(noise, loop) → Gain → Biquad(lowpass) → destination
 *   AudioBufferSourceNode(voice/cue)   → Gain → destination        [mixed over the bed]
 *
 * The graph-construction logic is a pure function of an `AudioContext`-shaped interface (injected,
 * never imported at module scope) so it is unit-testable against a fake context. §2: "All
 * scheduling via setTargetAtTime/linearRampToValueAtTime/start(when) — JS must never be in the
 * render loop" — this file only ever calls those three plus one-time initial `.value` assignment
 * before the source starts (not scheduling; the graph hasn't produced sound yet).
 *
 * The real `react-native-audio-api` import is wired only at the app's boot site, which passes its
 * `AudioContext` in as `ctx: AudioContextLike` — never imported here.
 */

import type { AudioGraphConfig } from './contracts';
import type { AudioTimeSeconds } from './contracts';
import { morphTo, type BedStatus } from './ColorMorph';
import { duckBed, restoreBed } from './DuckingMixer';

/** Minimal Web Audio `AudioParam` surface this plane schedules against. */
export interface AudioParamLike {
  value: number;
  setValueAtTime(value: number, startTime: AudioTimeSeconds): void;
  setTargetAtTime(target: number, startTime: AudioTimeSeconds, timeConstant: number): void;
  linearRampToValueAtTime(value: number, endTime: AudioTimeSeconds): void;
}

export interface AudioNodeLike {
  connect(destination: AudioNodeLike): void;
}

export interface GainNodeLike extends AudioNodeLike {
  readonly gain: AudioParamLike;
}

export interface BiquadFilterNodeLike extends AudioNodeLike {
  type: string;
  readonly frequency: AudioParamLike;
  readonly Q: AudioParamLike;
}

export interface AudioBufferSourceNodeLike extends AudioNodeLike {
  buffer: unknown;
  loop: boolean;
  start(when: AudioTimeSeconds): void;
  stop?(when?: AudioTimeSeconds): void;
}

export interface AudioContextLike {
  readonly destination: AudioNodeLike;
  readonly currentTime?: AudioTimeSeconds;
  createBufferSource(): AudioBufferSourceNodeLike;
  createGain(): GainNodeLike;
  createBiquadFilter(): BiquadFilterNodeLike;
}

/** Standard dBFS→linear-gain conversion (20·log10 inverse); not a spec number, just audio math. */
export function dbfsToLinearGain(db: number): number {
  return 10 ** (db / 20);
}

export interface BuiltAudioGraph {
  readonly bedSource: AudioBufferSourceNodeLike;
  readonly bedGain: GainNodeLike;
  readonly bedFilter: BiquadFilterNodeLike;
  readonly voiceGain: GainNodeLike;
  readonly pause: (atSeconds?: AudioTimeSeconds) => void;
  readonly recover: (atSeconds?: AudioTimeSeconds) => void;
  readonly duckVoice: (atSeconds?: AudioTimeSeconds) => AudioTimeSeconds;
  readonly restoreVoice: (atSeconds?: AudioTimeSeconds) => AudioTimeSeconds;
  readonly morphColor: (status: BedStatus, atSeconds?: AudioTimeSeconds) => AudioTimeSeconds;
  readonly shutdown: (atSeconds?: AudioTimeSeconds) => void;
}

/**
 * §2 — constructs the whole graph and starts the bed source at `when` (audio-clock seconds).
 * The bed source is the only node this plane ever starts; per INV1/§1 it is never stopped or
 * recreated afterward — that lifetime guarantee belongs to the caller, not this constructor.
 * `voiceGain` is returned so a transient per-utterance source can be connected to it later,
 * matching the config's "mixed voice branch" without this function owning that lifecycle.
 */
export function buildAudioGraph(
  ctx: AudioContextLike,
  config: AudioGraphConfig,
  noiseBuffer: unknown,
  when: AudioTimeSeconds,
): BuiltAudioGraph {
  const bedSource = ctx.createBufferSource();
  bedSource.buffer = noiseBuffer;
  bedSource.loop = config.bed.source.loop;

  const bedGain = ctx.createGain();
  bedGain.gain.value = dbfsToLinearGain(config.bed.gain.gain_db);

  const bedFilter = ctx.createBiquadFilter();
  bedFilter.type = config.bed.filter.type;
  bedFilter.frequency.value = config.bed.filter.frequency_hz;
  bedFilter.Q.value = config.bed.filter.q;

  bedSource.connect(bedGain);
  bedGain.connect(bedFilter);
  bedFilter.connect(ctx.destination);
  bedSource.start(when);

  const voiceGain = ctx.createGain();
  voiceGain.gain.value = dbfsToLinearGain(config.voice.gain.gain_db);
  voiceGain.connect(ctx.destination);

  let shutDown = false;
  const now = () => ctx.currentTime ?? when;
  const pause = (atSeconds = now()): void => {
    if (!shutDown) bedGain.gain.setTargetAtTime(0, atSeconds, 0.25 / 3);
  };
  const recover = (atSeconds = now()): void => {
    if (!shutDown) {
      bedGain.gain.setTargetAtTime(dbfsToLinearGain(config.bed.gain.gain_db), atSeconds, 0.25 / 3);
    }
  };
  const duckVoice = (atSeconds = now()): AudioTimeSeconds => {
    if (shutDown) return atSeconds;
    return duckBed(bedGain.gain, atSeconds);
  };
  const restoreVoice = (atSeconds = now()): AudioTimeSeconds => {
    if (shutDown) return atSeconds;
    return restoreBed(bedGain.gain, atSeconds);
  };
  const morphColor = (status: BedStatus, atSeconds = now()): AudioTimeSeconds => {
    if (shutDown) return atSeconds;
    return morphTo(bedFilter.frequency, status, atSeconds);
  };
  const shutdown = (atSeconds = now()): void => {
    if (shutDown) return;
    shutDown = true;
    bedGain.gain.setTargetAtTime(0, atSeconds, 0.25 / 3);
    bedSource.stop?.(atSeconds + 0.25);
  };

  return {
    bedSource,
    bedGain,
    bedFilter,
    voiceGain,
    pause,
    recover,
    duckVoice,
    restoreVoice,
    morphColor,
    shutdown,
  };
}

/** Connects one transient voice/cue source into the already-built voice branch (§2, mixed over bed). */
export function connectVoiceSource(
  voiceGain: GainNodeLike,
  source: AudioBufferSourceNodeLike,
  when: AudioTimeSeconds,
): void {
  source.connect(voiceGain);
  source.start(when);
}
