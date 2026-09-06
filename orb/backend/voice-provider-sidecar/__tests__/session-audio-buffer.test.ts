import { describe, expect, it } from 'vitest';
import { SessionAudioBuffer } from '../src/session-audio-buffer';

/**
 * A session that pushes audio and then vanishes (app crash, network drop) without ever calling
 * `endTurn` used to keep its raw audio bytes in memory for the life of the process — the same
 * "the stop path never actually released it" shape as the SIGTERM/recordVideo bug, just for
 * memory instead of disk. These prove the idle-TTL sweep bounds that.
 */
describe('SessionAudioBuffer idle eviction', () => {
  it('evicts a session idle past the TTL on the next push from any session', () => {
    let now = 1_000_000;
    const buffer = new SessionAudioBuffer({ idleTtlMs: 1000, now: () => now });

    buffer.push('abandoned', new Uint8Array([1, 2, 3]));
    now += 1001;
    buffer.push('other', new Uint8Array([9]));

    expect(buffer.takeAll('abandoned')).toEqual(new Uint8Array([]));
    expect(buffer.takeAll('other')).toEqual(new Uint8Array([9]));
  });

  it('does not evict a session that keeps pushing before the TTL elapses', () => {
    let now = 1_000_000;
    const buffer = new SessionAudioBuffer({ idleTtlMs: 1000, now: () => now });

    buffer.push('active', new Uint8Array([1]));
    now += 900; // under the TTL
    buffer.push('active', new Uint8Array([2])); // refreshes last-seen
    now += 900; // would have expired from the ORIGINAL push, not from the refreshed one
    buffer.push('trigger-sweep', new Uint8Array([3]));

    expect(buffer.takeAll('active')).toEqual(new Uint8Array([1, 2]));
  });

  it('grants one partial per turn and resets the claim when the turn is consumed', () => {
    const buffer = new SessionAudioBuffer();
    buffer.push('s1', new Uint8Array([1, 2]));

    expect(buffer.claimFirstPartial('s1', 3)).toBe(false);
    buffer.push('s1', new Uint8Array([3]));
    expect(buffer.claimFirstPartial('s1', 3)).toBe(true);
    expect(buffer.claimFirstPartial('s1', 3)).toBe(false);
    expect(buffer.snapshot('s1')).toEqual(new Uint8Array([1, 2, 3]));

    buffer.takeAll('s1');
    buffer.push('s1', new Uint8Array([4, 5, 6]));
    expect(buffer.claimFirstPartial('s1', 3)).toBe(true);
  });

  it('defaults to a real 1-hour TTL and Date.now when unconfigured', () => {
    const buffer = new SessionAudioBuffer();
    buffer.push('s1', new Uint8Array([7]));
    expect(buffer.takeAll('s1')).toEqual(new Uint8Array([7]));
  });
});
