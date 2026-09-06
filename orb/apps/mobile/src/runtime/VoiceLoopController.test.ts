import { describe, expect, it, vi } from 'vitest';

import { screenModelFromEnvelope } from '../AppModel';
import { PAUSE_CONTRACT_SEQUENCE } from '../presence/contracts';
import { BED_STATUS_CUTOFF_HZ, THINKING_MORPH_ENGAGE_BUDGET_MS } from '../presence/ColorMorph';
import { getOrBootPresenceBed, type PresenceAudioContext } from '../presence/PresenceBoot';
import { RelayClient, type SocketLike } from '../voice/RelayClient';
import { VAD_EAGER_POST_ENDPOINT_MS } from '../voice/VADGate';
import { VAD_ONSET_MIN_MS, VAD_POST_ENDPOINT_CLOSE_MS } from '../voice/contracts';
import type { AtomizerPort, AtomizerPortInput } from './AtomizerPort';
import { createT0FocusSession } from './T0FocusSession';
import { createVoiceLoopController, type VoiceLoopController, type VoiceLoopEvent } from './VoiceLoopController';

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

/**
 * Mirrors App.tsx's real wiring: RelayClient's onTranscript callback feeds straight into the
 * voice loop's handleTranscript. `getLoop` is a thunk because RelayClient must exist before
 * createVoiceLoopController can be called (the loop needs the relay), so the loop itself isn't
 * available yet at the point this callback is defined — same forward-reference shape App.tsx uses.
 */
function relayWith(socket: SocketLike, getLoop: () => { handleTranscript: VoiceLoopController['handleTranscript'] }) {
  return new RelayClient(socket, 't1', 's1', {
    onTranscript: (text, isFinal) => {
      void getLoop().handleTranscript(text, isFinal, 0, 0);
    },
    onSpeechStarting: vi.fn(),
    onSpeechAudio: vi.fn(),
    onSpeechComplete: vi.fn(),
    onClosing: vi.fn(),
    onProtocolError: vi.fn(),
  });
}

function atomizerReturning(stepText: string, observedTasks: string[] = []): AtomizerPort {
  return {
    async atomize(input: AtomizerPortInput) {
      observedTasks.push(input.task);
      return {
        output: {
          steps: [{ step_text: stepText, est_min: 1, done_signal: `${stepText} is visible` }],
          steps_total: 1,
        },
        envelope_source: 'model',
        atomizer_source: 'model',
        spent_paise: 1,
        latency_ms: 1,
      };
    },
  };
}

function controller(socket = fakeSocket(), atomizer = atomizerReturning('Open the tax portal')) {
  let loop: ReturnType<typeof createVoiceLoopController>;
  const relay = relayWith(socket, () => loop);
  loop = createVoiceLoopController(
    createT0FocusSession({
      tenant_id: 't1',
      user_id: 'u1',
      session_id: 's1',
      task: 'Open the tax portal',
    }, atomizer),
    relay,
  );
  return { socket, loop };
}

/**
 * The pre-existing `fakeSocket` is a pure sink: an `end_of_turn` frame sent into it produces no
 * server answer, so every test above is blind to what the REAL relay does next. `relay-rs`'s
 * `session.rs` answers `ClientFrame::EndOfTurn` by calling `stt.end_of_turn()` and sending back a
 * `ServerFrame::Transcript { is_final: true }` — which re-enters the loop through
 * `RelayClient.onTranscript` and completes the turn a second time.
 *
 * This socket mirrors that one behaviour and nothing else, so "how many times did one utterance
 * get answered?" becomes an assertable property rather than an assumption.
 */
function echoingSocket(finalText: string): SocketLike & {
  sent: (string | ArrayBuffer)[];
  closeCalls: number;
  deliver: () => void;
} {
  const socket: SocketLike & { sent: (string | ArrayBuffer)[]; closeCalls: number; deliver: () => void } = {
    sent: [],
    closeCalls: 0,
    send(data) {
      this.sent.push(data);
      if (typeof data !== 'string') return;
      const frame = JSON.parse(data) as { type: string };
      // relay-rs finalises STT synchronously inside the end_of_turn handler; the transcript comes
      // back over the socket, i.e. asynchronously from the client's point of view.
      if (frame.type === 'end_of_turn') pending.push(JSON.stringify({
        type: 'transcript',
        tenant_id: 't1',
        session_id: 's1',
        text: finalText,
        is_final: true,
      }));
    },
    close() {
      this.closeCalls += 1;
    },
    onmessage: null,
    onclose: null,
    deliver() {
      const frames = pending.splice(0);
      for (const frame of frames) this.onmessage?.({ data: frame });
    },
  };
  const pending: string[] = [];
  return socket;
}

