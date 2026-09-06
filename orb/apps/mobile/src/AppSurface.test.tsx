import React from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import { describe, expect, it, vi } from 'vitest';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// This Node runtime ships a real global `WebSocket` (Node 22+), so App.tsx's relay effect would
// otherwise attempt a genuine network connection to `ws://10.0.2.2:8091` during this test. App.tsx
// already has a graceful no-op path for exactly this case (`typeof WebSocket === 'undefined'`) —
// stubbing it out here exercises that path instead of hitting the network from a unit test.
delete (globalThis as { WebSocket?: unknown }).WebSocket;

// AuroraMist (CloudOrb's Skia mist) drives its drift animation off requestAnimationFrame, which
// this Node test environment doesn't provide (it's a browser/RN host API). setTimeout stands in
// so the effect mounts without throwing; this suite never asserts on animation frames.
(globalThis as typeof globalThis & { requestAnimationFrame?: (cb: (time: number) => void) => number }).requestAnimationFrame =
  (cb) => setTimeout(() => cb(Date.now()), 16) as unknown as number;
(globalThis as typeof globalThis & { cancelAnimationFrame?: (handle: number) => void }).cancelAnimationFrame =
  (handle) => clearTimeout(handle);

vi.mock('react-native', () => ({
  // CloudOrb's breathing pulse now runs on RN's own Animated driver instead of a
  // requestAnimationFrame-driven state variable (see orb/CloudOrb.tsx and orb/motion.ts). This
  // suite never asserts on animation frames or interpolated values, so inert stand-ins are enough
  // to let the effect start/stop without throwing — same philosophy as the Skia mock below.
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
  // Denies by default — exercises App.tsx's "permission denied, mic stays off, TTS/orb still
  // work" path, matching what a fresh install with no prior grant would do.
  PermissionsAndroid: {
    PERMISSIONS: { RECORD_AUDIO: 'android.permission.RECORD_AUDIO' },
    RESULTS: { GRANTED: 'granted' },
    request: async () => 'denied',
  },
  PixelRatio: {
    get: () => 2,
  },
  // Needed at module-load time by @shopify/react-native-skia's Platform shim (CloudOrb -> AuroraMist).
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

// @shopify/react-native-skia is a native rendering library (real GPU/Skia bindings via
// TurboModuleRegistry) — meaningless in a bare Node/vitest environment, same category as
// react-native-audio-api elsewhere in this app. Stand in with inert leaf components: this suite
// only asserts on outer View/Text/Pressable structure and background color, never pixel output.
vi.mock('@shopify/react-native-skia', () => ({
  Canvas: 'Canvas',
  Circle: 'Circle',
  LinearGradient: 'LinearGradient',
  BlurMask: 'BlurMask',
  // Group/rect/rrect: added for AuroraMist's Skia-level circular clip (see AuroraMist.tsx and its
  // report — the fix for the breath-scale-vs-static-mask shape defect measured by
  // scripts/orb-shape-gate.mjs). Same inert-stand-in philosophy as the rest of this mock: this
  // suite never asserts on clip geometry, only that mounting doesn't throw.
  Group: 'Group',
  rect: (x: number, y: number, width: number, height: number) => ({ x, y, width, height }),
  rrect: (r: unknown, rx: number, ry: number) => ({ rect: r, rx, ry }),
  vec: (x: number, y: number) => ({ x, y }),
}));

// NativeMicPort is a TurboModule bridge to real iOS/Android mic hardware — same category as
// react-native-audio-api and skia above. Unmocked it rejects in Node, which left `micStatus` stuck
// on 'pending' (an unhandled rejection, not a visible failure) so the mic-unusable UI never
// rendered. Denying here is the state the typed-path tests are about, and it is also what a fresh
// install with no grant does.
vi.mock('./voice/NativeMicPort', () => ({
  requestMicPermission: vi.fn(async () => false),
  startCapture: vi.fn(async () => {}),
  stopCapture: vi.fn(async () => {}),
}));

import FocusOrbApp from './App';
import type { PresenceAudioContext } from './presence/PresenceBoot';
import { createStaticAtomizerPort } from './runtime/AtomizerPort';
import { createT0FocusSession } from './runtime/T0FocusSession';
import { requestMicPermission, startCapture, stopCapture } from './voice/NativeMicPort';
import type { SpeechPort } from './voice/SpeechPort';

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

function nodesOfType(root: TestRenderer.ReactTestInstance, type: string): TestRenderer.ReactTestInstance[] {
  return root.findAll((node) => node.type === type);
}

async function waitForSpeech(spoken: readonly string[]): Promise<void> {
  for (let attempt = 0; attempt < 10; attempt++) {
    if (spoken.length > 0) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

/**
 * A WebSocket that connects to nothing.
 *
 * App.tsx nests the microphone effect INSIDE the relay effect, which returns early when
 * `typeof WebSocket === 'undefined'` — so with the global deleted (see the top of this file) the mic
 * permission request never runs and `micStatus` is stuck on 'pending' forever. That is worth knowing
 * on its own: no transport means no microphone at all. The two typed-path tests below need the mic
 * to actually reach its DENIED state, so they install this stub for the duration.
 */
class SilentWebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  readyState = 0;
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((event: unknown) => void) | null = null;
  constructor(public readonly url: string) {}
  addEventListener(): void {}
  removeEventListener(): void {}
  send(): void {}
  close(): void {
    this.readyState = 3;
    this.onclose?.();
  }
}

function withSilentSocket<T>(run: () => Promise<T>): Promise<T> {
  const holder = globalThis as { WebSocket?: unknown };
  const had = 'WebSocket' in holder;
  const previous = holder.WebSocket;
  holder.WebSocket = SilentWebSocket as unknown as typeof WebSocket;
  return run().finally(() => {
    if (had) holder.WebSocket = previous;
    else delete holder.WebSocket;
  });
}

describe('FocusOrbApp surface', () => {
  it('renders only the cloud orb on a dark background', () => {
    let rendered: TestRenderer.ReactTestRenderer | null = null;
    act(() => {
      rendered = TestRenderer.create(
        <FocusOrbApp autoStart={false} createAudioContext={createFakeAudioContext} speaker={createRecordingSpeaker([])} />,
      );
    });

    const root = rendered!.root;
    const screen = root.findByProps({ testID: 'focus-orb-screen' });
    // Deliberate, narrow exception to "no visible text" (2026-08-06 /goal — a user could not tell
    // whether the mic was actually capturing): the only Text node allowed is the real-mic-status
    // label, gated strictly on hardware status, never a decorative or arbitrary string.
    const textNodes = nodesOfType(root, 'Text');
    expect(textNodes.every((node) => node.props.testID === 'focus-orb-mic-status')).toBe(true);
    expect(nodesOfType(root, 'Pressable')).toHaveLength(0);
    expect(root.findAllByProps({ testID: 'focus-orb-cloud' })).toHaveLength(1);
    expect(screen.props.style.backgroundColor).toBe('#05070d');
  });

  it('auto-starts and speaks the launch greeting instead of injecting a first task', async () => {
    const spoken: string[] = [];
    let rendered: TestRenderer.ReactTestRenderer | null = null;
    await act(async () => {
      rendered = TestRenderer.create(
        <FocusOrbApp
          createAudioContext={createFakeAudioContext}
          speaker={createRecordingSpeaker(spoken)}
          runtime={createT0FocusSession(
            {
              tenant_id: 't0',
              user_id: 'local-user',
              session_id: 'local-session',
              task: 'Open the first small action',
            },
            createStaticAtomizerPort({
              step_text: 'Open the first small action',
              est_min: 1,
              done_signal: 'the first small action is open',
            }),
          )}
        />,
      );
      await waitForSpeech(spoken);
    });

    expect(spoken).toEqual(['Good morning, Rachit']);
    // Same narrow exception as the surface test above — only the real-mic-status label may render.
    const textNodes = nodesOfType(rendered!.root, 'Text');
    expect(textNodes.every((node) => node.props.testID === 'focus-orb-mic-status')).toBe(true);
  });

  /** A transient close keeps capture alive for one retry; exhausting that retry releases it. */
  it('keeps mic capture during one reconnect, then stops it after the retry closes', async () => {
    const sockets: SilentWebSocket[] = [];
    class RecordingSocket extends SilentWebSocket {
      constructor(url: string) {
        super(url);
        sockets.push(this);
      }
    }
    vi.mocked(stopCapture).mockClear();
    const holder = globalThis as { WebSocket?: unknown };
    const had = 'WebSocket' in holder;
    const previous = holder.WebSocket;
    holder.WebSocket = RecordingSocket as unknown as typeof WebSocket;
    try {
      await act(async () => {
        TestRenderer.create(
          <FocusOrbApp
            createAudioContext={createFakeAudioContext}
            speaker={createRecordingSpeaker([])}
            runtime={createT0FocusSession(
              {
                tenant_id: 't0',
                user_id: 'local-user',
                session_id: 'local-session',
                task: 'Open the first small action',
              },
              createStaticAtomizerPort({
                step_text: 'Open the first small action',
                est_min: 1,
                done_signal: 'the first small action is open',
              }),
            )}
          />,
        );
        await Promise.resolve();
      });

      expect(sockets.length).toBeGreaterThan(0);
      // Reach 'open' first so RelaySocket.handleClose takes its clean-close branch (state
      // 'open' -> 'closed', only onTransportClosed fires) instead of its connecting-failure
      // branch (state 'connecting', which also calls onerror/onTransportError — that already
      // called stopCapture before this fix, which would make this test pass for the wrong
      // reason without ever exercising the onTransportClosed code path this fix changed).
      await act(async () => {
        sockets[0]!.onopen?.();
        await Promise.resolve();
      });
      await act(async () => {
        sockets[0]!.onclose?.();
        await Promise.resolve();
      });

      expect(vi.mocked(stopCapture)).not.toHaveBeenCalled();
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 275));
      });
      expect(sockets).toHaveLength(2);
      await act(async () => {
        sockets[1]!.onopen?.();
        sockets[1]!.onclose?.();
        await Promise.resolve();
      });

      expect(vi.mocked(stopCapture)).toHaveBeenCalled();
    } finally {
      if (had) holder.WebSocket = previous;
      else delete holder.WebSocket;
    }
  });

  /**
   * Regression test: startCapture registers its native-frame listener before awaiting
   * module.start(). If start() rejects after partially acquiring the native audio resource, the
   * old catch block only logged and set micStatus('error') — nothing released the resource.
   */
  it('stops mic capture when native start fails, releasing any partially-acquired resource', async () => {
    vi.mocked(requestMicPermission).mockResolvedValueOnce(true);
    vi.mocked(startCapture).mockRejectedValueOnce(new Error('native start failed'));
    vi.mocked(stopCapture).mockClear();

    await withSilentSocket(async () => {
      await act(async () => {
        TestRenderer.create(
          <FocusOrbApp
            createAudioContext={createFakeAudioContext}
            speaker={createRecordingSpeaker([])}
            runtime={createT0FocusSession(
              {
                tenant_id: 't0',
                user_id: 'local-user',
                session_id: 'local-session',
                task: 'Open the first small action',
              },
              createStaticAtomizerPort({
                step_text: 'Open the first small action',
                est_min: 1,
                done_signal: 'the first small action is open',
              }),
            )}
          />,
        );
        await new Promise((resolve) => setTimeout(resolve, 0));
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    });

    expect(vi.mocked(stopCapture)).toHaveBeenCalled();
  });

  /**
   * The typed fallback exists because a driven simulator run dead-ended: mic unusable, a static orb,
   * and ZERO requests reaching the relay across four launches. `PermissionsAndroid.request` in this
   * file's mock already denies, which is exactly that state.
   *
   * These assert the WIRE, not the widget. A test that finds a text box proves nothing about whether
   * a typed turn reaches the backend — and "computed everywhere, transmitted nowhere" is this repo's
   * most expensive defect class (`mode` broke three paths at once with every suite green). So the
   * assertion is on `runtime.acceptAudio`, the same seam a final STT transcript crosses.
   */
  it('offers a typed path when the mic is unusable, and a typed turn crosses the runtime seam', async () => {
    await withSilentSocket(async () => {
    const accepted: string[] = [];
    const spoken: string[] = [];
    const base = createT0FocusSession(
      {
        tenant_id: 't0',
        user_id: 'local-user',
        session_id: 'typed-session',
        task: 'Open the first small action',
      },
      createStaticAtomizerPort({
        step_text: 'Open the first small action',
        est_min: 1,
        done_signal: 'the first small action is open',
      }),
    );
    const runtime = {
      ...base,
      acceptAudio: async (bytes: Uint8Array) => {
        accepted.push(new TextDecoder().decode(bytes));
        return base.acceptAudio(bytes);
      },
    };

    let rendered: TestRenderer.ReactTestRenderer | null = null;
    await act(async () => {
      rendered = TestRenderer.create(
        <FocusOrbApp
          createAudioContext={createFakeAudioContext}
          speaker={createRecordingSpeaker(spoken)}
          runtime={runtime}
        />,
      );
      await waitForSpeech(spoken);
    });
    // Let the denied permission settle so micStatus reaches its terminal value.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const root = rendered!.root;
    const input = root.findByProps({ testID: 'focus-orb-typed-input' });
    expect(input.props.accessibilityLabel).toBe('Type a message to the orb');
    // Must not grab the keyboard on mount — the user has been told the mic failed, not asked to type.
    expect(input.props.autoFocus).toBe(false);

    await act(async () => {
      input.props.onChangeText('my kitchen is a disaster');
    });
    await act(async () => {
      await root.findByProps({ testID: 'focus-orb-typed-send' }).props.onPress();
    });

    expect(accepted).toContain('my kitchen is a disaster');
    // And the answer is READABLE — someone typing may well be somewhere they cannot listen.
    expect(root.findAllByProps({ testID: 'focus-orb-reply-text' }).length).toBeGreaterThan(0);
    });
  });

  it('never sends an empty or whitespace-only typed turn', async () => {
    await withSilentSocket(async () => {
    const accepted: string[] = [];
    const spoken: string[] = [];
    const base = createT0FocusSession(
      { tenant_id: 't0', user_id: 'local-user', session_id: 'typed-empty', task: 'Open it' },
      createStaticAtomizerPort({ step_text: 'Open it', est_min: 1, done_signal: 'open' }),
    );
    const runtime = {
      ...base,
      acceptAudio: async (bytes: Uint8Array) => {
        accepted.push(new TextDecoder().decode(bytes));
        return base.acceptAudio(bytes);
      },
    };
    let rendered: TestRenderer.ReactTestRenderer | null = null;
    await act(async () => {
      rendered = TestRenderer.create(
        <FocusOrbApp
          createAudioContext={createFakeAudioContext}
          speaker={createRecordingSpeaker(spoken)}
          runtime={runtime}
        />,
      );
      await waitForSpeech(spoken);
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    const root = rendered!.root;
    await act(async () => {
      root.findByProps({ testID: 'focus-orb-typed-input' }).props.onChangeText('   ');
    });
    const send = root.findByProps({ testID: 'focus-orb-typed-send' });
    // Disabled, and inert even if pressed anyway — unasked-for speech is what the 0ms-silence rule
    // forbids, and `completeTurn` would otherwise be handed an empty transcript.
    expect(send.props.accessibilityState.disabled).toBe(true);
    await act(async () => {
      await send.props.onPress();
    });
    expect(accepted).toHaveLength(0);
    });
  });
});
