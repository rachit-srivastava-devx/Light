import { describe, expect, it, vi } from 'vitest';

import {
  INITIAL_BACKCHANNEL_STATE,
  advanceState,
  decideBackchannel,
} from './Backchannel';
import { decideBargeIn } from './BargeIn';
import { RelayClient, type SocketLike } from './RelayClient';
import { decideEndpoint, looksIncomplete, ENDPOINT_EAGER_PAUSE_MS } from './SemanticEndpointer';
import { INITIAL_VAD_GATE_STATE, reduceVadGate, VAD_EAGER_POST_ENDPOINT_MS } from './VADGate';
import {
  BACKCHANNEL_MAX_PER_UTTERANCE,
  BARGE_IN_MIN_SPEECH_MS,
  ENDPOINT_COMPLETE_PAUSE_MS,
  ENDPOINT_INCOMPLETE_MAX_MS,
  VAD_ONSET_MIN_MS,
  VAD_POST_ENDPOINT_CLOSE_MS,
} from './contracts';

describe('SemanticEndpointer', () => {
  // Three tiers now, not two (docs/adr/0013-vad-and-endpointing-architecture.md): genuinely too
  // soon, below the eager threshold; eager (probably done, non-committal — Track J's hook); and
  // confirmed (actually ends the turn). This test used to assert `ENDPOINT_COMPLETE_PAUSE_MS - 1`
  // stayed `too_soon`; it now falls inside the new eager window, so it is split into the two cases
  // below rather than silently changed in place.
  it('keeps listening before the eager pause elapses (genuinely too soon)', () => {
    const d = decideEndpoint('file my taxes', ENDPOINT_EAGER_PAUSE_MS - 1, 1_000);
    expect(d).toEqual({ kind: 'keep_listening', reason: 'too_soon' });
  });

  it('signals eager (non-committal) once the eager pause elapses, before confirming', () => {
    const d = decideEndpoint('file my taxes', ENDPOINT_COMPLETE_PAUSE_MS - 1, 1_000);
    expect(d).toEqual({ kind: 'eager_endpoint', reason: 'complete_thought_eager' });
  });

  it('endpoints (confirmed) a complete thought after the full pause', () => {
    const d = decideEndpoint('file my taxes', ENDPOINT_COMPLETE_PAUSE_MS, 1_000);
    expect(d).toEqual({ kind: 'endpoint', reason: 'complete_thought' });
  });

  it('waits through a trailing conjunction rather than cutting the user off', () => {
    // The product-critical case: ADHD speech trails and restarts, and a silence timer would
    // endpoint here mid-thought.
    const d = decideEndpoint('I need to file my taxes and', 500, 2_000);
    expect(d).toEqual({ kind: 'keep_listening', reason: 'incomplete_thought' });
  });

  it('endpoints at the hard cap even mid-thought', () => {
    const d = decideEndpoint('and then also um', 200, ENDPOINT_INCOMPLETE_MAX_MS);
    expect(d).toEqual({ kind: 'endpoint', reason: 'hard_cap' });
  });

  it('treats an empty transcript as incomplete', () => {
    expect(looksIncomplete('')).toBe(true);
    expect(looksIncomplete('   ')).toBe(true);
  });

  it('ignores trailing punctuation when matching continuations', () => {
    expect(looksIncomplete('I want to do the thing and.')).toBe(true);
  });

  it('does not treat a word merely containing a marker as a continuation', () => {
    // "band" ends with "and" as a substring — the same class of bug LESSONS L7 logged in the
    // intent classifier. Matching is word-boundary aware here.
    expect(looksIncomplete('I need to book the band')).toBe(false);
  });
});

describe('Backchannel', () => {
  const base = {
    pause_ms: 400,
    utterance_elapsed_ms: 5_000,
    emotional_load: 0.1,
    state: INITIAL_BACKCHANNEL_STATE,
  };

  it('plays on a mid-window pause in a settled utterance', () => {
    expect(decideBackchannel(base)).toEqual({ play: true });
  });

  it('suppresses entirely at high emotional load, overriding every other condition', () => {
    // Even with a perfect pause and full budget: backchannelling at distress reads as dismissive.
    const d = decideBackchannel({ ...base, emotional_load: 0.95 });
    expect(d).toEqual({ play: false, reason: 'emotional_load_high' });
  });

  it('stays quiet in the first seconds of an utterance', () => {
    const d = decideBackchannel({ ...base, utterance_elapsed_ms: 1_000 });
    expect(d).toEqual({ play: false, reason: 'too_early_in_utterance' });
  });

  it('never fires twice in a row', () => {
    const state = advanceState(INITIAL_BACKCHANNEL_STATE, { play: true });
    const d = decideBackchannel({ ...base, state });
    expect(d).toEqual({ play: false, reason: 'would_be_consecutive' });
  });

  it('stops after the per-utterance budget', () => {
    const state = { played_this_utterance: BACKCHANNEL_MAX_PER_UTTERANCE, last_decision_was_play: false };
    const d = decideBackchannel({ ...base, state });
    expect(d).toEqual({ play: false, reason: 'budget_spent' });
  });

  it('ignores a pause long enough to be an endpoint', () => {
    const d = decideBackchannel({ ...base, pause_ms: 900 });
    expect(d).toEqual({ play: false, reason: 'pause_too_long' });
  });

  it('ignores a pause too short to be a real breath', () => {
    const d = decideBackchannel({ ...base, pause_ms: 100 });
    expect(d).toEqual({ play: false, reason: 'pause_too_short' });
  });
});

