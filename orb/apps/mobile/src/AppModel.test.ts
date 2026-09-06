import { describe, expect, it } from 'vitest';

import {
  ORB_VISUAL_PRESETS,
  orbVisualStateFromEnvelope,
  screenModelFromEnvelope,
  type OrbLifecycleSignals,
} from './AppModel';
import { RESPONSE_ENVELOPE_VERSION, type ResponseEnvelope } from './lld/ResponseEnvelope';

function envelope(state: ResponseEnvelope['session']['state']): ResponseEnvelope {
  return {
    v: RESPONSE_ENVELOPE_VERSION,
    session_id: 's1',
    turn_id: 't1',
    seq: 1,
    speech: {
      text: 'status',
      audio: { mode: 'cached', phrase_id: 'status.v1' },
      voice: { provider: 'fish', voice_id: 'orb.warm.v1' },
    },
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

const readyMic: OrbLifecycleSignals = {
  micStatus: 'active',
  relayStatus: 'ready',
  listeningConfirmed: true,
};

describe('Focus Orb visual state model', () => {
  it.each([
    ['IDLE_PRESENT', {}, 'booting'],
    ['INTAKE', readyMic, 'listening'],
    ['STEP_PRESENT', readyMic, 'thinking'],
    ['WORKING', readyMic, 'working'],
    ['STEP_DONE', readyMic, 'success'],
    ['INTERRUPTED', readyMic, 'paused'],
  ] as const)('maps %s to %s', (sessionState, lifecycle, expected) => {
    expect(orbVisualStateFromEnvelope(envelope(sessionState), lifecycle)).toBe(expected);
  });

  it('does not show "listening" on local mic-open alone, only on the relay delivery receipt', () => {
    // The defect: `micStatus: 'active'` reflects the on-device mic engine, not anything the relay
    // has acknowledged. Before the fix this alone flipped the orb to "listening"; now it must
    // also wait for `listeningConfirmed` (App.tsx's `onListeningConfirmed` from RelayClient,
    // which only fires on a `listening_confirmed` frame from relay-rs).
    expect(
      orbVisualStateFromEnvelope(envelope('INTAKE'), {
        micStatus: 'active',
        relayStatus: 'ready',
        listeningConfirmed: false,
      }),
    ).toBe('booting');
    expect(
      orbVisualStateFromEnvelope(envelope('INTAKE'), {
        micStatus: 'active',
        relayStatus: 'ready',
        listeningConfirmed: true,
      }),
    ).toBe('listening');
  });

  it('defaults to not-listening when listeningConfirmed is omitted entirely (fail-safe default)', () => {
    // Mutation-testing gate (G4) found this: DEFAULT_LIFECYCLE.listeningConfirmed must default to
    // false, but every existing test always sets the field explicitly, so a regression that flips
    // the default to true would pass every prior test. Pin the default itself, not just the two
    // explicit values.
    expect(
      orbVisualStateFromEnvelope(envelope('INTAKE'), {
        micStatus: 'active',
        relayStatus: 'ready',
      }),
    ).toBe('booting');
  });

  it('uses the warm degraded preset for relay and mic failures', () => {
    expect(orbVisualStateFromEnvelope(envelope('WORKING'), { ...readyMic, relayStatus: 'degraded' })).toBe('error');
    expect(orbVisualStateFromEnvelope(envelope('INTAKE'), { micStatus: 'denied', relayStatus: 'ready' })).toBe('error');
    expect(ORB_VISUAL_PRESETS.error.colors[0]).toBe('#c87552');
    expect(ORB_VISUAL_PRESETS.error.colors[0]).not.toMatch(/^#f00/i);
  });

  it('shows live speech even when the session envelope remains intake', () => {
    const speaking = screenModelFromEnvelope(envelope('INTAKE'), {
      ...readyMic,
      speechStatus: 'speaking',
    });
    expect(speaking.voiceState).toBe('speaking');
    expect(speaking.orbVisual.state).toBe('thinking');
  });

  it('keeps the visual presets distinct in rhythm and emphasis', () => {
    expect(Object.keys(ORB_VISUAL_PRESETS)).toHaveLength(7);
    expect(new Set(Object.values(ORB_VISUAL_PRESETS).map((preset) => preset.breathPeriodMs)).size).toBeGreaterThan(4);
    expect(ORB_VISUAL_PRESETS.thinking.speed).toBeGreaterThan(ORB_VISUAL_PRESETS.working.speed);
    expect(ORB_VISUAL_PRESETS.working.breathPeriodMs).toBeGreaterThan(ORB_VISUAL_PRESETS.thinking.breathPeriodMs);
    expect(ORB_VISUAL_PRESETS.success.glow).toBeGreaterThan(ORB_VISUAL_PRESETS.paused.glow);
  });

  it('makes the key envelope/lifecycle transitions readable without changing the renderer', () => {
    const transitions = [
      [envelope('IDLE_PRESENT'), { micStatus: 'pending', relayStatus: 'connecting' }],
      [envelope('INTAKE'), readyMic],
      [envelope('STEP_PRESENT'), readyMic],
      [envelope('WORKING'), readyMic],
      [envelope('STEP_DONE'), readyMic],
      [envelope('INTERRUPTED'), readyMic],
      [envelope('WORKING'), { ...readyMic, relayStatus: 'degraded' }],
    ] as const;

    expect(transitions.map(([current, lifecycle]) => screenModelFromEnvelope(current, lifecycle).orbVisual.state)).toEqual([
      'booting',
      'listening',
      'thinking',
      'working',
      'success',
      'paused',
      'error',
    ]);
  });
});
