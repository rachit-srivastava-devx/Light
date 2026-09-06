import { describe, expect, it } from 'vitest';

import {
  PRESENCE_GUARD_RETRY_MS,
  PresenceEventQueue,
  activePresenceEvent,
  proactiveCheckInEvent,
  type PresenceEvent,
  type PresenceSnapshot,
} from './PresenceEventQueue';

const ready: PresenceSnapshot = {
  appActive: true,
  userTurn: false,
  muted: false,
  goalActive: true,
  hyperfocus: false,
};

function event(id: string, dueAtMs: number, priority = 1): PresenceEvent {
  return {
    id,
    kind: 'gentle_presence',
    dueAtMs,
    expiresAtMs: dueAtMs + 10_000,
    priority,
    requires: ['app_active', 'not_user_turn'],
    cancelOn: ['user_speaks', 'pause'],
  };
}

describe('PresenceEventQueue', () => {
  it('emits due events in time/priority order and removes them', () => {
    const queue = new PresenceEventQueue();
    queue.schedule(event('later', 100, 10));
    queue.schedule(event('first', 100, 20));
    queue.schedule(event('future', 500));

    expect(queue.drain(100, ready).map((item) => item.id)).toEqual(['first', 'later']);
    expect(queue.pending().map((item) => item.id)).toEqual(['future']);
  });

  it('defers a due event while the user owns the turn', () => {
    const queue = new PresenceEventQueue();
    queue.schedule(event('check', 100));

    expect(queue.drain(100, { ...ready, userTurn: true })).toEqual([]);
    expect(queue.pending()[0]?.dueAtMs).toBe(100 + PRESENCE_GUARD_RETRY_MS);
    expect(queue.drain(1_099, { ...ready, userTurn: true })).toEqual([]);
    expect(queue.drain(1_100, ready).map((item) => item.id)).toEqual(['check']);
  });

  it('cancels only events subscribed to a cancellation signal', () => {
    const queue = new PresenceEventQueue();
    queue.schedule(event('cancel-me', 100));
    queue.schedule({ ...event('keep-me', 100), cancelOn: ['background'] });

    expect(queue.cancelBy('pause')).toBe(1);
    expect(queue.drain(100, ready).map((item) => item.id)).toEqual(['keep-me']);
  });

  it('expires stale events and encodes the production check-in guardrails', () => {
    const queue = new PresenceEventQueue();
    queue.schedule({ ...event('expired', 100), expiresAtMs: 150 });
    expect(queue.drain(151, ready)).toEqual([]);

    const checkIn = proactiveCheckInEvent(1_000);
    expect(checkIn.kind).toBe('stall_assist');
    expect(checkIn.requires).toEqual(['app_active', 'not_user_turn', 'not_muted', 'goal_active', 'not_hyperfocus']);
    expect(checkIn.cancelOn).toContain('pause');
    expect(checkIn.cancelOn).toContain('error');
  });

  it('has a separate launch presence event that is not blocked by goal mechanics', () => {
    const presence = activePresenceEvent(1_000);
    expect(presence.kind).toBe('gentle_presence');
    expect(presence.dueAtMs).toBe(6_000);
    expect(presence.requires).toEqual(['app_active', 'not_user_turn', 'not_muted', 'not_hyperfocus']);
    expect(presence.cancelOn).toContain('user_speaks');
  });
});