describe('BargeIn', () => {
  const base = { orb_is_speaking: true, sustained_speech_ms: 300, is_echo_residue: false };

  it('yields to sustained speech while the orb is talking', () => {
    expect(decideBargeIn(base)).toEqual({ yield: true });
  });

  it('does not yield to a blip shorter than the minimum', () => {
    const d = decideBargeIn({ ...base, sustained_speech_ms: BARGE_IN_MIN_SPEECH_MS - 1 });
    expect(d).toEqual({ yield: false, reason: 'too_brief' });
  });

  it('does not interrupt itself on echo residue', () => {
    const d = decideBargeIn({ ...base, is_echo_residue: true });
    expect(d).toEqual({ yield: false, reason: 'echo_residue' });
  });

  it('is a no-op when the orb is silent', () => {
    const d = decideBargeIn({ ...base, orb_is_speaking: false });
    expect(d).toEqual({ yield: false, reason: 'orb_silent' });
  });
});

function fakeSocket(): SocketLike & { sent: (string | ArrayBuffer)[]; closeCalls: number } {
  return {
    sent: [],
    closeCalls: 0,
    send(data) {
      this.sent.push(data);
    },
    close() {
      this.closeCalls += 1;
    },
    onmessage: null,
    onclose: null,
  };
}

function handlers() {
  return {
    onListeningConfirmed: vi.fn(),
    onTranscript: vi.fn(),
    onSpeechStarting: vi.fn(),
    onSpeechAudio: vi.fn(),
    onSpeechComplete: vi.fn(),
    onClosing: vi.fn(),
    onProtocolError: vi.fn(),
  };
}

