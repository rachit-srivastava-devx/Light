import { describe, expect, it, vi } from 'vitest';

import {
  bindPresenceToAppLifecycle,
  shouldRecoverOnSessionStateChange,
  type AppLifecycleSource,
  type AppLifecycleState,
} from './PresenceLifecycle';

function fakeSource() {
  let listener: ((state: AppLifecycleState) => void) | null = null;
  const remove = vi.fn();
  const source: AppLifecycleSource = {
    addEventListener(type, next) {
      expect(type).toBe('change');
      listener = next;
      return { remove };
    },
  };
  return {
    source,
    remove,
    emit(state: AppLifecycleState) {
      listener?.(state);
    },
  };
}

describe('bindPresenceToAppLifecycle', () => {
  it('pauses the bed while backgrounded and does NOT auto-recover it on return to foreground', () => {
    // Per blueprints/ADHD-Focus-Orb-L8-Deep-Dive/02-ALWAYS-ON-AUDIO-ENGINE.md §5/§5.1: "App
    // backgrounded / loses focus" is classified as a *user-intent* pause (not the *transient*
    // class — phone calls, Siri, audio-stack reset — which recovers automatically with a
    // cover). §5.1 enumerates "Bed stops" as step 1 of the pause contract and states plainly:
    // "Resume is explicit (the user returns and taps)... The session does not silently restart
    // itself when the app comes back to the foreground — the user decides when listening
    // resumes." So AppState flipping back to 'active' — including a momentary
    // notification-shade dismissal that never even reaches 'background' — must never itself
    // call recover(). Only a genuine explicit action (App.tsx's onStart, wired to a user tap)
    // or real in-session activity (the envelope-state effect) may bring the bed back.
    const lifecycle = fakeSource();
    const graph = { pause: vi.fn(), recover: vi.fn() };
    const subscription = bindPresenceToAppLifecycle(lifecycle.source, () => graph as never);

    lifecycle.emit('background');
    lifecycle.emit('inactive');
    lifecycle.emit('active');
    subscription.remove();

    expect(graph.pause).toHaveBeenCalledTimes(2);
    expect(graph.recover).not.toHaveBeenCalled();
    expect(lifecycle.remove).toHaveBeenCalledTimes(1);
  });

  it('notifies the event scheduler without changing audio behavior', () => {
    const lifecycle = fakeSource();
    const states: AppLifecycleState[] = [];
    bindPresenceToAppLifecycle(lifecycle.source, () => null, (state) => states.push(state));

    lifecycle.emit('background');
    lifecycle.emit('active');

    expect(states).toEqual(['background', 'active']);
  });
});

describe('shouldRecoverOnSessionStateChange (B6 — delayed silent resume)', () => {
  // B4 fixed the INSTANT case: AppState -> 'active' must never itself call recover(). But
  // App.tsx's `bindPresenceToAppLifecycle` callback schedules a fresh `proactiveCheckInEvent` the
  // moment the app returns to the foreground (PresenceEventQueue.ts: dueAtMs = nowMs + 10_000).
  // If the user has an active goal, isn't muted, and doesn't speak or tap anything, that timer
  // fires `checkIn()` ~10s later, `checkIn()` dispatches a new envelope (WORKING -> CHECK_IN),
  // and App.tsx's envelope-state effect previously called `recover()` off THAT state change alone
  // — silently un-pausing the bed, just delayed by 10s instead of instant. Same class of
  // violation as B4, delayed rather than removed.
  it('does NOT recover when the envelope change is a proactive (check-in-driven) update', () => {
    expect(shouldRecoverOnSessionStateChange('CHECK_IN', 'proactive')).toBe(false);
    expect(shouldRecoverOnSessionStateChange('WORKING', 'proactive')).toBe(false);
  });

  it('still recovers on a genuine user-driven envelope change (the pre-existing contract)', () => {
    expect(shouldRecoverOnSessionStateChange('CHECK_IN', 'user_action')).toBe(true);
    expect(shouldRecoverOnSessionStateChange('WORKING', 'user_action')).toBe(true);
  });

  it('never recovers into INTERRUPTED, regardless of source', () => {
    expect(shouldRecoverOnSessionStateChange('INTERRUPTED', 'user_action')).toBe(false);
    expect(shouldRecoverOnSessionStateChange('INTERRUPTED', 'proactive')).toBe(false);
  });
});

describe('B6 end-to-end: foreground-return check-in must not silently resume the bed', () => {
  it('reproduces the real timer path — a stall_assist event fired ~10s after foreground return is a proactive update, not a user action', async () => {
    const { PresenceEventQueue, proactiveCheckInEvent } = await import('./PresenceEventQueue');
    const queue = new PresenceEventQueue();
    const graph = { pause: vi.fn(), recover: vi.fn() };

    // App backgrounded, then returns to foreground: bindPresenceToAppLifecycle's callback
    // (App.tsx L724-731) pauses the bed and cancels the pending check-in...
    graph.pause();
    queue.cancelBy('background');
    // ...then, the instant AppState flips back to 'active', schedules a fresh one — this is the
    // exact call at App.tsx L727.
    const foregroundReturnMs = 1_000_000;
    queue.schedule(proactiveCheckInEvent(foregroundReturnMs));

    // No user action for the next 10s: not muted, has an active goal, not currently their turn.
    const dueEvents = queue.drain(foregroundReturnMs + 10_000, {
      appActive: true,
      userTurn: false,
      muted: false,
      goalActive: true,
      hyperfocus: false,
    });

    expect(dueEvents).toHaveLength(1);
    expect(dueEvents[0]?.kind).toBe('stall_assist');

    // This is the envelope update `checkIn()` would produce off that event — it is a proactive
    // update, so per the guard it must not recover the bed App.tsx paused on backgrounding.
    if (shouldRecoverOnSessionStateChange('CHECK_IN', 'proactive')) {
      graph.recover();
    }

    expect(graph.recover).not.toHaveBeenCalled();
  });
});
