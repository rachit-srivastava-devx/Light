import { decideBackchannel, advanceState, INITIAL_BACKCHANNEL_STATE, type BackchannelState } from '../voice/Backchannel';
import { classifyInterruption, pauseContractSteps, planRecovery } from '../presence/InterruptionHandler';
import { getBootedPresenceBed } from '../presence/PresenceBoot';
import { THINKING_MORPH_ENGAGE_BUDGET_MS, type BedStatus } from '../presence/ColorMorph';
import type { InterruptionEvent, PauseContractStep } from '../presence/contracts';
import {
  createBargeInCoordinator,
  decideBargeIn,
  type BargeInInput,
  type BargeInObservation,
} from '../voice/BargeIn';
import { decideEndpoint } from '../voice/SemanticEndpointer';
import { INITIAL_VAD_GATE_STATE, reduceVadGate, type VadFrame, type VadGateState } from '../voice/VADGate';
import type { ResponseEnvelope } from '../lld/ResponseEnvelope';
import type { T0FocusSessionRuntime } from './T0FocusSession';
import { utf8Encode } from '../voice/Utf8';

export interface VoiceLoopRelay {
  readonly isClosed: boolean;
  startListening(): void;
  sendAudio(frame: ArrayBuffer): void;
  endOfTurn(): void;
  speak(text: string, voiceId: string, emotion: string): void;
  bargeIn(): void;
  pause(): void;
}

export type VoiceLoopEvent =
  | { readonly kind: 'envelope'; readonly envelope: ResponseEnvelope }
  | { readonly kind: 'relay_start_listening' }
  | { readonly kind: 'relay_audio_sent'; readonly bytes: number }
  | { readonly kind: 'relay_end_of_turn'; readonly reason: 'vad' | 'semantic' | 'final_transcript' }
  | { readonly kind: 'relay_speak'; readonly text: string }
  | {
      readonly kind: 'presence_morph_scheduled';
      readonly status: BedStatus;
      readonly budget_ms: number;
    }
  | { readonly kind: 'play_backchannel'; readonly phrase_id: 'backchannel.mm.v1' }
  | { readonly kind: 'pause_contract_step'; readonly step: PauseContractStep }
  | {
      readonly kind: 'transient_recovery_planned';
      readonly action: 'ramp_and_swell' | 'ramp_with_cover';
      readonly budget_ms: number;
      readonly cover_required: boolean;
    }
  | { readonly kind: 'barge_in_yield' }
  | { readonly kind: 'ignored'; readonly reason: string };

export interface VoiceLoopController {
  start(): Promise<readonly VoiceLoopEvent[]>;
  handleAudioFrame(frame: VadFrame, audio: ArrayBuffer): Promise<readonly VoiceLoopEvent[]>;
  handleTranscript(text: string, isFinal: boolean, pauseMs: number, utteranceMs: number): Promise<readonly VoiceLoopEvent[]>;
  handleMidUtterancePause(pauseMs: number, utteranceMs: number, emotionalLoad: number): readonly VoiceLoopEvent[];
  handleBargeIn(input: BargeInInput): readonly VoiceLoopEvent[];
  handleBargeInObservation(input: BargeInObservation): readonly VoiceLoopEvent[];
  handleInterruption(event: InterruptionEvent): Promise<readonly VoiceLoopEvent[]>;
  pause(): Promise<readonly VoiceLoopEvent[]>;
}

function bytesFromText(text: string): Uint8Array {
  return utf8Encode(text);
}

function arrayBufferFromBytes(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  return buffer;
}

function speakRelay(relay: VoiceLoopRelay | undefined, envelope: ResponseEnvelope): VoiceLoopEvent {
  if (!envelope.speech) return { kind: 'ignored', reason: 'silent_envelope' };
  const text = envelope.speech.text;
  relay?.speak(text, envelope.speech.voice.voice_id, envelope.orb.emotion);
  return { kind: 'relay_speak', text };
}