describe('RelayClient', () => {
  it('sends frames matching the relay-rs wire format', () => {
    const socket = fakeSocket();
    const client = new RelayClient(socket, 't1', 's1', handlers());
    client.startListening();
    expect(JSON.parse(socket.sent[0] as string)).toEqual({
      type: 'start_listening',
      tenant_id: 't1',
      session_id: 's1',
    });
  });

  it('starts the relay session before a launch greeting is spoken', () => {
    const socket = fakeSocket();
    const client = new RelayClient(socket, 't1', 's1', handlers());

    client.speak('Good morning, Rachit', 'orb.warm.v1', 'warm');

    expect(socket.sent.map((frame) => JSON.parse(frame as string).type)).toEqual(['start_listening', 'speak']);
  });

  it('dispatches the relay delivery receipt separately from local mic-open', () => {
    // The defect this guards against: the client used to have no way to distinguish "the relay
    // acknowledged start_listening" from "the mic hardware opened locally" — the UI inferred
    // listening from mic capture alone. `onListeningConfirmed` is the only signal that should
    // ever represent the relay side of that state.
    const socket = fakeSocket();
    const h = handlers();
    new RelayClient(socket, 't1', 's1', h);
    expect(h.onListeningConfirmed).not.toHaveBeenCalled();
    socket.onmessage?.({
      data: JSON.stringify({ type: 'listening_confirmed', tenant_id: 't1', session_id: 's1' }),
    });
    expect(h.onListeningConfirmed).toHaveBeenCalledTimes(1);
  });

  it('dispatches a transcript frame to the handler', () => {
    const socket = fakeSocket();
    const h = handlers();
    const log = vi.fn();
    new RelayClient(socket, 't1', 's1', h, { log });
    socket.onmessage?.({
      data: JSON.stringify({ type: 'transcript', tenant_id: 't1', session_id: 's1', text: 'hi', is_final: true }),
    });
    expect(h.onTranscript).toHaveBeenCalledWith('hi', true);
    expect(log).toHaveBeenCalledWith(
      'stt.transcript.received',
      expect.objectContaining({
        provider: 'relay-rs',
        session_id: 's1',
        is_final: true,
        transcript_kind: 'final',
        text: 'hi',
        text_chars: 2,
      }),
    );
  });

  it('dispatches binary TTS chunks to the audio handler', () => {
    const socket = fakeSocket();
    const h = handlers();
    new RelayClient(socket, 't1', 's1', h);
    const chunk = new ArrayBuffer(320);
    socket.onmessage?.({ data: chunk });
    expect(h.onSpeechAudio).toHaveBeenCalledWith(chunk);
  });

  it('supports Blob binary frames without letting speech_complete overtake the audio', async () => {
    const socket = fakeSocket();
    const h = handlers();
    new RelayClient(socket, 't1', 's1', h);
    const chunk = new Uint8Array(320).fill(7);
    socket.onmessage?.({ data: new Blob([chunk]) });
    socket.onmessage?.({
      data: JSON.stringify({ type: 'speech_complete', tenant_id: 't1', session_id: 's1' }),
    });

    await new Promise((resolve) => setTimeout(resolve, 0));

    const delivered = h.onSpeechAudio.mock.calls[0]?.[0] as ArrayBuffer;
    expect(new Uint8Array(delivered)).toEqual(chunk);
    expect(h.onSpeechComplete).toHaveBeenCalledTimes(1);
  });

  it('releases the mic immediately on pause without waiting for the server', () => {
    // §5: the mic must be released now, not after a round trip.
    const socket = fakeSocket();
    const client = new RelayClient(socket, 't1', 's1', handlers());
    client.pause();
    expect(socket.closeCalls).toBe(1);
    expect(client.isClosed).toBe(true);
  });

  it('sends nothing after a pause', () => {
    const socket = fakeSocket();
    const client = new RelayClient(socket, 't1', 's1', handlers());
    client.pause();
    const afterPause = socket.sent.length;
    client.speak('hello', 'v', 'warm');
    client.sendAudio(new ArrayBuffer(320));
    expect(socket.sent.length).toBe(afterPause);
  });

  it('surfaces the close reason so the presence plane can act oppositely on each', () => {
    const socket = fakeSocket();
    const h = handlers();
    new RelayClient(socket, 't1', 's1', h);
    socket.onmessage?.({
      data: JSON.stringify({ type: 'closing', tenant_id: 't1', session_id: 's1', reason: 'provider_failure' }),
    });
    expect(h.onClosing).toHaveBeenCalledWith('provider_failure');
  });

  it('reports malformed JSON instead of throwing mid-audio', () => {
    const socket = fakeSocket();
    const h = handlers();
    new RelayClient(socket, 't1', 's1', h);
    expect(() => socket.onmessage?.({ data: 'not json' })).not.toThrow();
    expect(h.onProtocolError).toHaveBeenCalled();
  });

  it('reports an unknown frame type rather than crashing on version skew', () => {
    const socket = fakeSocket();
    const h = handlers();
    new RelayClient(socket, 't1', 's1', h);
    socket.onmessage?.({ data: JSON.stringify({ type: 'teleport', tenant_id: 't1', session_id: 's1' }) });
    expect(h.onProtocolError).toHaveBeenCalled();
  });
});

