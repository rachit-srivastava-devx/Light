import { describe, expect, it } from 'vitest';

import { isVoiceFrame, pauseMsSince } from './PauseClock';
import { reduceVadGate, INITIAL_VAD_GATE_STATE, type VadGateState } from './VADGate';
import {
  ENDPOINT_COMPLETE_PAUSE_MS,
  VAD_ONSET_MIN_MS,
  VAD_SILENCE_PROBABILITY,
  VAD_SPEECH_PROBABILITY,
} from './contracts';
import { decideEndpoint } from './SemanticEndpointer';

describe('isVoiceFrame', () => {
  it('counts a frame as voice at exactly the hysteresis floor, not the onset ceiling', () => {
    expect(isVoiceFrame(VAD_SILENCE_PROBABILITY)).toBe(true);
    expect(isVoiceFrame(VAD_SILENCE_PROBABILITY - 0.001)).toBe(false);
    // The onset threshold is higher; using it here would under-count voice and over-count silence.
    expect(VAD_SPEECH_PROBABILITY).toBeGreaterThan(VAD_SILENCE_PROBABILITY);
    expect(isVoiceFrame((VAD_SILENCE_PROBABILITY + VAD_SPEECH_PROBABILITY) / 2)).toBe(true);
  });

  it('rejects frames VADGate itself would refuse to trust', () => {
    for (const probability of [Number.NaN, Number.POSITIVE_INFINITY, -0.1, 1.1]) {
      expect(isVoiceFrame(probability)).toBe(false);
    }
    expect(isVoiceFrame(0)).toBe(false);
    expect(isVoiceFrame(1)).toBe(true);
  });

  /**
   * The property that actually matters: this module and VADGate must agree, frame by frame, on
   * which frames count as voice while a turn is open. A census over the whole probability range,
   * not a sample — a threshold that drifted by one step would still pass three hand-picked cases.
   */
  it('agrees with VADGate on every frame in an open turn (census, 0.00-1.00)', () => {
    let opened: VadGateState = INITIAL_VAD_GATE_STATE;
    opened = reduceVadGate(opened, { at_ms: 0, speech_probability: 0.9 }).state;
    opened = reduceVadGate(opened, { at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.9 }).state;
    expect(opened.phase).toBe('listening');

    let checked = 0;
    for (let step = 0; step <= 100; step += 1) {
      const probability = step / 100;
      const before = opened.last_voice_at_ms;
      const after = reduceVadGate(opened, { at_ms: VAD_ONSET_MIN_MS + 10, speech_probability: probability })
        .state.last_voice_at_ms;
      const vadCountedItAsVoice = after !== before;
      expect(isVoiceFrame(probability)).toBe(vadCountedItAsVoice);
      checked += 1;
    }
    expect(checked).toBe(101);
  });
});

describe('pauseMsSince', () => {
  it('measures elapsed silence', () => {
    expect(pauseMsSince(1_000, 1_150)).toBe(150);
    expect(pauseMsSince(1_000, 1_000)).toBe(0);
  });

  it('reports no measurable pause when no voice frame has been seen this turn', () => {
    expect(pauseMsSince(null, 5_000)).toBe(0);
    // ...and that value is inert: decideEndpoint short-circuits before looksIncomplete, which is
    // exactly the pre-wiring behaviour, so a mic that never streams cannot cut anyone off.
    expect(decideEndpoint('file my taxes', pauseMsSince(null, 5_000), 1_000)).toEqual({
      kind: 'keep_listening',
      reason: 'too_soon',
    });
  });

  it('clamps a backwards clock to zero rather than reporting negative silence', () => {
    expect(pauseMsSince(2_000, 1_400)).toBe(0);
  });

  it('returns 0 for non-finite inputs instead of poisoning the threshold comparison with NaN', () => {
    expect(pauseMsSince(Number.NaN, 1_000)).toBe(0);
    expect(pauseMsSince(1_000, Number.NaN)).toBe(0);
    expect(pauseMsSince(Number.POSITIVE_INFINITY, 1_000)).toBe(0);
  });

  it('produces a confirmed endpoint at the documented threshold and not before', () => {
    const at = (elapsedMs: number) => decideEndpoint('file my taxes', pauseMsSince(1_000, 1_000 + elapsedMs), 1_000);
    expect(at(ENDPOINT_COMPLETE_PAUSE_MS).kind).toBe('endpoint');
    expect(at(ENDPOINT_COMPLETE_PAUSE_MS - 1).kind).toBe('eager_endpoint');
  });

  it('keeps listening through a long pause on a trailing Hinglish conjunction', () => {
    // The bilingual marker fix is only reachable once a real pause is measured; before this
    // wiring, decideEndpoint returned `too_soon` here and never consulted looksIncomplete at all.
    const pauseMs = pauseMsSince(1_000, 1_000 + 600);
    expect(pauseMs).toBe(600);
    expect(decideEndpoint('mujhe taxes file karni hain aur', pauseMs, 4_000)).toEqual({
      kind: 'keep_listening',
      reason: 'incomplete_thought',
    });
    expect(decideEndpoint('मुझे टैक्स फाइल करना है और', pauseMs, 4_000)).toEqual({
      kind: 'keep_listening',
      reason: 'incomplete_thought',
    });
  });
});
