/**
 * Drives the REAL `App.tsx` to prove the property a unit test on RelayClient alone cannot: that
 * receiving relay-rs's `audio_bed_fallback` frame (session.rs's `Phase::Degraded` +
 * `Action::SendFallback`, protocol.rs's `ServerFrame::AudioBedFallback`) does NOT cancel presence
 * and DOES still speak the pre-existing `provider_failure` apology.
 *
 * Before this frame type reached the client, a provider failure closed the socket and arrived as
 * a `closing` frame with `reason: "provider_failure"`, which App.tsx handled by cancelling
 * presence (`cancelBy('error')`) and stopping capture — correct for a session that really was
 * ending. relay-rs no longer ends the session on provider failure (that is the whole point of the
 * fix this closes the client-side gap for); the client's frame-dispatch switch previously had no
 * case for `audio_bed_fallback`, so it fell to `default` -> `onProtocolError`, which ALSO called
 * `cancelBy('error')` and set the app into an error state — cancelling presence for an event that
 * is specifically supposed to keep it alive, and silently dropping the apology that used to fire
 * on `closing`'s `provider_failure` path. This test is RED against that prior wiring and GREEN once
 * `audio_bed_fallback` gets its own dispatch case.
 *
 * The mock preamble mirrors `AppVoiceWiring.test.tsx` (same reason: this file needs both a native
 * mic port stub and an installed WebSocket).
 */
import React from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
(globalThis as typeof globalThis & { requestAnimationFrame?: (cb: (time: number) => void) => number }).requestAnimationFrame =
  (cb) => setTimeout(() => cb(0), 16) as unknown as number;
(globalThis as typeof globalThis & { cancelAnimationFrame?: (handle: number) => void }).cancelAnimationFrame =
  (handle) => clearTimeout(handle);

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
  AppState: { addEventListener: () => ({ remove() {} }) },
  Easing: {
    inOut: (fn: (t: number) => number) => fn,
    sin: (t: number) => t,
  },
  Image: { resolveAssetSource: () => ({ uri: '' }) },
  PermissionsAndroid: {
    PERMISSIONS: { RECORD_AUDIO: 'android.permission.RECORD_AUDIO' },
    RESULTS: { GRANTED: 'granted' },
    request: async () => 'granted',
  },
  PixelRatio: { get: () => 2 },
  Platform: { OS: 'android' },
  Pressable: 'Pressable',
  StyleSheet: {
    create: <T extends object>(styles: T) => styles,
    flatten: (style: unknown) => (Array.isArray(style) ? Object.assign({}, ...style) : style),
  },
  Text: 'Text',
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

const mic = vi.hoisted(() => ({
  onFrame: null as null | ((frame: { at_ms: number; speech_probability: number }, audio: ArrayBuffer) => void),
}));
vi.mock('./voice/NativeMicPort', () => ({
  requestMicPermission: async () => true,
  startCapture: async (onFrame: NonNullable<typeof mic.onFrame>) => {
    mic.onFrame = onFrame;
  },
  stopCapture: async () => {
    mic.onFrame = null;
  },
}));

import FocusOrbApp from './App';
import type { PresenceAudioContext } from './presence/PresenceBoot';
import { PresenceEventQueue, type PresenceCancellation } from './presence/PresenceEventQueue';
import { createStaticAtomizerPort } from './runtime/AtomizerPort';
import { createT0FocusSession } from './runtime/T0FocusSession';

class FakeParam {
  value = 0;
  setValueAtTime(value: number): void { this.value = value; }
  setTargetAtTime(value: number): void { this.value = value; }
  linearRampToValueAtTime(value: number): void { this.value = value; }
}
class FakeNode { connect(): void {} }
class FakeGain extends FakeNode { readonly gain = new FakeParam(); }
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
  createBuffer(_channels: number, length: number) {
    const data = new Float32Array(length);
    return { getChannelData: () => data };
  }
  createBufferSource(): FakeSource { return new FakeSource(); }
  createGain(): FakeGain { return new FakeGain(); }
  createBiquadFilter(): FakeFilter { return new FakeFilter(); }
}

