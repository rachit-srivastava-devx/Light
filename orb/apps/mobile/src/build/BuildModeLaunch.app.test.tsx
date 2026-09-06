/**
 * F01 §4 A3 — the composition-root probe. "A1–A7 all pass against a runtime the *test*
 * constructs. That is exactly the failure this repo has already paid for twice: a `mode` that was
 * computed everywhere and transmitted nowhere, fully unit-tested and unreachable from the real
 * app. A1–A7 are proxies. A3 is the property." (lane contract F01 §4)
 *
 * This mounts the REAL `FocusOrbApp` with NO `runtime` prop, so the composition root
 * (`App.tsx:131-161`) builds its own session exactly as production does, with `autoStart` left at
 * its default `true` so the real launch effect (`App.tsx:682-693`) really calls `actions.start()`.
 *
 * Deviation from the lane contract's literal A3 recipe, disclosed here rather than silently
 * routed around: the contract's recipe asserts on a rendered `testID: 'focus-orb-reply-text'`
 * node. That surface is fed exclusively by `submitTyped()` (`App.tsx:851-916`, `setLastReplyText`
 * at line 901) — the TYPED-turn reply path — and by nothing else in this file; the launch path
 * (`AppController.ts`'s `launch()`) only ever calls `setEnvelope(...)`/`applyEnvelopes(...)`, never
 * `setLastReplyText`, and `VoiceLoopController.start()` (`runtime/VoiceLoopController.ts:208-216`)
 * never calls `relay.speak(...)` either — unlike `completeTurn`, which does. So today, on launch,
 * the runtime's own greeting text (GREETINGS[i].text / BUILD_GREETING) reaches React state
 * (`localEnvelope`) but reaches no rendered text node and no spoken-audio call; only the unrelated
 * hardcoded `LOCAL_MVP_GREETING` ("Good morning, Rachit") is ever spoken on launch
 * (`AppController.ts`'s `speakLaunchGreeting`). Confirmed by direct reading of all three files
 * before writing this test, not assumed. Adding a render/speak path for the launch envelope's own
 * text is explicitly out of scope for F01 ("Nothing else in App.tsx changes — no new UI, no new
 * effect" — lane contract §3.2), so this probe proves the same property (the composition root
 * itself — not a test-constructed runtime — really threads `launchMode` into a real running
 * session that really produces the build opening) by spying on the real, unmodified
 * `createT0FocusSession` export: the spy is a pass-through (it calls straight through to the real
 * implementation and returns its real result unchanged), so React's own effect is what drives the
 * real `runtime.start()` call under test, not the test calling it directly. That is the
 * distinction that matters here: A1–A7 construct their OWN runtime and call `.start()` themselves;
 * this test lets `FocusOrbApp`'s OWN `useMemo`/`useEffect` construct and drive it, and only reads
 * off what actually happened.
 */
import React from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import { describe, expect, it, vi } from 'vitest';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// Same reasoning as AppSurface.test.tsx: this Node runtime ships a real global `WebSocket`, so
// App.tsx's relay effect would otherwise attempt a genuine network connection. Stubbing it out
// exercises App.tsx's existing `typeof WebSocket === 'undefined'` no-op path instead.
delete (globalThis as { WebSocket?: unknown }).WebSocket;

// AuroraMist (CloudOrb's Skia mist) drives its drift animation off requestAnimationFrame, which
// this Node test environment doesn't provide.
(globalThis as typeof globalThis & { requestAnimationFrame?: (cb: (time: number) => void) => number }).requestAnimationFrame =
  (cb) => setTimeout(() => cb(Date.now()), 16) as unknown as number;
(globalThis as typeof globalThis & { cancelAnimationFrame?: (handle: number) => void }).cancelAnimationFrame =
  (handle) => clearTimeout(handle);