/** Same wiring as `controller()`, but every re-entrant `handleTranscript` promise is awaitable. */
function echoingController(finalText: string, atomizer = atomizerReturning('Open the tax portal')) {
  const socket = echoingSocket(finalText);
  const inFlight: Promise<readonly VoiceLoopEvent[]>[] = [];
  let loop: ReturnType<typeof createVoiceLoopController>;
  const relay = new RelayClient(socket, 't1', 's1', {
    onTranscript: (text, isFinal) => {
      inFlight.push(loop.handleTranscript(text, isFinal, 0, 0));
    },
    onSpeechStarting: vi.fn(),
    onSpeechAudio: vi.fn(),
    onSpeechComplete: vi.fn(),
    onClosing: vi.fn(),
    onProtocolError: vi.fn(),
  });
  loop = createVoiceLoopController(
    createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'Open the tax portal' },
      atomizer,
    ),
    relay,
  );
  /** Drains the relay's queued answers until the loop stops producing new work. */
  async function settle(): Promise<readonly VoiceLoopEvent[]> {
    const events: VoiceLoopEvent[] = [];
    for (let round = 0; round < 8; round += 1) {
      socket.deliver();
      if (inFlight.length === 0) return events;
      for (const batch of await Promise.all(inFlight.splice(0))) events.push(...batch);
    }
    return events;
  }
  return { socket, loop, settle };
}

function frameTypes(socket: { sent: (string | ArrayBuffer)[] }): string[] {
  return socket.sent.map((frame) => (typeof frame === 'string' ? (JSON.parse(frame) as { type: string }).type : 'binary'));
}

function fakePresenceContext(targets: number[]): PresenceAudioContext {
  const node = { connect() {} };
  const param = () => ({
    value: 0,
    setValueAtTime() {},
    setTargetAtTime(target: number) {
      targets.push(target);
    },
    linearRampToValueAtTime() {},
  });
  return {
    currentTime: 1,
    sampleRate: 48_000,
    destination: node,
    createBuffer: (_channels, length) => {
      const samples = new Float32Array(length);
      return { getChannelData: () => samples };
    },
    createBufferSource: () => ({
      ...node,
      buffer: null,
      loop: false,
      start() {},
      stop() {},
    }),
    createGain: () => ({ ...node, gain: param() }),
    createBiquadFilter: () => ({ ...node, type: '', frequency: param(), Q: param() }),
  };
}