class FakeWebSocket {
  static latest: FakeWebSocket | null = null;
  readonly readyState = 1;
  binaryType: 'blob' | 'arraybuffer' = 'blob';
  readonly sent: (string | ArrayBuffer)[] = [];
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onopen: (() => void) | null = null;
  onerror: ((event?: unknown) => void) | null = null;
  onclose: ((event?: unknown) => void) | null = null;
  closeCalls = 0;
  constructor() { FakeWebSocket.latest = this; }
  send(data: string | ArrayBuffer): void { this.sent.push(data); }
  close(): void { this.closeCalls += 1; }
  deliver(frame: Record<string, unknown>): void {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }
}

const flush = async (): Promise<void> => {
  for (let tick = 0; tick < 6; tick += 1) await Promise.resolve();
};

function mount(speak: (text: string) => void) {
  return TestRenderer.create(
    <FocusOrbApp
      autoStart={false}
      createAudioContext={() => new FakeAudioContext()}
      speaker={{ speak: async (text: string) => speak(text), stop() {} }}
      runtime={createT0FocusSession(
        { tenant_id: 't0', user_id: 'local-user', session_id: 's', task: 'Open the tax portal' },
        createStaticAtomizerPort({ step_text: 'Open the tax portal', est_min: 1, done_signal: 'it is open' }),
      )}
    />,
  );
}

describe('App.tsx voice wiring: audio_bed_fallback frame', () => {
  let rendered: TestRenderer.ReactTestRenderer | null = null;
  let cancelBySpy: import('vitest').MockInstance<(signal: PresenceCancellation) => number>;
  let spoken: string[];

  beforeEach(async () => {
    (globalThis as { WebSocket?: unknown }).WebSocket = FakeWebSocket;
    FakeWebSocket.latest = null;
    mic.onFrame = null;
    spoken = [];
    // Spy on the real queue class so the app's own instance is observed, rather than replacing
    // the presence system with a mock — the property under test is what the REAL queue receives.
    cancelBySpy = vi.spyOn(PresenceEventQueue.prototype, 'cancelBy');
    await act(async () => {
      rendered = mount((text) => spoken.push(text));
      await flush();
    });
    expect(mic.onFrame, 'App.tsx must have started mic capture').not.toBeNull();
  });

  afterEach(() => {
    act(() => rendered?.unmount());
    rendered = null;
    delete (globalThis as { WebSocket?: unknown }).WebSocket;
    vi.restoreAllMocks();
  });

  it('does not cancel presence on audio_bed_fallback', async () => {
    const socket = FakeWebSocket.latest!;
    await act(async () => {
      socket.deliver({ type: 'audio_bed_fallback', tenant_id: 't0', session_id: 's' });
      await flush();
    });

    expect(cancelBySpy).not.toHaveBeenCalledWith('error');
  });

  it('does not close the transport on audio_bed_fallback (the session stays alive)', async () => {
    const socket = FakeWebSocket.latest!;
    await act(async () => {
      socket.deliver({ type: 'audio_bed_fallback', tenant_id: 't0', session_id: 's' });
      await flush();
    });

    expect(socket.closeCalls).toBe(0);
  });

  it('still speaks the provider_failure apology on audio_bed_fallback', async () => {
    const socket = FakeWebSocket.latest!;
    await act(async () => {
      socket.deliver({ type: 'audio_bed_fallback', tenant_id: 't0', session_id: 's' });
      await flush();
    });

    expect(spoken.some((text) => text.includes('still here'))).toBe(true);
  });

  it('does NOT fall through to the generic protocol-error path (which used to cancel presence and drop the apology)', async () => {
    const socket = FakeWebSocket.latest!;
    const errorLog = vi.spyOn(console, 'error').mockImplementation(() => {});
    await act(async () => {
      socket.deliver({ type: 'audio_bed_fallback', tenant_id: 't0', session_id: 's' });
      await flush();
    });

    expect(errorLog.mock.calls.some(([msg]) => typeof msg === 'string' && msg.includes('relay-protocol-error'))).toBe(
      false,
    );
    errorLog.mockRestore();
  });
});
