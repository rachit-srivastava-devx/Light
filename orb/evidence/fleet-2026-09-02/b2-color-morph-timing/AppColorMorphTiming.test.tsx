/**
 * B2 — the presence bed's brown<->pink status channel (`presence/ColorMorph.ts`) must morph to
 * "thinking" only during the crew-forming/computation window (blueprint 02 §7: "Thinking (crew
 * forming steps)"), never merely because the session envelope is presenting an already-computed
 * step. `ColorMorph.ts`'s own header is explicit: "Step ready / win" is a brief gain-domain swell,
 * NOT a sustained color change — "modelling it as a brighter cutoff would leave the bed permanently
 * brighter after a step is presented, contradicting §7".
 *
 * Defect: `App.tsx`'s own `useEffect` (keyed on `currentEnvelope.session.state`) called
 * `presenceBed.current?.morphColor(state === 'STEP_PRESENT' ? 'thinking' : 'idle')` — driving the
 * bed's color off the SPEAKING state (STEP_PRESENT, per AppModel.ts's own `voiceState()` mapping:
 * "STEP_PRESENT" -> 'speaking'), not the computing state. This duplicated and fought
 * `runtime/VoiceLoopController.ts`'s own, correct `schedulePresenceMorph`, which already drives
 * 'thinking' from the real computation window (turn-end-signaled -> crew computing) and back to
 * 'idle' once new user speech starts. This test isolates the App.tsx-owned driver in isolation, via
 * the `envelope` test-injection prop, so it cannot be confused with VoiceLoopController's own
 * (correct) calls, which never fire in this test (no mic/relay activity is played).
 *
 * The mock preamble (react-native/skia/native-mic-port) is duplicated from `AppVoiceWiring.test.tsx`
 * per that file's own note: `vi.mock` factories are hoisted per file and cannot be shared.
 */
import React from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import { afterEach, describe, expect, it } from 'vitest';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
(globalThis as typeof globalThis & { requestAnimationFrame?: (cb: (time: number) => number) => number }).requestAnimationFrame =
  (cb) => setTimeout(() => cb(0), 16) as unknown as number;
(globalThis as typeof globalThis & { cancelAnimationFrame?: (handle: number) => void }).cancelAnimationFrame =
  (handle) => clearTimeout(handle);

import { vi } from 'vitest';

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
import { BED_STATUS_CUTOFF_HZ } from './presence/ColorMorph';
import { createStaticAtomizerPort } from './runtime/AtomizerPort';
import { createT0FocusSession } from './runtime/T0FocusSession';
import { RESPONSE_ENVELOPE_VERSION, type ResponseEnvelope } from './lld/ResponseEnvelope';

function envelope(state: ResponseEnvelope['session']['state']): ResponseEnvelope {
  return {
    v: RESPONSE_ENVELOPE_VERSION,
    session_id: 's1',
    turn_id: 't1',
    seq: 1,
    orb: {
      emotion: 'calm',
      intensity: 0.6,
      animation: 'breathe',
      bed: { color: 'brown', gain_db: -18 },
    },
    session: { state, step_index: 1, steps_total: 3 },
    meta: {
      model_version: 'm',
      prompt_version: 'p',
      source: 'cache_hit',
      latency_ms: 0,
      cost_paise: 0,
      trace_id: 'trace',
    },
  };
}

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
  readonly filters: FakeFilter[] = [];
  createBuffer(_channels: number, length: number) {
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
    const filter = new FakeFilter();
    this.filters.push(filter);
    return filter;
  }
}

const flush = async (): Promise<void> => {
  for (let tick = 0; tick < 6; tick += 1) await Promise.resolve();
};

describe('presence bed color-morph timing (App.tsx-owned driver)', () => {
  let rendered: TestRenderer.ReactTestRenderer | null = null;

  afterEach(() => {
    act(() => rendered?.unmount());
    rendered = null;
  });

  it('does not morph the bed to "thinking" just because the envelope is presenting a step (STEP_PRESENT)', async () => {
    const ctx = new FakeAudioContext();
    const runtime = createT0FocusSession(
      { tenant_id: 't0', user_id: 'local-user', session_id: 's', task: 'Open the tax portal' },
      createStaticAtomizerPort({ step_text: 'Open the tax portal', est_min: 1, done_signal: 'it is open' }),
    );

    await act(async () => {
      rendered = TestRenderer.create(
        <FocusOrbApp
          autoStart={false}
          createAudioContext={() => ctx}
          speaker={{ speak() {}, stop() {} }}
          runtime={runtime}
          envelope={envelope('INTAKE')}
        />,
      );
      await flush();
    });

    const filter = ctx.filters[0];
    if (!filter) throw new Error('presence bed must have booted a real biquad filter');

    // Sanity: nothing has driven the bed to the 'thinking' cutoff yet (no voice-loop turn ran —
    // this test never touches the mic or the relay, only the `envelope` prop App.tsx exposes for
    // exactly this kind of isolated assertion).
    expect(filter.frequency.value).not.toBe(BED_STATUS_CUTOFF_HZ.thinking);

    // The defect: presenting an already-computed step (STEP_PRESENT — AppModel.ts's own
    // `voiceState()` maps this to 'speaking', not 'thinking') must not, by itself, morph the bed to
    // pink. Only the real computation window (owned by VoiceLoopController's
    // `schedulePresenceMorph`, which this test never exercises) may do that.
    await act(async () => {
      rendered!.update(
        <FocusOrbApp
          autoStart={false}
          createAudioContext={() => ctx}
          speaker={{ speak() {}, stop() {} }}
          runtime={runtime}
          envelope={envelope('STEP_PRESENT')}
        />,
      );
      await flush();
    });

    expect(filter.frequency.value).not.toBe(BED_STATUS_CUTOFF_HZ.thinking);
  });
});
