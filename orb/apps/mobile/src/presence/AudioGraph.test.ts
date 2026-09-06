import { describe, expect, it } from 'vitest';
import { buildAudioGraph, connectVoiceSource, dbfsToLinearGain } from './AudioGraph';
import type {
  AudioBufferSourceNodeLike,
  AudioContextLike,
  AudioNodeLike,
  AudioParamLike,
  BiquadFilterNodeLike,
  GainNodeLike,
} from './AudioGraph';
import { BED_STATUS_CUTOFF_HZ } from './ColorMorph';
import { BED_GAIN_DUCKED_DBFS, BED_GAIN_NOMINAL_DBFS, type AudioGraphConfig } from './contracts';

function fakeParam(): AudioParamLike & { calls: string[] } {
  const calls: string[] = [];
  return {
    value: 0,
    calls,
    setValueAtTime(value, startTime) {
      calls.push(`setValueAtTime(${value},${startTime})`);
    },
    setTargetAtTime(target, startTime, timeConstant) {
      calls.push(`setTargetAtTime(${target},${startTime},${timeConstant})`);
    },
    linearRampToValueAtTime(value, endTime) {
      calls.push(`linearRampToValueAtTime(${value},${endTime})`);
    },
  };
}

class FakeNode implements AudioNodeLike {
  connectedTo: AudioNodeLike[] = [];
  connect(destination: AudioNodeLike): void {
    this.connectedTo.push(destination);
  }
}

class FakeGainNode extends FakeNode implements GainNodeLike {
  readonly gain = fakeParam();
}

class FakeBiquadFilterNode extends FakeNode implements BiquadFilterNodeLike {
  type = '';
  readonly frequency = fakeParam();
  readonly Q = fakeParam();
}

class FakeBufferSourceNode extends FakeNode implements AudioBufferSourceNodeLike {
  buffer: unknown = null;
  loop = false;
  startedAt: number[] = [];
  stoppedAt: number[] = [];
  start(when: number): void {
    this.startedAt.push(when);
  }
  stop(when?: number): void {
    this.stoppedAt.push(when ?? 0);
  }
}

class FakeAudioContext implements AudioContextLike {
  readonly destination = new FakeNode();
  readonly currentTime = 0;
  createdSources: FakeBufferSourceNode[] = [];
  createdGains: FakeGainNode[] = [];
  createdFilters: FakeBiquadFilterNode[] = [];

  createBufferSource(): AudioBufferSourceNodeLike {
    const node = new FakeBufferSourceNode();
    this.createdSources.push(node);
    return node;
  }
  createGain(): GainNodeLike {
    const node = new FakeGainNode();
    this.createdGains.push(node);
    return node;
  }
  createBiquadFilter(): BiquadFilterNodeLike {
    const node = new FakeBiquadFilterNode();
    this.createdFilters.push(node);
    return node;
  }
}

const config: AudioGraphConfig = {
  bed: {
    source: { loop: true, buffer_seconds: 30, sample_rate_hz: 48_000, crossfade_ms: 250 },
    gain: { gain_db: -18 },
    filter: { type: 'lowpass', frequency_hz: 800, q: 0.7 },
  },
  voice: { gain: { gain_db: -6 } },
};

describe('dbfsToLinearGain', () => {
  it('maps 0 dBFS to unity gain', () => {
    expect(dbfsToLinearGain(0)).toBeCloseTo(1, 10);
  });
  it('maps -20 dBFS to 0.1 linear gain', () => {
    expect(dbfsToLinearGain(-20)).toBeCloseTo(0.1, 10);
  });
});

