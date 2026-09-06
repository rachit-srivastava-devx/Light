import { describe, expect, it } from 'vitest';

import { RESPONSE_ENVELOPE_VERSION, type ResponseEnvelope } from './lld/ResponseEnvelope';
import { screenModelFromEnvelope } from './AppModel';

describe('FocusOrbApp screen model', () => {
  it('renders state, speech, and current step from the envelope', () => {
    const envelope: ResponseEnvelope = {
      v: RESPONSE_ENVELOPE_VERSION,
      session_id: 's1',
      turn_id: 't1',
      seq: 1,
      speech: {
        text: 'Open the tax portal.',
        audio: { mode: 'cached', phrase_id: 'step.v1' },
        voice: { provider: 'fish', voice_id: 'orb.warm.v1' },
      },
      orb: {
        emotion: 'calm',
        intensity: 0.5,
        animation: 'breathe',
        bed: { color: 'brown', gain_db: -18 },
      },
      widgets: [{ type: 'step_card', index: 1, total: 3, text: 'Open the tax portal' }],
      session: { state: 'STEP_PRESENT', step_index: 1, steps_total: 3 },
      meta: {
        model_version: 'm',
        prompt_version: 'p',
        source: 'cache_hit',
        latency_ms: 0,
        cost_paise: 0,
        trace_id: 'trace',
      },
    };

    expect(screenModelFromEnvelope(envelope)).toMatchObject({
      stateLabel: 'STEP PRESENT',
      stepLabel: '1/3 Open the tax portal',
      speechText: 'Open the tax portal.',
      voiceState: 'speaking',
      voiceVolume: 0.5,
    });
  });

  it('maps the native screen model to the Orb UI cloud state pattern', () => {
    const baseEnvelope: ResponseEnvelope = {
      v: RESPONSE_ENVELOPE_VERSION,
      session_id: 's1',
      turn_id: 't1',
      seq: 1,
      speech: {
        text: 'Tell me the task.',
        audio: { mode: 'cached', phrase_id: 'intake.v1' },
        voice: { provider: 'fish', voice_id: 'orb.warm.v1' },
      },
      orb: {
        emotion: 'calm',
        intensity: 0.8,
        animation: 'breathe',
        bed: { color: 'brown', gain_db: -18 },
      },
      session: { state: 'INTAKE', step_index: 0, steps_total: 0 },
      meta: {
        model_version: 'm',
        prompt_version: 'p',
        source: 'cache_hit',
        latency_ms: 0,
        cost_paise: 0,
        trace_id: 'trace',
      },
    };

    const listening = screenModelFromEnvelope(baseEnvelope);
    const speaking = screenModelFromEnvelope({
      ...baseEnvelope,
      session: { ...baseEnvelope.session, state: 'STEP_PRESENT' },
    });
    const idle = screenModelFromEnvelope({
      ...baseEnvelope,
      session: { ...baseEnvelope.session, state: 'INTERRUPTED' },
    });

    expect(listening.voiceState).toBe('listening');
    expect(speaking.voiceState).toBe('speaking');
    expect(idle.voiceState).toBe('idle');
    expect(listening.orbStyle.scale).toBeLessThan(1);
    expect(speaking.orbStyle.scale).toBeGreaterThan(1);
    expect(idle.orbStyle.opacity).toBeLessThan(listening.orbStyle.opacity);
  });
});