export function createVoiceLoopController(
  runtime: T0FocusSessionRuntime,
  relay?: VoiceLoopRelay,
): VoiceLoopController {
  let vadState: VadGateState = INITIAL_VAD_GATE_STATE;
  let backchannelState: BackchannelState = INITIAL_BACKCHANNEL_STATE;
  let relayListening = false;
  let turnAudio: Uint8Array[] = [];
  let presenceStatus: BedStatus = 'idle';
  const bargeInCoordinator = createBargeInCoordinator({
    cancelProviderCall: () => relay?.bargeIn(),
  });
  /**
   * True from the moment either endpointing path has told the relay this turn is over until the
   * turn is actually answered (or new speech opens the next one).
   *
   * Two independent detectors can end the same turn — `VADGate`'s 800ms audio hangover and
   * `SemanticEndpointer`'s 150ms pause-on-a-complete-thought — and `relay-rs` answers every
   * `end_of_turn` frame with a fresh `Transcript { is_final: true }` (`session.rs`). Without this
   * latch the second detector re-sends `end_of_turn`, the relay finalises STT again (billed
   * again), and the user hears the same utterance answered twice. Regression-covered by
   * `VoiceLoopController.test.ts`'s "turn completion against a relay that answers end_of_turn".
   */
  let turnEndSignaled = false;

  function schedulePresenceMorph(status: BedStatus): VoiceLoopEvent | null {
    if (presenceStatus === status) return null;
    const graph = getBootedPresenceBed(runtime);
    if (!graph) return null;
    graph.morphColor(status);
    presenceStatus = status;
    return {
      kind: 'presence_morph_scheduled',
      status,
      budget_ms: THINKING_MORPH_ENGAGE_BUDGET_MS,
    };
  }

  /**
   * Answers a turn from the server's authoritative final transcript.
   *
   * The parameter is deliberately narrowed to the one reason that can legitimately reach here:
   * `end_of_turn` is never re-sent, because 'final_transcript' means the server already told us
   * the turn ended (that is what produced this text) and re-sending would ask a real STT provider
   * to re-derive the same answer at real cost. The two *local* detectors ('vad' and 'semantic')
   * go through `signalTurnEnd` instead — they decide the turn is over, they never decide what was
   * said. See `signalTurnEnd`'s doc comment.
   */
  async function completeTurn(reason: 'final_transcript', transcript: string): Promise<readonly VoiceLoopEvent[]> {
    const presenceEvent = schedulePresenceMorph('thinking');
    turnEndSignaled = false;
    const transcriptText = transcript.trim();
    turnAudio = [];
    backchannelState = INITIAL_BACKCHANNEL_STATE;
    // An empty transcript means the turn carried no recognised speech — room noise, a cough, or an
    // STT miss. Answering it would speak the generic "I'm here. Tell me the task." fallback, and
    // because VAD keeps ending turns on silence that repeats forever (live-observed). Staying quiet
    // is both correct and required: unasked-for speech is exactly what §1's 0ms-silence and the
    // policy's DO-NO-HARM default forbid. The session state is left untouched, so the next real
    // utterance is handled normally.
    if (transcriptText.length === 0) {
      return [
        ...(presenceEvent ? [presenceEvent] : []),
        { kind: 'relay_end_of_turn', reason },
        { kind: 'ignored', reason: 'empty_transcript' },
      ];
    }
    const step = await runtime.acceptAudio(bytesFromText(transcriptText));
    // acceptAudio's own envelope already carries the right thing to say for every branch except
    // "a step just became presentable" (clarify questions, acknowledgments, reanchor lines, the
    // crisis response — all of it lives in `step.speech.text`). Only STEP_PRESENT needs the
    // separate speakCurrentStep() call, because that's the one call that both speaks the step AND
    // dispatches the STEP_PRESENT -> WORKING transition — skipping it there would strand the FSM.
    // Calling it unconditionally (the previous behaviour) silently discarded acceptAudio's real
    // speech text everywhere else and spoke a generic "I'm here. Tell me the task." fallback
    // instead — live-confirmed: a clarify question was never actually heard, only ever returned
    // in the envelope's data.
    if (step.session.state === 'STEP_PRESENT') {
      const speech = await runtime.speakCurrentStep();
      return [
        ...(presenceEvent ? [presenceEvent] : []),
        { kind: 'relay_end_of_turn', reason },
        { kind: 'envelope', envelope: step },
        { kind: 'envelope', envelope: speech },
        speakRelay(relay, speech),
      ];
    }
    return [
      ...(presenceEvent ? [presenceEvent] : []),
      { kind: 'relay_end_of_turn', reason },
      { kind: 'envelope', envelope: step },
      speakRelay(relay, step),
    ];
  }

  /**
   * Tells the relay this turn is over, and stops there.
   *
   * Both local detectors land here, and neither one answers the turn. A local detector knows only
   * that the user *stopped*; the words are still in flight from the relay's STT provider and
   * arrive asynchronously through `handleTranscript(.., isFinal = true)`. Answering here instead
   * would process a partial (or, on the VAD path, raw mic PCM) now and then process the real
   * transcript again moments later — two responses per utterance, one of them wrong.
   *
   * Resetting `vadState` is what stops the *other* detector from re-firing: after a semantic
   * endpoint the audio gate would otherwise still be in `listening` and would send its own
   * `end_of_turn` once the 800ms hangover elapsed. From `idle` it needs a fresh `VAD_ONSET_MIN_MS`
   * of speech to reach `listening` again — which is exactly the next turn.
   */
  function signalTurnEnd(reason: 'vad' | 'semantic'): readonly VoiceLoopEvent[] {
    const presenceEvent = schedulePresenceMorph('thinking');
    turnEndSignaled = true;
    relayListening = false;
    relay?.endOfTurn();
    turnAudio = [];
    backchannelState = INITIAL_BACKCHANNEL_STATE;
    vadState = INITIAL_VAD_GATE_STATE;
    return [
      ...(presenceEvent ? [presenceEvent] : []),
      { kind: 'relay_end_of_turn', reason },
    ];
  }

  function sendAudio(audio: ArrayBuffer): VoiceLoopEvent {
    relay?.sendAudio(audio);
    turnAudio.push(new Uint8Array(audio));
    return { kind: 'relay_audio_sent', bytes: audio.byteLength };
  }

  return {
    async start() {
      const envelope = await runtime.start();
      relay?.startListening();
      relayListening = true;
      turnEndSignaled = false;
      turnAudio = [];
      backchannelState = INITIAL_BACKCHANNEL_STATE;
      return [{ kind: 'envelope', envelope }, { kind: 'relay_start_listening' }];
    },

    async handleAudioFrame(frame, audio) {
      const result = reduceVadGate(vadState, frame);
      vadState = result.state;
      if (result.decision.kind === 'start_listening') {
        const events: VoiceLoopEvent[] = [];
        // Speech after an endpoint is the next turn, whichever detector ended the last one.
        turnEndSignaled = false;
        const presenceEvent = schedulePresenceMorph('idle');
        if (presenceEvent) events.push(presenceEvent);
        if (!relayListening) {
          relay?.startListening();
          relayListening = true;
          events.push({ kind: 'relay_start_listening' });
        }
        for (const acceptedCandidate of turnAudio) {
          relay?.sendAudio(arrayBufferFromBytes(acceptedCandidate));
          events.push({ kind: 'relay_audio_sent', bytes: acceptedCandidate.byteLength });
        }
        events.push(sendAudio(audio));
        return events;
      }
      if (result.decision.kind === 'eager_end_of_turn') {
        // Track K's eager signal is deliberately reversible. Cover the likely endpoint with an
        // on-device timbre morph now, but keep sending microphone audio and do not commit the
        // relay/session turn. If speech resumes, the voice_active branch below returns to idle.
        const presenceEvent = schedulePresenceMorph('thinking');
        return [...(presenceEvent ? [presenceEvent] : []), sendAudio(audio)];
      }
      if (result.decision.kind === 'end_of_turn') {
        // Local VAD detected silence — a signal to stop sending audio, NOT a transcript.
        return signalTurnEnd('vad');
      }
      if (result.decision.reason === 'candidate') {
        turnAudio.push(new Uint8Array(audio));
      }
      if (result.decision.reason === 'candidate_reset') {
        turnAudio = [];
      }
      if (vadState.phase === 'listening') {
        const presenceEvent =
          result.decision.reason === 'voice_active' ? schedulePresenceMorph('idle') : null;
        return [...(presenceEvent ? [presenceEvent] : []), sendAudio(audio)];
      }
      return [{ kind: 'ignored', reason: result.decision.reason }];
    },

    async handleTranscript(text, isFinal, pauseMs, utteranceMs) {
      // The server's final transcript always wins, latch or no latch: it is the authoritative
      // text, and it is what the outstanding `end_of_turn` was sent to obtain.
      if (isFinal) return completeTurn('final_transcript', text);
      // This turn's end is already signalled and its final transcript is in flight. STT keeps
      // streaming revised partials right through finalisation, and every one of them still looks
      // like a complete thought after a long pause — without this guard each re-sends
      // `end_of_turn`, and each of those is another billed STT finalise plus another answer.
      if (turnEndSignaled) return [{ kind: 'ignored', reason: 'turn_end_pending' }];
      const endpoint = decideEndpoint(text, pauseMs, utteranceMs);
      // Confirmed tier (and the 30s hard cap, which arrives on this same path). Local decision:
      // tell the relay to finalise now instead of waiting out VAD's 800ms hangover, then answer
      // from the final transcript that comes back — the ~150-650ms this endpointer exists for.
      if (endpoint.kind === 'endpoint') return signalTurnEnd('semantic');
      if (endpoint.kind === 'eager_endpoint') {
        const presenceEvent = schedulePresenceMorph('thinking');
        if (presenceEvent) return [presenceEvent];
      }
      return [{ kind: 'ignored', reason: endpoint.reason }];
    },

    handleMidUtterancePause(pauseMs, utteranceMs, emotionalLoad) {
      const decision = decideBackchannel({
        pause_ms: pauseMs,
        utterance_elapsed_ms: utteranceMs,
        emotional_load: emotionalLoad,
        state: backchannelState,
      });
      backchannelState = advanceState(backchannelState, decision);
      if (decision.play) return [{ kind: 'play_backchannel', phrase_id: 'backchannel.mm.v1' }];
      return [{ kind: 'ignored', reason: decision.reason }];
    },

    handleBargeIn(input) {
      const decision = decideBargeIn(input);
      if (!decision.yield) return [{ kind: 'ignored', reason: decision.reason }];
      relay?.bargeIn();
      return [{ kind: 'barge_in_yield' }];
    },

    handleBargeInObservation(input) {
      const decision = bargeInCoordinator.observe(input);
      if (!decision.yield) return [{ kind: 'ignored', reason: decision.reason }];
      return [{ kind: 'barge_in_yield' }];
    },

    async handleInterruption(event) {
      const interruption = classifyInterruption(event);
      if (event.kind === 'user_intent') return this.pause();
      const plan = planRecovery(event);
      return [
        {
          kind: 'transient_recovery_planned',
          action: plan.action,
          budget_ms: plan.budget_ms,
          cover_required: interruption.cover_required,
        },
      ];
    },

    async pause() {
      const interruption = classifyInterruption({
        kind: 'user_intent',
        cause: 'manual_pause',
        at: 0,
      });
      relay?.pause();
      relayListening = false;
      turnEndSignaled = false;
      turnAudio = [];
      backchannelState = INITIAL_BACKCHANNEL_STATE;
      bargeInCoordinator.reset();
      presenceStatus = 'idle';
      const steps =
        interruption.kind === 'user_intent'
          ? pauseContractSteps().map((step): VoiceLoopEvent => ({ kind: 'pause_contract_step', step }))
          : [];
      return [...steps, { kind: 'envelope', envelope: await runtime.pause() }];
    },
  };
}