describe('VADGate', () => {
  it('requires sustained speech before opening the relay socket', () => {
    const first = reduceVadGate(INITIAL_VAD_GATE_STATE, { at_ms: 1_000, speech_probability: 0.8 });
    expect(first.decision).toEqual({ kind: 'hold', reason: 'candidate' });
    const second = reduceVadGate(first.state, {
      at_ms: 1_000 + VAD_ONSET_MIN_MS,
      speech_probability: 0.82,
    });
    expect(second.decision).toEqual({ kind: 'start_listening' });
  });

  it('resets a candidate on quiet rather than billing a breath', () => {
    const first = reduceVadGate(INITIAL_VAD_GATE_STATE, { at_ms: 10, speech_probability: 0.8 });
    const second = reduceVadGate(first.state, { at_ms: 30, speech_probability: 0.1 });
    expect(second.state).toEqual(INITIAL_VAD_GATE_STATE);
    expect(second.decision).toEqual({ kind: 'ignore', reason: 'candidate_reset' });
  });

  it('closes after the post-endpoint quiet window', () => {
    let r = reduceVadGate(INITIAL_VAD_GATE_STATE, { at_ms: 0, speech_probability: 0.8 });
    r = reduceVadGate(r.state, { at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.8 });
    r = reduceVadGate(r.state, {
      at_ms: VAD_ONSET_MIN_MS + VAD_POST_ENDPOINT_CLOSE_MS,
      speech_probability: 0.0,
    });
    expect(r.decision).toEqual({ kind: 'end_of_turn', reason: 'post_endpoint' });
    expect(r.state).toEqual(INITIAL_VAD_GATE_STATE);
  });

  it('signals eager_end_of_turn once, strictly before the confirmed close, without ending the turn', () => {
    // The audio-domain half of the eager/confirmed pair (docs/adr/0013): fires while still
    // 'listening' (audio keeps flowing — Track J's filler scheduler subscribes to this event, not
    // to a turn-completion signal), and at most once per quiet run even though frames keep arriving
    // every ~20ms through the eager window.
    let r = reduceVadGate(INITIAL_VAD_GATE_STATE, { at_ms: 0, speech_probability: 0.8 });
    r = reduceVadGate(r.state, { at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.8 });
    const listeningStart = VAD_ONSET_MIN_MS;
    r = reduceVadGate(r.state, { at_ms: listeningStart + VAD_EAGER_POST_ENDPOINT_MS, speech_probability: 0.0 });
    expect(r.decision).toEqual({ kind: 'eager_end_of_turn', reason: 'post_endpoint_quiet_eager' });
    expect(r.state.phase).toBe('listening');
    // A second quiet frame in the same run does not re-signal eager (one-shot latch).
    const again = reduceVadGate(r.state, {
      at_ms: listeningStart + VAD_EAGER_POST_ENDPOINT_MS + 20,
      speech_probability: 0.0,
    });
    expect(again.decision).toEqual({ kind: 'hold', reason: 'post_endpoint_quiet' });
    // ...and the confirmed close still fires on schedule afterwards.
    const closed = reduceVadGate(again.state, {
      at_ms: listeningStart + VAD_POST_ENDPOINT_CLOSE_MS,
      speech_probability: 0.0,
    });
    expect(closed.decision).toEqual({ kind: 'end_of_turn', reason: 'post_endpoint' });
  });

  it('a resumed voice clears the eager latch so a later quiet run can signal eager again', () => {
    let r = reduceVadGate(INITIAL_VAD_GATE_STATE, { at_ms: 0, speech_probability: 0.8 });
    r = reduceVadGate(r.state, { at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.8 });
    const t0 = VAD_ONSET_MIN_MS;
    r = reduceVadGate(r.state, { at_ms: t0 + VAD_EAGER_POST_ENDPOINT_MS, speech_probability: 0.0 });
    expect(r.decision.kind).toBe('eager_end_of_turn');
    // Speaker resumes briefly, then goes quiet again — a fresh eager signal should be available.
    r = reduceVadGate(r.state, { at_ms: t0 + VAD_EAGER_POST_ENDPOINT_MS + 40, speech_probability: 0.9 });
    expect(r.decision).toEqual({ kind: 'hold', reason: 'voice_active' });
    const t1 = t0 + VAD_EAGER_POST_ENDPOINT_MS + 40;
    r = reduceVadGate(r.state, { at_ms: t1 + VAD_EAGER_POST_ENDPOINT_MS, speech_probability: 0.0 });
    expect(r.decision).toEqual({ kind: 'eager_end_of_turn', reason: 'post_endpoint_quiet_eager' });
  });

  it('an eager pre-close threshold configured past the confirmed close never fires (confirmed still wins)', () => {
    // Defensive: a misconfigured eager threshold must not create a state where eager fires AFTER
    // (or is starved by) the confirmed close firing first — confirmed close is checked first in
    // source order, so an eager threshold >= the confirmed one is simply never reached.
    let r = reduceVadGate(INITIAL_VAD_GATE_STATE, { at_ms: 0, speech_probability: 0.8 });
    r = reduceVadGate(r.state, { at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.8 });
    r = reduceVadGate(
      r.state,
      { at_ms: VAD_ONSET_MIN_MS + VAD_POST_ENDPOINT_CLOSE_MS, speech_probability: 0.0 },
      { eagerPostEndpointMs: VAD_POST_ENDPOINT_CLOSE_MS + 1_000 },
    );
    expect(r.decision).toEqual({ kind: 'end_of_turn', reason: 'post_endpoint' });
  });

  it('fails closed on malformed frames without mutating state', () => {
    const state = {
      phase: 'listening' as const,
      candidate_started_at_ms: null,
      turn_started_at_ms: 0,
      last_voice_at_ms: 10,
    };
    const r = reduceVadGate(state, { at_ms: Number.NaN, speech_probability: 0.8 });
    expect(r.state).toBe(state);
    expect(r.decision).toEqual({ kind: 'ignore', reason: 'invalid_frame' });
  });

  it('hard-caps a turn even when speech never endpoints', () => {
    let r = reduceVadGate(INITIAL_VAD_GATE_STATE, { at_ms: 0, speech_probability: 0.8 });
    r = reduceVadGate(r.state, { at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.8 });
    r = reduceVadGate(r.state, { at_ms: ENDPOINT_INCOMPLETE_MAX_MS, speech_probability: 0.8 });
    expect(r.decision).toEqual({ kind: 'end_of_turn', reason: 'hard_cap' });
  });
});
