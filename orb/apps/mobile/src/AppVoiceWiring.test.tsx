/**
 * Drives the REAL `App.tsx` — mounted, with its relay effect and mic callback actually running —
 * to prove the property the unit tests can only approximate: that a pause the user really took
 * ends the turn early in the app, and that the cases which must NOT end it still do not.
 *
 * This is the layer the defect lived at. `SemanticEndpointer.test.ts` was 19/19 green and
 * `VoiceLoopController.test.ts` was green the whole time the early-completion path was unreachable
 * in the running app, because the one input that made it reachable — `pause_ms` — was hardcoded to
 * 0 at this call site and no test ever went through it.
 *
 * The mock preamble is duplicated from `AppSurface.test.tsx` rather than shared: `vi.mock` factories
 * are hoisted per-file, and this file additionally has to mock the native mic port and install a
 * WebSocket, both of which that file deliberately does without.
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
  // Group/rect/rrect: added for AuroraMist's Skia-level circular clip (see AuroraMist.tsx and its
  // report — the fix for the breath-scale-vs-static-mask shape defect measured by
  // scripts/orb-shape-gate.mjs). Same inert-stand-in philosophy as the rest of this mock: this
  // suite never asserts on clip geometry, only that mounting doesn't throw.
  Group: 'Group',
  rect: (x: number, y: number, width: number, height: number) => ({ x, y, width, height }),
  rrect: (r: unknown, rx: number, ry: number) => ({ rect: r, rx, ry }),
  vec: (x: number, y: number) => ({ x, y }),
}));

/**
 * The native mic module cannot exist here, so `startCapture` becomes a handle on the frame
 * callback App.tsx registers: the test plays the microphone.
 */
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
import { createStaticAtomizerPort } from './runtime/AtomizerPort';
import { createT0FocusSession } from './runtime/T0FocusSession';
import { ENDPOINT_COMPLETE_PAUSE_MS, VAD_ONSET_MIN_MS, VAD_POST_ENDPOINT_CLOSE_MS } from './voice/contracts';

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

/** Minimal open WebSocket the app's relay effect can construct and the test can push frames into. */
class FakeWebSocket {
  static latest: FakeWebSocket | null = null;
  readonly readyState = 1;
  binaryType: 'blob' | 'arraybuffer' = 'blob';
  readonly sent: (string | ArrayBuffer)[] = [];
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onopen: (() => void) | null = null;
  onerror: ((event?: unknown) => void) | null = null;
  onclose: ((event?: unknown) => void) | null = null;
  constructor() { FakeWebSocket.latest = this; }
  send(data: string | ArrayBuffer): void { this.sent.push(data); }
  close(): void {}
  controlFrames(): string[] {
    return this.sent
      .filter((frame): frame is string => typeof frame === 'string')
      .map((frame) => (JSON.parse(frame) as { type: string }).type);
  }
  deliverTranscript(text: string, isFinal: boolean): void {
    this.onmessage?.({
      data: JSON.stringify({ type: 'transcript', tenant_id: 't0', session_id: 's', text, is_final: isFinal }),
    });
  }
  deliverSpeechStarting(): void {
    this.onmessage?.({
      data: JSON.stringify({ type: 'speech_starting', tenant_id: 't0', session_id: 's' }),
    });
  }
}

let clockMs = 1_700_000_000_000;
const advance = (ms: number): void => { clockMs += ms; };
const flush = async (): Promise<void> => {
  for (let tick = 0; tick < 6; tick += 1) await Promise.resolve();
};

function mount() {
  return TestRenderer.create(
    <FocusOrbApp
      autoStart={false}
      createAudioContext={() => new FakeAudioContext()}
      speaker={{ speak() {}, stop() {} }}
      runtime={createT0FocusSession(
        { tenant_id: 't0', user_id: 'local-user', session_id: 's', task: 'Open the tax portal' },
        createStaticAtomizerPort({ step_text: 'Open the tax portal', est_min: 1, done_signal: 'it is open' }),
      )}
    />,
  );
}