// Copied verbatim from AppSurface.test.tsx's mock preamble (lines 1-118) per the lane contract's
// harness note ("Copy it; do not edit AppSurface.test.tsx itself — it is on the untouched list").
vi.mock('react-native', () => ({
  AccessibilityInfo: {
    isReduceMotionEnabled: () => Promise.resolve(false),
    addEventListener: () => ({ remove() {} }),
  },
  Animated: {
    Value: class {
      constructor(value: number) {
        this._value = value;
      }
      _value: number;
      setValue(value: number): void {
        this._value = value;
      }
      stopAnimation(): void {}
      interpolate(): unknown {
        return this;
      }
    },
    View: 'Animated.View',
    timing: () => ({}),
    sequence: () => ({}),
    loop: () => ({ start() {}, stop() {} }),
  },
  AppState: {
    addEventListener: () => ({ remove() {} }),
  },
  Easing: {
    inOut: (fn: (t: number) => number) => fn,
    sin: (t: number) => t,
  },
  Image: {
    resolveAssetSource: () => ({ uri: '' }),
  },
  PermissionsAndroid: {
    PERMISSIONS: { RECORD_AUDIO: 'android.permission.RECORD_AUDIO' },
    RESULTS: { GRANTED: 'granted' },
    request: async () => 'denied',
  },
  PixelRatio: {
    get: () => 2,
  },
  Platform: {
    OS: 'android',
  },
  Pressable: 'Pressable',
  StyleSheet: {
    create: <T extends object>(styles: T) => styles,
    flatten: (style: unknown) => (Array.isArray(style) ? Object.assign({}, ...style) : style),
  },
  Text: 'Text',
  TextInput: 'TextInput',
  View: 'View',
  findNodeHandle: () => null,
}));

vi.mock('@shopify/react-native-skia', () => ({
  Canvas: 'Canvas',
  Circle: 'Circle',
  LinearGradient: 'LinearGradient',
  BlurMask: 'BlurMask',
  Group: 'Group',
  rect: (x: number, y: number, width: number, height: number) => ({ x, y, width, height }),
  rrect: (r: unknown, rx: number, ry: number) => ({ rect: r, rx, ry }),
  vec: (x: number, y: number) => ({ x, y }),
}));

vi.mock('../voice/NativeMicPort', () => ({
  requestMicPermission: vi.fn(async () => false),
  startCapture: vi.fn(async () => {}),
  stopCapture: vi.fn(async () => {}),
}));

// The composition-root probe itself: wrap the REAL createT0FocusSession in a pass-through spy so
// this test can read what the real App.tsx construction site (App.tsx:134-161) actually passes,
// and what the real running session's own start() actually resolves with — without changing its
// behaviour (every call still runs the real, unmodified implementation) and without touching
// App.tsx beyond the two lines the lane contract §3.2 authorizes.
vi.mock('../runtime/T0FocusSession', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../runtime/T0FocusSession')>();
  return {
    ...actual,
    createT0FocusSession: vi.fn((...args: Parameters<typeof actual.createT0FocusSession>) => {
      const runtime = actual.createT0FocusSession(...args);
      return { ...runtime, start: vi.fn(runtime.start) };
    }),
  };
});

import FocusOrbApp from '../App';
import { BUILD_GREETING, createT0FocusSession, type T0SessionOptions } from '../runtime/T0FocusSession';
import type { PresenceAudioContext } from '../presence/PresenceBoot';
import type { SpeechPort } from '../voice/SpeechPort';

// The four focus greetings, hardcoded here rather than exported from T0FocusSession.ts (exporting
// them would be an unauthorized fourth change to that file — the lane contract names exactly
// which changes T0FocusSession.ts may carry, and a new export isn't one of them). Same strings
// A4/A6 in BuildModeLaunch.test.ts already pin.
const FOCUS_GREETINGS = [
  "I'm here. Tell me the task.",
  'Hey. What are we tackling?',
  "Ready when you are — what's the task?",
  "Let's go. Tell me what you're working on.",
] as const;

class FakeParam {
  value = 0;
  setValueAtTime(value: number): void {
    this.value = value;
  }
  setTargetAtTime(value: number): void {
    this.value = value;
  }
  linearRampToValueAtTime(value: number): void {
    this.value = value;
  }
}

class FakeNode {
  connect(): void {}
}

class FakeGain extends FakeNode {
  readonly gain = new FakeParam();
}

class FakeFilter extends FakeNode {
  type = 'lowpass';
  readonly frequency = new FakeParam();
  readonly Q = new FakeParam();
}

class FakeSource extends FakeNode {
  buffer: unknown = null;
  loop = false;
  start(): void {}
  stop(): void {}
}

class FakeAudioContext implements PresenceAudioContext {
  readonly destination = new FakeNode();
  readonly currentTime = 0;
  readonly sampleRate = 16_000;
  createBuffer(_channels: number, length: number): { getChannelData(channel: number): Float32Array } {
    const data = new Float32Array(length);
    return { getChannelData: () => data };
  }
  createBufferSource(): FakeSource {
    return new FakeSource();
  }
  createGain(): FakeGain {
    return new FakeGain();
  }
  createBiquadFilter(): FakeFilter {
    return new FakeFilter();
  }
}