describe('buildAudioGraph', () => {
  it('wires the bed chain in §2 order: source → gain → biquad(lowpass) → destination', () => {
    const ctx = new FakeAudioContext();
    const { bedSource, bedGain, bedFilter } = buildAudioGraph(ctx, config, 'noise-buffer', 1.5);

    expect((bedSource as FakeBufferSourceNode).connectedTo).toEqual([bedGain]);
    expect((bedGain as FakeGainNode).connectedTo).toEqual([bedFilter]);
    expect((bedFilter as FakeBiquadFilterNode).connectedTo).toEqual([ctx.destination]);
  });

  it('sets loop=true, the buffer, and starts the source at `when` (§2 start(when))', () => {
    const ctx = new FakeAudioContext();
    const { bedSource } = buildAudioGraph(ctx, config, 'noise-buffer', 2.25);
    const source = bedSource as FakeBufferSourceNode;
    expect(source.loop).toBe(true);
    expect(source.buffer).toBe('noise-buffer');
    expect(source.startedAt).toEqual([2.25]);
  });

  it('applies dBFS-converted gain and the exact filter params from config', () => {
    const ctx = new FakeAudioContext();
    const { bedGain, bedFilter, voiceGain } = buildAudioGraph(ctx, config, 'buf', 0);
    expect(bedGain.gain.value).toBeCloseTo(dbfsToLinearGain(-18), 10);
    expect(bedFilter.type).toBe('lowpass');
    expect(bedFilter.frequency.value).toBe(800);
    expect(bedFilter.Q.value).toBe(0.7);
    expect(voiceGain.gain.value).toBeCloseTo(dbfsToLinearGain(-6), 10);
  });

  it('connects the persistent voice gain to destination, separate from the bed chain', () => {
    const ctx = new FakeAudioContext();
    const { voiceGain } = buildAudioGraph(ctx, config, 'buf', 0);
    expect((voiceGain as FakeGainNode).connectedTo).toEqual([ctx.destination]);
  });

  it('never calls start on more than one node (only the bed source starts here)', () => {
    const ctx = new FakeAudioContext();
    buildAudioGraph(ctx, config, 'buf', 0);
    const starts = ctx.createdSources.filter((s) => s.startedAt.length > 0);
    expect(starts.length).toBe(1);
  });
});

describe('connectVoiceSource', () => {
  it('connects and starts a transient voice source into the existing voice gain', () => {
    const ctx = new FakeAudioContext();
    const { voiceGain } = buildAudioGraph(ctx, config, 'buf', 0);
    const cue = ctx.createBufferSource() as FakeBufferSourceNode;
    connectVoiceSource(voiceGain, cue, 3.0);
    expect(cue.connectedTo).toEqual([voiceGain]);
    expect(cue.startedAt).toEqual([3.0]);
  });
});

describe('audio graph lifecycle', () => {
  it('pauses and recovers by scheduling the persistent bed gain, then shuts down once', () => {
    const ctx = new FakeAudioContext();
    const graph = buildAudioGraph(ctx, config, 'buf', 0);
    const gain = graph.bedGain.gain as ReturnType<typeof fakeParam>;
    const source = graph.bedSource as FakeBufferSourceNode;

    graph.pause(1);
    graph.recover(2);
    graph.shutdown(3);
    graph.shutdown(4);
    graph.recover(5);

    expect(gain.calls).toEqual([
      'setTargetAtTime(0,1,0.08333333333333333)',
      `setTargetAtTime(${dbfsToLinearGain(-18)},2,0.08333333333333333)`,
      'setTargetAtTime(0,3,0.08333333333333333)',
    ]);
    expect(source.stoppedAt).toEqual([3.25]);
  });

  it('exposes duck, restore, and color morph as runtime graph operations', () => {
    const ctx = new FakeAudioContext();
    const graph = buildAudioGraph(ctx, config, 'buf', 0);
    const gain = graph.bedGain.gain as ReturnType<typeof fakeParam>;
    const frequency = graph.bedFilter.frequency as ReturnType<typeof fakeParam>;

    graph.duckVoice(1);
    graph.restoreVoice(2);
    graph.morphColor('thinking', 3);

    expect(gain.calls).toEqual([
      `setTargetAtTime(${dbfsToLinearGain(BED_GAIN_DUCKED_DBFS)},1,0.049999999999999996)`,
      `setTargetAtTime(${dbfsToLinearGain(BED_GAIN_NOMINAL_DBFS)},2,0.049999999999999996)`,
    ]);
    expect(frequency.calls).toEqual([
      `setTargetAtTime(${BED_STATUS_CUTOFF_HZ.thinking},3,0.13333333333333333)`,
    ]);
  });
});