/** Two voice frames `VAD_ONSET_MIN_MS` apart is what VADGate needs to open a turn. */
async function speak(): Promise<void> {
  await act(async () => {
    mic.onFrame?.({ at_ms: 0, speech_probability: 0.95 }, new ArrayBuffer(4));
    advance(VAD_ONSET_MIN_MS);
    mic.onFrame?.({ at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.95 }, new ArrayBuffer(4));
    await flush();
  });
}

/**
 * One utterance played into the mounted app at a realistic 20ms frame cadence, silence included,
 * returning the simulated millisecond at which `end_of_turn` actually reached the socket.
 *
 * `deliverPartialsFrom` is how far behind the audio the STT partials trail; `null` plays the
 * utterance with no transcripts at all, which is the VAD-only baseline — exactly what the running
 * app did before this wiring, since `pause_ms` was pinned at 0 and nothing else could endpoint.
 */
async function playUtterance(options: {
  readonly socket: FakeWebSocket;
  readonly speechMs: number;
  readonly trailingSilenceMs: number;
  readonly transcript: string | null;
  readonly sttLagMs: number;
}): Promise<number | null> {
  const FRAME_MS = 20;
  const startedAtMs = clockMs;
  let endOfTurnAtMs: number | null = null;
  const noteEndOfTurn = (): void => {
    if (endOfTurnAtMs === null && options.socket.controlFrames().includes('end_of_turn')) {
      endOfTurnAtMs = clockMs - startedAtMs;
    }
  };

  for (let atMs = 0; atMs <= options.speechMs + options.trailingSilenceMs; atMs += FRAME_MS) {
    const speaking = atMs <= options.speechMs;
    await act(async () => {
      mic.onFrame?.({ at_ms: atMs, speech_probability: speaking ? 0.95 : 0.05 }, new ArrayBuffer(640));
      // STT streams partials while the user talks and for a little while after they stop; each
      // one carries the transcript so far, and each is a chance for the endpointer to decide.
      if (options.transcript !== null && atMs >= options.sttLagMs) {
        options.socket.deliverTranscript(options.transcript, false);
      }
      await flush();
    });
    noteEndOfTurn();
    if (endOfTurnAtMs !== null) return endOfTurnAtMs;
    advance(FRAME_MS);
  }
  noteEndOfTurn();
  return endOfTurnAtMs;
}

