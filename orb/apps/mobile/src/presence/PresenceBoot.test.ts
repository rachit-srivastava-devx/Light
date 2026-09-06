import { describe, expect, it } from 'vitest';

import { bootPresenceBed, getBootedPresenceBed, getOrBootPresenceBed } from './PresenceBoot';
import { BED_BUFFER_SECONDS, BED_SAMPLE_RATE_HZ } from './contracts';
import type { AudioBufferSourceNodeLike, AudioContextLike } from './AudioGraph';

class FakeParam {
  value = 0;
  setValueAtTime(): void {}
  setTargetAtTime(): void {}
  linearRampToValueAtTime(): void {}
}

class FakeNode {
  connections: unknown[] = [];
  connect(destination: unknown): void {
    this.connections.push(destination);
  }
}

class FakeSource extends FakeNode implements AudioBufferSourceNodeLike {
  buffer: unknown;
  loop = false;
  startedAt: number | null = null;
  stoppedAt: number | null = null;
  start(when: number): void {
    this.startedAt = when;
  }
  stop(when?: number): void {
    this.stoppedAt = when ?? 0;
  }
}

class FakeGain extends FakeNode {
  readonly gain = new FakeParam();
}

class FakeBiquad extends FakeNode {
  type = '';
  readonly frequency = new FakeParam();
  readonly Q = new FakeParam();
}

class FakeContext implements AudioContextLike {
  readonly destination = new FakeNode();
  readonly currentTime = 1.25;
  readonly sampleRate = 16_000;
  readonly buffers: Float32Array[] = [];

  createBuffer(_channels: number, length: number): { getChannelData: () => Float32Array } {
    const channel = new Float32Array(length);
    this.buffers.push(channel);
    return { getChannelData: () => channel };
  }

  createBufferSource(): FakeSource {
    return new FakeSource();
  }

  createGain(): FakeGain {
    return new FakeGain();
  }

  createBiquadFilter(): FakeBiquad {
    return new FakeBiquad();
  }
}

describe('bootPresenceBed', () => {
  it('starts the looped bed source at app-open audio time', () => {
    const ctx = new FakeContext();
    const graph = bootPresenceBed(ctx);

    expect(graph.bedSource.loop).toBe(true);
    expect((graph.bedSource as FakeSource).startedAt).toBe(1.25);
    expect(ctx.buffers[0]?.length).toBe(BED_BUFFER_SECONDS * BED_SAMPLE_RATE_HZ);
  });

  it('boots one graph for an app runtime across rerenders', () => {
    const runtime = {};
    let contextCreations = 0;
    const first = getOrBootPresenceBed(runtime, () => {
      contextCreations += 1;
      return new FakeContext();
    });
    const second = getOrBootPresenceBed(runtime, () => {
      contextCreations += 1;
      return new FakeContext();
    });

    expect(second).toBe(first);
    expect(getBootedPresenceBed(runtime)).toBe(first);
    expect(contextCreations).toBe(1);
  });

  it('does not cold-boot audio when turn control only asks for an existing bed', () => {
    expect(getBootedPresenceBed({})).toBeUndefined();
  });

  it('exposes explicit pause, recover, and terminal shutdown controls', () => {
    const ctx = new FakeContext();
    const graph = bootPresenceBed(ctx);
    const gain = graph.bedGain.gain as FakeParam;
    const source = graph.bedSource as FakeSource;

    graph.pause(2);
    graph.recover(3);
    graph.shutdown(4);
    graph.recover(5);
    graph.shutdown(6);

    expect(gain.value).toBeCloseTo(0.12589254117941673, 10);
    expect(source.stoppedAt).toBe(4.25);
  });
});