describe('VoiceLoopController', () => {
  it('starts the runtime and opens the relay listening turn', async () => {
    const { socket, loop } = controller();
    const events = await loop.start();
    expect(events.map((event) => event.kind)).toEqual(['envelope', 'relay_start_listening']);
    expect(JSON.parse(socket.sent[0] as string)).toEqual({
      type: 'start_listening',
      tenant_id: 't1',
      session_id: 's1',
    });
  });

  /**
   * Regression for docs/REVIEW-2026-08-06-END-TO-END-FLOW.md #1 ("app start immediately injects
   * the final transcript ... before any user speech"). `T0FocusSession`'s production constructor
   * used to default its (now-removed) `@pe/realtime-voice` port to
   * `createMemoryRealtimeVoiceFeature({ transcript: input.task })` — a fake transcript built from
   * the launch-time seed task, fed in at construction time, before a single mic frame exists.
   * `start()` also called into that fake port before anything from a real STT provider could have
   * arrived. Boot here with an atomizer spy standing in for the real STT/atomizer path and an STT
   * source that never emits anything (no `handleAudioFrame`/`handleTranscript` call at all — pure
   * silence): the atomizer must never fire and the spoken launch text must be the fixed greeting,
   * never the seed task string that would only be legitimate if it had come from real speech.
   */
  it('does not set any transcript-derived state at boot — only a real STT transcript may drive the atomizer', async () => {
    const observedTasks: string[] = [];
    const atomizer = atomizerReturning('Open the tax portal', observedTasks);
    const { loop } = controller(fakeSocket(), atomizer);

    const events = await loop.start();

    expect(observedTasks).toEqual([]);
    const envelope = events.find((event) => event.kind === 'envelope');
    expect(envelope?.kind === 'envelope' ? envelope.envelope.speech?.text : undefined).not.toBe(
      'Open the tax portal',
    );
  });

  it('routes sustained VAD speech into relay audio frames', async () => {
    const { socket, loop } = controller();
    await loop.handleAudioFrame({ at_ms: 0, speech_probability: 0.8 }, new ArrayBuffer(4));
    const opening = await loop.handleAudioFrame({ at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.9 }, new ArrayBuffer(4));
    const sent = await loop.handleAudioFrame(
      { at_ms: VAD_ONSET_MIN_MS + 10, speech_probability: 0.9 },
      new ArrayBuffer(12),
    );
    expect(opening).toEqual([
      { kind: 'relay_start_listening' },
      { kind: 'relay_audio_sent', bytes: 4 },
      { kind: 'relay_audio_sent', bytes: 4 },
    ]);
    expect(sent).toEqual([{ kind: 'relay_audio_sent', bytes: 12 }]);
    expect(socket.sent.filter((frame) => frame instanceof ArrayBuffer && frame.byteLength === 4)).toHaveLength(2);
    expect(socket.sent.some((frame) => frame instanceof ArrayBuffer && frame.byteLength === 12)).toBe(true);
  });

  it('does not complete the turn locally on VAD end-of-turn — waits for the relay transcript', async () => {
    // Regression test: a local VAD end-of-turn used to fall back to decoding raw mic PCM bytes as
    // UTF-8 text and process THAT immediately, then process the real transcript again when it
    // arrived from the relay — two responses per utterance, the first one garbage. It must now
    // produce no envelope until the real transcript actually arrives.
    const observedTasks: string[] = [];
    const { socket, loop } = controller(fakeSocket(), atomizerReturning('open the tax portal', observedTasks));
    await loop.start();
    await loop.handleAudioFrame({ at_ms: 0, speech_probability: 0.8 }, new ArrayBuffer(4));
    await loop.handleAudioFrame({ at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.9 }, new ArrayBuffer(4));
    const vadEvents = await loop.handleAudioFrame(
      { at_ms: VAD_ONSET_MIN_MS + VAD_POST_ENDPOINT_CLOSE_MS, speech_probability: 0 },
      new ArrayBuffer(0),
    );

    // VAD detecting silence must not, by itself, produce a spoken response or call the atomizer.
    expect(vadEvents).toEqual([{ kind: 'relay_end_of_turn', reason: 'vad' }]);
    expect(observedTasks).toEqual([]);
    expect(socket.sent.some((frame) => typeof frame === 'string' && JSON.parse(frame).type === 'end_of_turn')).toBe(
      true,
    );

    // Now simulate the relay's real STT response arriving asynchronously, as it would over the
    // real WebSocket — this is the only thing that should complete the turn.
    socket.onmessage?.({
      data: JSON.stringify({
        type: 'transcript',
        tenant_id: 't1',
        session_id: 's1',
        text: 'just open the tax portal',
        is_final: true,
      }),
    });
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(observedTasks).toEqual(['just open the tax portal']);
  });

  it('schedules the on-device thinking morph at VAD end-of-turn without restarting the bed', async () => {
    const socket = fakeSocket();
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 'presence-s1', task: 'Open it' },
      atomizerReturning('Open it'),
    );
    const morphTargets: number[] = [];
    const graph = getOrBootPresenceBed(runtime, () => fakePresenceContext(morphTargets));
    let loop: ReturnType<typeof createVoiceLoopController>;
    loop = createVoiceLoopController(runtime, relayWith(socket, () => loop));

    await loop.handleAudioFrame({ at_ms: 0, speech_probability: 0.8 }, new ArrayBuffer(4));
    await loop.handleAudioFrame({ at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.9 }, new ArrayBuffer(4));
    const events = await loop.handleAudioFrame(
      { at_ms: VAD_ONSET_MIN_MS + VAD_POST_ENDPOINT_CLOSE_MS, speech_probability: 0 },
      new ArrayBuffer(0),
    );

    expect(events).toEqual([
      {
        kind: 'presence_morph_scheduled',
        status: 'thinking',
        budget_ms: THINKING_MORPH_ENGAGE_BUDGET_MS,
      },
      { kind: 'relay_end_of_turn', reason: 'vad' },
    ]);
    expect(morphTargets).toContain(BED_STATUS_CUTOFF_HZ.thinking);
    expect(graph.bedSource.loop).toBe(true);
  });

  it('covers Track K eager endpointing without committing the turn and reverses on resumed speech', async () => {
    const socket = fakeSocket();
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 'presence-eager-s1', task: 'Open it' },
      atomizerReturning('Open it'),
    );
    const morphTargets: number[] = [];
    const graph = getOrBootPresenceBed(runtime, () => fakePresenceContext(morphTargets));
    let loop: ReturnType<typeof createVoiceLoopController>;
    loop = createVoiceLoopController(runtime, relayWith(socket, () => loop));

    await loop.handleAudioFrame({ at_ms: 0, speech_probability: 0.8 }, new ArrayBuffer(4));
    await loop.handleAudioFrame({ at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.9 }, new ArrayBuffer(4));
    const eager = await loop.handleAudioFrame(
      { at_ms: VAD_ONSET_MIN_MS + VAD_EAGER_POST_ENDPOINT_MS, speech_probability: 0 },
      new ArrayBuffer(4),
    );

    expect(eager).toEqual([
      {
        kind: 'presence_morph_scheduled',
        status: 'thinking',
        budget_ms: THINKING_MORPH_ENGAGE_BUDGET_MS,
      },
      { kind: 'relay_audio_sent', bytes: 4 },
    ]);
    expect(socket.sent.some((frame) => typeof frame === 'string' && JSON.parse(frame).type === 'end_of_turn')).toBe(
      false,
    );

    const resumed = await loop.handleAudioFrame(
      { at_ms: VAD_ONSET_MIN_MS + VAD_EAGER_POST_ENDPOINT_MS + 10, speech_probability: 0.9 },
      new ArrayBuffer(4),
    );
    expect(resumed).toEqual([
      {
        kind: 'presence_morph_scheduled',
        status: 'idle',
        budget_ms: THINKING_MORPH_ENGAGE_BUDGET_MS,
      },
      { kind: 'relay_audio_sent', bytes: 4 },
    ]);
    expect(morphTargets).toContain(BED_STATUS_CUTOFF_HZ.thinking);
    expect(morphTargets).toContain(BED_STATUS_CUTOFF_HZ.idle);
    expect(graph.bedSource.loop).toBe(true);
  });

  it('covers a semantic eager endpoint without treating it as a confirmed turn', async () => {
    const socket = fakeSocket();
    const runtime = createT0FocusSession(
      { tenant_id: 't1', user_id: 'u1', session_id: 'presence-semantic-s1', task: 'Open it' },
      atomizerReturning('Open it'),
    );
    const morphTargets: number[] = [];
    getOrBootPresenceBed(runtime, () => fakePresenceContext(morphTargets));
    let loop: ReturnType<typeof createVoiceLoopController>;
    loop = createVoiceLoopController(runtime, relayWith(socket, () => loop));

    const events = await loop.handleTranscript('open the tax portal', false, 60, 1_000);

    expect(events).toEqual([
      {
        kind: 'presence_morph_scheduled',
        status: 'thinking',
        budget_ms: THINKING_MORPH_ENGAGE_BUDGET_MS,
      },
    ]);
    expect(socket.sent).toEqual([]);
    expect(morphTargets).toContain(BED_STATUS_CUTOFF_HZ.thinking);
  });

  it('speaks a clarify question, not the generic fallback — regression, live-found bug', async () => {
    // Regression test: completeTurn used to call speakCurrentStep() unconditionally after every
    // acceptAudio(), which only knows about `gate.currentStep` — null while still in CLARIFY. It
    // silently discarded acceptAudio's real clarify-question text and spoke "I'm here. Tell me
    // the task." instead. Live-confirmed with real STT+LLM: the user never heard the question the
    // app itself decided to ask. `atomizerReturning` here is never actually reached — the point is
    // that a genuinely ambiguous first utterance triggers CLARIFY before any atomize call.
    const { socket, loop } = controller();
    await loop.start();
    const events = await loop.handleTranscript(
      'I need to reply to my landlord about the leaking sink',
      true,
      0,
      1_000,
    );
    const clarifyEnvelope = events.find((e) => e.kind === 'envelope' && e.envelope.session.state === 'CLARIFY');
    expect(clarifyEnvelope).toBeTruthy();
    const spoken = events.find((e) => e.kind === 'relay_speak');
    expect(spoken).toBeTruthy();
    if (spoken?.kind === 'relay_speak' && clarifyEnvelope?.kind === 'envelope' && clarifyEnvelope.envelope.speech) {
      // The actual regression assertion: what gets spoken must match what the envelope asked,
      // not a generic "I'm here. Tell me the task." fallback.
      expect(spoken.text).toBe(clarifyEnvelope.envelope.speech.text);
      expect(spoken.text).not.toBe("I'm here. Tell me the task.");
    }
    expect(socket.sent.map((frame) => (typeof frame === 'string' ? JSON.parse(frame).type : 'binary'))).toContain(
      'speak',
    );
  });

  it('semantic endpointing ends the turn locally and answers from the relay transcript', async () => {
    // A semantic endpoint is a decision about WHEN the user stopped, never about WHAT they said:
    // it sends end_of_turn ~150ms into the pause instead of waiting out VADGate's 800ms hangover,
    // and the answer is then built from the authoritative final transcript the relay returns.
    // Asserting the round trip (not just the local events) is what makes "exactly one answer"
    // observable — the plain `fakeSocket` never replies to end_of_turn, so it cannot see it.
    const { socket, loop, settle } = echoingController('just open the tax portal');
    await loop.start();

    const local = await loop.handleTranscript('just open the tax portal', false, 200, 1_000);
    // No presence bed is booted for this runtime, so the 'thinking' morph is a no-op here; the
    // eager-endpoint test above is where the morph itself is asserted.
    expect(local).toEqual([{ kind: 'relay_end_of_turn', reason: 'semantic' }]);
    // Nothing is spoken and no envelope exists yet — the words are still in flight.
    expect(frameTypes(socket)).toEqual(['start_listening', 'end_of_turn']);

    const answered = await settle();
    expect(answered.filter((event) => event.kind === 'envelope').map((event) => screenModelFromEnvelope(event.envelope).stateLabel)).toEqual([
      'STEP PRESENT',
      'WORKING',
    ]);
    expect(answered.some((event) => event.kind === 'relay_speak' && event.text === 'Open the tax portal')).toBe(true);
    expect(frameTypes(socket).filter((type) => type === 'speak')).toHaveLength(1);
  });

  it('plays cached backchannel only inside the allowed pause window', () => {
    const { loop } = controller();
    expect(loop.handleMidUtterancePause(400, 5_000, 0.1)).toEqual([
      { kind: 'play_backchannel', phrase_id: 'backchannel.mm.v1' },
    ]);
    expect(loop.handleMidUtterancePause(400, 5_500, 0.1)).toEqual([
      { kind: 'ignored', reason: 'would_be_consecutive' },
    ]);
  });

  it('resets the backchannel budget after a completed turn', async () => {
    const { loop } = controller();
    expect(loop.handleMidUtterancePause(400, 5_000, 0.1)).toEqual([
      { kind: 'play_backchannel', phrase_id: 'backchannel.mm.v1' },
    ]);
    await loop.handleTranscript('just open the tax portal', true, 0, 1_000);
    expect(loop.handleMidUtterancePause(400, 5_000, 0.1)).toEqual([
      { kind: 'play_backchannel', phrase_id: 'backchannel.mm.v1' },
    ]);
  });

  it('suppresses backchannel at high emotional load', () => {
    const { loop } = controller();
    expect(loop.handleMidUtterancePause(400, 5_000, 0.95)).toEqual([
      { kind: 'ignored', reason: 'emotional_load_high' },
    ]);
  });

  it('sends barge-in only for sustained non-echo speech while the orb is speaking', () => {
    const { socket, loop } = controller();
    expect(
      loop.handleBargeIn({ orb_is_speaking: true, sustained_speech_ms: 250, is_echo_residue: false }),
    ).toEqual([{ kind: 'barge_in_yield' }]);
    expect(socket.sent.map((frame) => (typeof frame === 'string' ? JSON.parse(frame).type : 'binary'))).toContain(
      'barge_in',
    );
  });

  it('plans transient interruption recovery without running the user pause contract', async () => {
    const { loop } = controller();
    expect(
      await loop.handleInterruption({ kind: 'transient', cause: 'audio_stack_reset', at: 1 }),
    ).toEqual([
      {
        kind: 'transient_recovery_planned',
        action: 'ramp_with_cover',
        budget_ms: 300,
        cover_required: true,
      },
    ]);
  });

  it('routes user-intent interruptions through the pause contract', async () => {
    const { loop } = controller();
    const events = await loop.handleInterruption({ kind: 'user_intent', cause: 'backgrounded', at: 1 });
    expect(events.filter((event) => event.kind === 'pause_contract_step').map((event) => event.step)).toEqual(
      PAUSE_CONTRACT_SEQUENCE,
    );
  });

  it('pauses the runtime and closes the relay immediately', async () => {
    const socket = fakeSocket();
    let loop: ReturnType<typeof createVoiceLoopController>;
    loop = createVoiceLoopController(
      createT0FocusSession(
        { tenant_id: 't1', user_id: 'u1', session_id: 's1', task: 'Open it' },
        atomizerReturning('Open it'),
      ),
      relayWith(socket, () => loop),
    );
    const events = await loop.pause();
    const envelopeEvent = events.find((event) => event.kind === 'envelope');
    expect(events.filter((event) => event.kind === 'pause_contract_step').map((event) => event.step)).toEqual(
      PAUSE_CONTRACT_SEQUENCE,
    );
    expect(socket.closeCalls).toBe(1);
    expect(envelopeEvent?.kind).toBe('envelope');
    expect(screenModelFromEnvelope(envelopeEvent!.envelope).stateLabel).toBe('INTERRUPTED');
  });
});