describe('App.tsx voice wiring: measured pause reaches the semantic endpointer', () => {
  let rendered: TestRenderer.ReactTestRenderer | null = null;

  beforeEach(async () => {
    clockMs = 1_700_000_000_000;
    vi.spyOn(Date, 'now').mockImplementation(() => clockMs);
    (globalThis as { WebSocket?: unknown }).WebSocket = FakeWebSocket;
    FakeWebSocket.latest = null;
    mic.onFrame = null;
    await act(async () => {
      rendered = mount();
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

  it('ends the turn early on a complete thought, without waiting out the VAD hangover', async () => {
    const socket = FakeWebSocket.latest!;
    await speak();
    expect(socket.controlFrames()).toContain('start_listening');
    expect(socket.controlFrames()).not.toContain('end_of_turn');

    // A real pause, far short of VAD's 800ms close: this is the whole latency win.
    advance(ENDPOINT_COMPLETE_PAUSE_MS + 50);
    expect(ENDPOINT_COMPLETE_PAUSE_MS + 50).toBeLessThan(VAD_POST_ENDPOINT_CLOSE_MS);
    await act(async () => {
      socket.deliverTranscript('just open the tax portal', false);
      await flush();
    });

    expect(socket.controlFrames().filter((type) => type === 'end_of_turn')).toHaveLength(1);
  });

  it('does not cut off a Hinglish speaker mid-thought on the same pause', async () => {
    const socket = FakeWebSocket.latest!;
    await speak();

    advance(ENDPOINT_COMPLETE_PAUSE_MS + 50);
    await act(async () => {
      socket.deliverTranscript('mujhe taxes file karni hain aur', false);
      await flush();
    });

    // looksIncomplete's Hindi/Hinglish half is only reachable once pause_ms is real; before this
    // wiring decideEndpoint returned `too_soon` here and never consulted it at all.
    expect(socket.controlFrames()).not.toContain('end_of_turn');
  });

  it('does not endpoint when no voice frame has been observed in the turn', async () => {
    const socket = FakeWebSocket.latest!;
    // No speak(): the mic never delivered a voice frame, so there is no measurement to act on.
    advance(10_000);
    await act(async () => {
      socket.deliverTranscript('just open the tax portal', false);
      await flush();
    });

    expect(socket.controlFrames()).not.toContain('end_of_turn');
  });

  it('does not carry the previous turn\'s silence into the next one', async () => {
    const socket = FakeWebSocket.latest!;
    await speak();
    advance(ENDPOINT_COMPLETE_PAUSE_MS + 50);
    await act(async () => {
      socket.deliverTranscript('just open the tax portal', false);
      await flush();
    });
    expect(socket.controlFrames().filter((type) => type === 'end_of_turn')).toHaveLength(1);

    // Assistant think/speak time, during which App.tsx's mic callback deliberately drops frames.
    // A stale "last voice" timestamp here would make the next turn's very first partial look like
    // it followed 6 seconds of silence and endpoint it instantly.
    advance(6_000);
    await act(async () => {
      socket.deliverTranscript('and then', false);
      await flush();
    });

    expect(socket.controlFrames().filter((type) => type === 'end_of_turn')).toHaveLength(1);
  });

  it('sends provider cancellation when the mic detects sustained speech over an in-flight reply', async () => {
    const socket = FakeWebSocket.latest!;
    await act(async () => {
      // The provider has accepted a speak request but has not produced/completed its audio yet.
      socket.deliverSpeechStarting();
      mic.onFrame?.({ at_ms: 0, speech_probability: 0.95 }, new ArrayBuffer(640));
      mic.onFrame?.({ at_ms: 200, speech_probability: 0.95 }, new ArrayBuffer(640));
      await flush();
    });

    // No audio chunk was delivered in this test, so local playback stopping cannot make it pass:
    // the observable is the cancellation control frame that retires relay/provider work.
    expect(socket.controlFrames().filter((type) => type === 'barge_in')).toHaveLength(1);
  });

  it('reaches end_of_turn hundreds of ms earlier than the VAD-only path it replaces', async () => {
    const SPEECH_MS = 1_000;
    const TRAILING_SILENCE_MS = 2_000;
    // Partials trail the audio; the endpointer only ever acts on one that has actually arrived.
    const STT_LAG_MS = 200;

    const semanticSocket = FakeWebSocket.latest!;
    const semanticMs = await playUtterance({
      socket: semanticSocket,
      speechMs: SPEECH_MS,
      trailingSilenceMs: TRAILING_SILENCE_MS,
      transcript: 'just open the tax portal',
      sttLagMs: STT_LAG_MS,
    });

    // Same utterance, same cadence, no transcripts: VADGate's 800ms hangover is the only thing
    // that can end the turn — the app's actual behaviour before this change.
    act(() => rendered?.unmount());
    mic.onFrame = null;
    await act(async () => {
      rendered = mount();
      await flush();
    });
    const vadOnlyMs = await playUtterance({
      socket: FakeWebSocket.latest!,
      speechMs: SPEECH_MS,
      trailingSilenceMs: TRAILING_SILENCE_MS,
      transcript: null,
      sttLagMs: STT_LAG_MS,
    });

    expect(semanticMs).not.toBeNull();
    expect(vadOnlyMs).not.toBeNull();
    const savedMs = vadOnlyMs! - semanticMs!;
    console.info(
      `focus-orb:endpoint-latency semantic_ms=${semanticMs} vad_only_ms=${vadOnlyMs} saved_ms=${savedMs}`,
    );
    // The blueprint (03-VOICE-LATENCY-PIPELINE.md §5) claims 150-500ms for this endpointer. Guard
    // the low end of its own claim rather than the number this run happens to produce, so a
    // threshold change that quietly gives the win back fails here.
    expect(savedMs).toBeGreaterThanOrEqual(150);
    expect(semanticMs!).toBeLessThan(SPEECH_MS + VAD_POST_ENDPOINT_CLOSE_MS);
  });
});