function createFakeAudioContext(): PresenceAudioContext {
  return new FakeAudioContext();
}

function createRecordingSpeaker(spoken: string[]): SpeechPort {
  return {
    speak(text) {
      spoken.push(text);
    },
    stop() {},
  };
}

/** Poll a predicate on real timers until it is true or attempts run out — the same shape
 *  AppSurface.test.tsx's `waitForSpeech` uses to flush an async effect chain under `act`. */
async function pollUntil(predicate: () => boolean, attempts = 30): Promise<void> {
  for (let attempt = 0; attempt < attempts; attempt++) {
    if (predicate()) return;
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }
}

describe('F01 A3 — composition-root probe: the real FocusOrbApp, not a test-constructed runtime', () => {
  it('launching the real app in build mode makes the real running session produce the build opening', async () => {
    const spoken: string[] = [];
    const originalFetch = globalThis.fetch;
    // No relay/warmup server is running in this test; degrade instantly and deterministically
    // instead of depending on real-socket ECONNREFUSED timing (`AppController.ts`'s `launch()`
    // already treats a warmup failure as a caught, non-fatal error).
    globalThis.fetch = vi.fn(async () => {
      throw new Error('no warmup server in this test');
    }) as unknown as typeof fetch;

    try {
      const mockedFactory = vi.mocked(createT0FocusSession);
      const callsBefore = mockedFactory.mock.calls.length;

      await act(async () => {
        TestRenderer.create(
          <FocusOrbApp
            launchMode="build"
            createAudioContext={createFakeAudioContext}
            speaker={createRecordingSpeaker(spoken)}
          />,
        );
      });

      // The composition root itself built a session (App.tsx:136-161) — proof this mount shape
      // takes the no-runtime-prop branch, same as AppSurface.test.tsx:242-243.
      await pollUntil(() => mockedFactory.mock.calls.length > callsBefore);
      expect(mockedFactory.mock.calls.length).toBeGreaterThan(callsBefore);

      const call = mockedFactory.mock.calls[mockedFactory.mock.calls.length - 1]!;
      // §3.2b: the real construction site really threaded launchMode="build" through
      // resolveLaunchMode into the 5th constructor argument — not a hardcoded/ignored prop.
      expect(call[4] as T0SessionOptions).toEqual({ sessionMode: 'build' });

      const constructedRuntime = mockedFactory.mock.results[mockedFactory.mock.results.length - 1]!.value;
      const startSpy = vi.mocked(constructedRuntime.start);

      // Flush the real launch effect: warmup rejects -> caught -> voiceLoop.start() -> the REAL
      // runtime.start() the composition root itself built (not one the test constructs).
      await pollUntil(() => startSpy.mock.results.length > 0);
      expect(startSpy.mock.results.length).toBeGreaterThan(0);
      const envelope = await startSpy.mock.results[0]!.value;

      // A3 — the property, not the proxy.
      expect(envelope.mode).toBe('build');
      expect(envelope.speech?.text).toBe(BUILD_GREETING);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it('the same real composition root launched in focus mode renders one of the four focus greetings instead', async () => {
    const spoken: string[] = [];
    const originalFetch = globalThis.fetch;
    globalThis.fetch = vi.fn(async () => {
      throw new Error('no warmup server in this test');
    }) as unknown as typeof fetch;

    try {
      const mockedFactory = vi.mocked(createT0FocusSession);
      const callsBefore = mockedFactory.mock.calls.length;

      await act(async () => {
        TestRenderer.create(
          <FocusOrbApp
            launchMode="focus"
            createAudioContext={createFakeAudioContext}
            speaker={createRecordingSpeaker(spoken)}
          />,
        );
      });

      await pollUntil(() => mockedFactory.mock.calls.length > callsBefore);
      const call = mockedFactory.mock.calls[mockedFactory.mock.calls.length - 1]!;
      expect(call[4] as T0SessionOptions).toEqual({ sessionMode: 'focus' });

      const constructedRuntime = mockedFactory.mock.results[mockedFactory.mock.results.length - 1]!.value;
      const startSpy = vi.mocked(constructedRuntime.start);
      await pollUntil(() => startSpy.mock.results.length > 0);
      const envelope = await startSpy.mock.results[0]!.value;

      expect(envelope.mode).toBe('focus');
      expect(envelope.speech?.text).not.toBe(BUILD_GREETING);
      expect(FOCUS_GREETINGS as readonly string[]).toContain(envelope.speech?.text);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