describe('VoiceLoopController turn completion against a relay that answers end_of_turn', () => {
  it('answers a semantically-endpointed utterance exactly once', async () => {
    const { socket, loop, settle } = echoingController('just open the tax portal');
    await loop.start();

    // A real pause of 200ms on a complete-looking partial: the semantic endpointer's confirmed
    // tier. This is the input App.tsx could not produce while pauseMs was hardcoded to 0.
    await loop.handleTranscript('just open the tax portal', false, 200, 1_000);
    await settle();

    expect(frameTypes(socket).filter((type) => type === 'end_of_turn')).toHaveLength(1);
    expect(frameTypes(socket).filter((type) => type === 'speak')).toHaveLength(1);
  });

  it('answers a VAD-endpointed utterance exactly once', async () => {
    const { socket, loop, settle } = echoingController('just open the tax portal');
    await loop.start();

    await loop.handleAudioFrame({ at_ms: 0, speech_probability: 0.9 }, new ArrayBuffer(2));
    await loop.handleAudioFrame({ at_ms: VAD_ONSET_MIN_MS, speech_probability: 0.9 }, new ArrayBuffer(2));
    await loop.handleAudioFrame(
      { at_ms: VAD_ONSET_MIN_MS + VAD_POST_ENDPOINT_CLOSE_MS, speech_probability: 0.05 },
      new ArrayBuffer(2),
    );
    await settle();

    expect(frameTypes(socket).filter((type) => type === 'end_of_turn')).toHaveLength(1);
    expect(frameTypes(socket).filter((type) => type === 'speak')).toHaveLength(1);
  });

  it('does not re-signal end_of_turn on further partials after a semantic endpoint', async () => {
    const { socket, loop, settle } = echoingController('just open the tax portal');
    await loop.start();

    await loop.handleTranscript('just open the tax portal', false, 200, 1_000);
    // STT keeps streaming revised partials while it finalises; none may re-open the turn.
    await loop.handleTranscript('just open the tax portal now', false, 400, 1_200);
    await loop.handleTranscript('just open the tax portal now please', false, 600, 1_400);
    await settle();

    expect(frameTypes(socket).filter((type) => type === 'end_of_turn')).toHaveLength(1);
    expect(frameTypes(socket).filter((type) => type === 'speak')).toHaveLength(1);
  });
});
