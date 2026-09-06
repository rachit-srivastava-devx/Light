/**
 * The orb-only native shell promised by the app brief.
 *
 * This component renders no visible text and no visible buttons. The cloud orb is a passive
 * orb-ui-compatible status surface; spoken output leaves through SpeechPort.
 */

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AppState, Platform, Pressable, StyleSheet, Text, TextInput, View } from 'react-native';

import { createFocusOrbActions, requestSessionWarmup } from './AppController';
import { IDLE_ENVELOPE, screenModelFromEnvelope, type MicStatus, type RelayStatus } from './AppModel';
import type { ResponseEnvelope } from './lld/ResponseEnvelope';
import { CloudOrb } from './orb/CloudOrb';
import { getOrBootPresenceBed, type PresenceAudioContext } from './presence/PresenceBoot';
import {
  ACTIVE_PRESENCE_REPEAT_DELAY_MS,
  PresenceEventQueue,
  activePresenceEvent,
  proactiveCheckInEvent,
} from './presence/PresenceEventQueue';
import { voiceEmotionForStimulation, type StimulationModel } from './presence/StimulationController';
import {
  bindPresenceToAppLifecycle,
  shouldRecoverOnSessionStateChange,
  type SessionEnvelopeUpdateSource,
} from './presence/PresenceLifecycle';
import {
  UNRECOGNIZED_SPEECH_TEXT,
  createT0FocusSession,
  type T0FocusSessionRuntime,
} from './runtime/T0FocusSession';
import { createRelayAtomizerPort, createStaticAtomizerPort } from './runtime/AtomizerPort';
import { createRelayConversationPort, createStaticConversationPort } from './runtime/ConversationPort';
import { classifyConversationControl } from './router/IntentClassifier';
import { createVoiceLoopController, type VoiceLoopRelay } from './runtime/VoiceLoopController';
import { RelayAudioPlayer } from './voice/RelayAudioPlayer';
import { RelayClient } from './voice/RelayClient';
import {
  RelayUnexpectedCloseError,
  createQueuedSocket,
  type WebSocketLike,
} from './voice/RelaySocket';
import { sendFishSpeech } from './voice/FishSpeech';
import { requestMicPermission, startCapture, stopCapture } from './voice/NativeMicPort';
import type { SpeechPort } from './voice/SpeechPort';
import { isVoiceFrame, pauseMsSince } from './voice/PauseClock';
import {
  BARGE_IN_YIELD_BUDGET_MS,
  ENDPOINT_INCOMPLETE_MAX_MS,
  VAD_POST_ENDPOINT_CLOSE_MS,
} from './voice/contracts';
import { createDevLogger } from './shared/DevLogger';

function createDefaultPresenceAudioContext(): PresenceAudioContext {
  const { AudioContext, AudioManager } = require('react-native-audio-api') as typeof import('react-native-audio-api');
  // THE mic fix. react-native-audio-api owns the process-wide AVAudioSession and re-asserts its
  // own category every time its engine starts or the route changes — by default `playback`, which
  // has no input capability at all. That silently strips the microphone route out from under
  // OrbMic no matter what OrbMic sets: live-observed as the bed logging
  // `configureAudioSession, category: AVAudioSessionCategoryPlayback` and OrbMic then reading an
  // input format of `channels=2 rate=0` (a half-dead route) seconds later.
  //
  // Declaring `playAndRecord` here — before the first AudioContext exists, so it is in force for
  // every subsequent re-assert — makes the ONE shared session record-capable for both engines.
  AudioManager.setAudioSessionOptions({
    iosCategory: 'playAndRecord',
    // Voice-chat mode enables iOS's communication audio processing/AEC. Without it, the mic
    // hears the orb's own Fish TTS through the simulator speaker and Fish STT can transcribe the
    // echo as unrelated language (for example, Hindi) instead of the user's English speech.
    iosMode: 'voiceChat',
    // `mixWithOthers` keeps the bed and the mic coexisting instead of interrupting each other;
    // `defaultToSpeaker` stops playAndRecord from routing output to the quiet earpiece.
    iosOptions: ['mixWithOthers', 'defaultToSpeaker', 'allowBluetoothHFP'],
  });
  return new AudioContext() as unknown as PresenceAudioContext;
}

/**
 * How this device reaches the dev machine's loopback. The Android emulator proxies the host at the
 * magic 10.0.2.2 alias; the iOS Simulator shares the host's network stack, so plain localhost is
 * correct there and 10.0.2.2 is simply unroutable — which silently black-holed every relay
 * connection on iOS (mic frames sent, no transcript ever returned, no error logged).
 */
function devHost(): string {
  return Platform.OS === 'android' ? '10.0.2.2' : 'localhost';
}

function relayHttpUrl(): string {
  const configured = typeof process !== 'undefined' ? process.env.ORB_RELAY_HTTP_URL : undefined;
  return configured?.trim() || `http://${devHost()}:8765`;
}

/** `backend/relay-rs`'s WebSocket endpoint; 8091 is `ORB_RELAY_ADDR`'s default port. */
function relayWsUrl(): string {
  const configured = typeof process !== 'undefined' ? process.env.ORB_RELAY_WS_URL : undefined;
  return configured?.trim() || `ws://${devHost()}:8091`;
}

export interface FocusOrbAppProps {
  readonly envelope?: ResponseEnvelope;
  readonly runtime?: T0FocusSessionRuntime;
  readonly createAudioContext?: () => PresenceAudioContext;
  readonly speaker?: SpeechPort;
  readonly autoStart?: boolean;
  readonly onStart?: () => void;
  readonly onPause?: () => void;
}

export default function FocusOrbApp({
  envelope = IDLE_ENVELOPE,
  runtime,
  createAudioContext,
  speaker,
  autoStart = true,
  onStart,
  onPause,
}: FocusOrbAppProps): React.ReactElement {
  // dev-logs/ pipeline (see docs/BUILD-DIGEST.md-adjacent tooling/dev-logs/watch.mjs): fires
  // best-effort POSTs at relay-py's /dev/log collector, __DEV__-gated so a release build never
  // makes this network call.
  const devLogger = useMemo(() => createDevLogger({ baseUrl: relayHttpUrl() }), []);
  // A relay session is one live app mount. Reusing `local-session` across launches caused the
  // backend's compressed conversation to resurrect stale context from an earlier run (for
  // example, a bathroom-door task) and present it as if it belonged to the current user turn.
  const sessionIdRef = useRef(`local-${Date.now()}`);
  const sessionId = sessionIdRef.current;
  // Which launch greeting plays — picked once per mount at this impure boundary (Date.now(), same
  // as `mountedAtMs` below) and injected into the runtime, which stays free of wall-clock/random.
  const greetingIndexRef = useRef(Date.now());
  const localRuntime = useMemo(
    () =>
      runtime ??
      createT0FocusSession({
        tenant_id: 't0',
        user_id: 'local-user',
        session_id: sessionId,
        task: 'Open the first small action',
      }, createRelayAtomizerPort({
        baseUrl: relayHttpUrl(),
        fallback: createStaticAtomizerPort({
          step_text: 'Open the first small action',
          est_min: 1,
          done_signal: 'the first small action is open',
        }),
        devLogger,
      }), greetingIndexRef.current, createRelayConversationPort({
        baseUrl: relayHttpUrl(),
        fallback: createStaticConversationPort(),
        devLogger,
      })),
    [runtime, devLogger, sessionId],
  );
  const [localEnvelope, setLocalEnvelope] = useState(envelope);
  const currentEnvelope = envelope === IDLE_ENVELOPE ? localEnvelope : envelope;
  // Real mic hardware status — distinct from `model.voiceState` (which is derived purely from
  // session FSM state and was never actually tied to whether the mic hardware is capturing).
  // 'active' only flips on the FIRST real audio frame received from NativeMicPort, not merely on
  // permission grant or engine start succeeding — those can both succeed while zero frames ever
  // arrive (the exact silent-failure mode a user can't distinguish from "just not listening yet").
  const [micStatus, setMicStatus] = useState<MicStatus>('pending');
  const [relayStatus, setRelayStatus] = useState<RelayStatus>('connecting');
  // Flips true only on relay-rs's `listening_confirmed` delivery receipt (RelayClient's
  // `onListeningConfirmed`), and back to false whenever the relay session is no longer live. The
  // orb's "listening" visual state must gate on this, not on `micStatus === 'active'` alone — the
  // mic hardware opening locally is not evidence the relay ever accepted the session.
  const [relayListeningConfirmed, setRelayListeningConfirmed] = useState(false);
  const [speechStatus, setSpeechStatus] = useState<'idle' | 'starting' | 'speaking' | 'error'>('idle');
  const micFrameSeenRef = useRef(false);
  const mountedAtMs = useRef(Date.now());
  const [stimulationElapsedMs, setStimulationElapsedMs] = useState(0);
  const model = screenModelFromEnvelope(
    currentEnvelope,
    { micStatus, relayStatus, speechStatus, listeningConfirmed: relayListeningConfirmed },
    stimulationElapsedMs,
  );
  const stimulationRef = useRef<StimulationModel>(model.stimulation);
  stimulationRef.current = model.stimulation;
  const presenceEventsRef = useRef(new PresenceEventQueue());
  const appActiveRef = useRef(true);
  const userTurnRef = useRef(false);
  const mutedRef = useRef(false);
  const sessionStateRef = useRef(currentEnvelope.session.state);
  sessionStateRef.current = currentEnvelope.session.state;
  // B6: which of the two envelope-update paths produced the current `localEnvelope` — a genuine
  // user action (a tap, or transcript -> intent through `createFocusOrbActions`), or a proactive,
  // self-triggered nudge (`checkIn`/`proactivePresence`, below). Defaults to 'user_action' since
  // that is the source for every setter except the two proactive branches, which flip it
  // immediately before calling `setLocalEnvelope` — see `shouldRecoverOnSessionStateChange`'s
  // header comment in PresenceLifecycle.ts for why this distinction exists.
  const envelopeUpdateSourceRef = useRef<SessionEnvelopeUpdateSource>('user_action');
  // Fish Audio is the only production speech path. A native AVSpeechSynthesizer fallback made
  // the audible voice change between Fish and iPhone voices whenever the relay was still
  // connecting or had closed. Keep an explicitly injected speaker available for tests/diagnostic
  // harnesses, but never create native TTS in the app.
  const speech = speaker;
  const warmup = useMemo(
    () =>
      runtime === undefined
        ? () =>
            requestSessionWarmup(relayHttpUrl(), {
              tenant_id: 't0',
              user_id: 'local-user',
              session_id: sessionId,
            })
        : undefined,
    [runtime, sessionId],
  );
  const elapsedSinceMountMs = () => Date.now() - mountedAtMs.current;
  // Same T0 identity the runtime above is constructed with — kept as one pair of constants so the
  // relay session and the focus-session runtime never drift apart.
  const tenantId = 't0';
  const relayClientRef = useRef<RelayClient | null>(null);
  const assistantSpeakingRef = useRef(false);
  const hasUserSpokenRef = useRef(false);
  const turnStartedAtMsRef = useRef<number | null>(null);
  /**
   * Wall-clock time of the most recent mic frame the VAD still counted as voice, or null when no
   * such frame has been seen in the current turn. This is the client's measurement of "how long
   * has the user been silent", and it is the input `SemanticEndpointer.decideEndpoint` needs to do
   * anything at all — with the hardcoded 0 that used to be passed here, every partial transcript
   * short-circuited on `pause_ms < eagerPauseMs` and the whole early-completion path (including
   * the bilingual continuation markers) was unreachable outside the 30s hard cap.
   *
   * `Date.now()` rather than `VadFrame.at_ms`: `at_ms` comes from the native mic module's own
   * timeline (`NativeMicPort.startCapture`, `event.atMs`), and the value derived from it here is
   * compared against `Date.now()` inside `onTranscript`, where no frame is in scope. Mixing the
   * two clocks would produce a difference with no meaning. `turnStartedAtMsRef` above already
   * uses the same wall clock for the same reason. The measurement itself lives in
   * `voice/PauseClock.ts`; this ref is only where it is carried between the two handlers.
   */
  const lastSpeechAtMsRef = useRef<number | null>(null);
  const relay = useMemo<VoiceLoopRelay>(
    () => ({
      get isClosed() {
        return relayClientRef.current?.isClosed ?? true;
      },
      startListening() {
        turnStartedAtMsRef.current = Date.now();
        relayClientRef.current?.startListening();
      },
      sendAudio(frame) {
        userTurnRef.current = true;
        hasUserSpokenRef.current = true;
        presenceEventsRef.current.cancelBy('user_speaks');
        relayClientRef.current?.sendAudio(frame);
      },
      endOfTurn() {
        userTurnRef.current = false;
        // The turn is over, so its silence measurement is too. Without this the NEXT turn's first
        // partial would be handed the gap since the PREVIOUS turn's last voice frame — seconds of
        // assistant speech and think time, during which the mic callback below is deliberately not
        // running — and would endpoint instantly on a one-word partial.
        lastSpeechAtMsRef.current = null;
        relayClientRef.current?.endOfTurn();
      },
      speak(text, voiceId, emotion) {
        // The assistant has the floor, so the user's silence measurement is over. `endOfTurn`
        // below covers the two local detectors; this covers the third way a turn can end, which
        // sends no end_of_turn at all: the STT provider endpointing on its own and pushing a
        // final transcript (`relay-rs` session.rs `push_audio` forwards `transcript.is_final`).
        lastSpeechAtMsRef.current = null;
        // Do not feed the assistant's own output back into STT. `RelayAudioPlayer.isPlaying()`
        // covers audio already scheduled after `speech_complete`; this synchronous flag covers
        // the gap between requesting speech and receiving the first relay chunk (including the
        // launch greeting).
        assistantSpeakingRef.current = true;
        userTurnRef.current = false;
        setSpeechStatus('starting');
        const graph = presenceBed.current;
        graph?.duckVoice();
        console.info(`focus-orb:speech-request elapsed_ms=${elapsedSinceMountMs()}`);
        const relayClient = relayClientRef.current;
        const voiceEmotion = voiceEmotionForStimulation(emotion, stimulationRef.current);
        // The relay's `speak` frame routes through `backend/relay-rs` to Fish Audio and the audio
        // comes back over `onSpeechAudio`/RelayAudioPlayer. Never fall back to AVSpeechSynthesizer:
        // it ignores the selected Fish voice/emotion and produced the user's reported male/female
        // voice switching and robotic output.
        try {
          if (!sendFishSpeech(relayClient, text, voiceId, voiceEmotion)) {
            // Tests and diagnostic harnesses may inject a speaker deliberately. This is not a
            // production fallback: production passes no speaker, so unavailable Fish audio stays
            // an explicit degraded state instead of switching to device TTS.
            if (speech) {
              devLogger.log('voice.explicit_speaker_override', {
                tenant_id: tenantId,
                session_id: sessionId,
                voice_id: voiceId,
                emotion: voiceEmotion,
                transport_state: relayClient?.connectionState ?? 'missing',
              }, 'warn');
              void Promise.resolve(speech.speak(text, voiceId, voiceEmotion)).finally(() => {
                assistantSpeakingRef.current = false;
                setSpeechStatus('idle');
                graph?.restoreVoice();
              });
              return;
            }
            assistantSpeakingRef.current = false;
            setSpeechStatus('error');
            setRelayStatus('degraded');
            presenceEventsRef.current.cancelBy('error');
            graph?.restoreVoice();
            devLogger.log('voice.fish_unavailable', {
              tenant_id: tenantId,
              session_id: sessionId,
              text: text.slice(0, 160),
              voice_id: voiceId,
              emotion: voiceEmotion,
              transport_state: relayClient?.connectionState ?? 'missing',
              native_tts_fallback: false,
            }, 'error');
            console.error('focus-orb:fish-unavailable native-tts-fallback-disabled');
          }
        } catch (error) {
          assistantSpeakingRef.current = false;
          setSpeechStatus('error');
          setRelayStatus('degraded');
          presenceEventsRef.current.cancelBy('error');
          graph?.restoreVoice();
          devLogger.log('voice.fish_send_failed', {
            tenant_id: tenantId,
            session_id: sessionId,
            text: text.slice(0, 160),
            voice_id: voiceId,
            emotion: voiceEmotion,
            transport_state: relayClient?.connectionState ?? 'missing',
            native_tts_fallback: false,
            error: error instanceof Error ? error.message : String(error),
          }, 'error');
          console.error(`focus-orb:fish-send-failed ${error instanceof Error ? error.message : String(error)}`);
        }
      },
      bargeIn() {
        relayClientRef.current?.bargeIn();
      },
      pause() {
        assistantSpeakingRef.current = false;
        lastSpeechAtMsRef.current = null;
        void speech?.stop?.();
        void stopCapture();
        relayClientRef.current?.pause();
      },
    }),
    [speech],
  );
  const voiceLoop = useMemo(() => createVoiceLoopController(localRuntime, relay), [localRuntime, relay]);
  const presenceBed = useRef<ReturnType<typeof getOrBootPresenceBed> | null>(null);
  const presenceRuntime = useRef<object | null>(null);
  // Set by `audioContextFactory` (below) the one time it runs, so the relay-audio player below can
  // reuse the exact same `AudioContext` instance the presence bed's `voiceGain` node belongs to —
  // nodes from two different Web Audio contexts cannot connect to each other.
  const presenceAudioContext = useRef<PresenceAudioContext | null>(null);
  const relayAudioPlayer = useRef<RelayAudioPlayer | null>(null);
  const started = useRef(false);
  const micGranted = useRef(false);
  const actions = useMemo(
    () =>
      createFocusOrbActions(localRuntime, setLocalEnvelope, {
        onStart: () => {
          presenceBed.current?.recover();
          onStart?.();
        },
      onPause: () => {
          presenceBed.current?.pause();
          presenceEventsRef.current.cancelBy('pause');
          onPause?.();
        },
        voiceLoop,
        // Route launch greetings and spoken failures through the same Fish relay as task replies.
        // There is no native TTS fallback; a failed relay is a visible/degraded error.
        speaker: relay,
        warmup,
      }),
    [localRuntime, onPause, onStart, relay, voiceLoop, warmup],
  );
  const audioContextFactory = useMemo(() => {
    const factory = createAudioContext ?? createDefaultPresenceAudioContext;
    return () => {
      const ctx = factory();
      presenceAudioContext.current = ctx;
      return ctx;
    };
  }, [createAudioContext]);

  // Real transport + mic: connect to `backend/relay-rs` and, once RECORD_AUDIO is granted, start
  // streaming native mic frames into the voice loop. A denied permission leaves the socket wired
  // (so TTS/orb text still flows) but never starts capture — logged, not thrown.
  useEffect(() => {
    if (typeof WebSocket === 'undefined') {
      // No global WebSocket (e.g. this component under test in a bare Node/vitest environment,
      // not a real RN runtime): leave the relay unwired rather than crash. TTS/orb still work.
      console.info('focus-orb:relay-websocket-unavailable');
      setRelayStatus('degraded');
      return undefined;
    }
    const openRelaySocket = (): WebSocketLike => {
      const raw = new WebSocket(relayWsUrl()) as unknown as WebSocketLike;
      // React Native/WebKit versions differ on the default binary WebSocket payload (`Blob` vs
      // `ArrayBuffer`). RelayAudioPlayer is deliberately synchronous so speech_complete cannot
      // race queued chunks; pin every initial/reconnect socket before a server frame can arrive.
      try {
        raw.binaryType = 'arraybuffer';
      } catch {
        // Older RN WebSocket shims expose no writable binaryType; the relay still reports the
        // missing binary frame through the protocol/error diagnostics rather than guessing bytes.
      }
      return raw;
    };
    const rawSocket = openRelaySocket();
    const socket = createQueuedSocket(rawSocket, { reconnectFactory: openRelaySocket });
    setRelayStatus(rawSocket.readyState === 1 ? 'ready' : 'connecting');
    const client = new RelayClient(socket, tenantId, sessionId, {
      onListeningConfirmed: () => {
        setRelayListeningConfirmed(true);
      },
      onTranscript: (text, isFinal) => {
        const now = Date.now();
        const turnStartedAtMs = turnStartedAtMsRef.current;
        const utteranceMs = turnStartedAtMs === null ? 0 : now - turnStartedAtMs;
        // Was hardcoded to 0, which made every early-completion tier in SemanticEndpointer
        // unreachable in the running app. See PauseClock.ts for what 0 still means (no voice frame
        // observed this turn) and why that stays the safe fallback.
        const pauseMs = pauseMsSince(lastSpeechAtMsRef.current, now);
        if (isFinal) userTurnRef.current = false;
        // "Wait, stop." / "hold on" must CUT THE AUDIO, now — before the model round trip, which
        // takes ~800ms and would otherwise let the orb keep talking over someone who just asked it
        // to stop. `classifyConversationControl` already recognised these (it stopped them being
        // answered with a task step), but nothing consumed the label: it routed to converse and the
        // orb replied "Okay, I'm stopping" while still playing the previous sentence. A classifier
        // whose result nothing acts on is a scaffold, which is this repo's most repeated defect.
        //
        // Deliberately only `stop`. `reject`/`divert`/`chat_invitation` are conversational moves
        // that belong in the reply, not interruptions of it — cutting audio on "no, not that" would
        // make ordinary disagreement feel like an error.
        if (isFinal && classifyConversationControl(text) === 'stop') {
          relayAudioPlayer.current?.handleClosing('user_pause');
          relayClientRef.current?.bargeIn();
          presenceEventsRef.current.cancelBy('user_speaks');
          assistantSpeakingRef.current = false;
          devLogger.log('voice.stop_honoured', {
            tenant_id: tenantId,
            session_id: sessionId,
            transcript: text.slice(0, 120),
            was_speaking: true,
          });
        }
        if (isFinal) {
          devLogger.log('stt.final_to_voice_loop', {
            session_id: sessionId,
            tenant_id: tenantId,
            transcript: text.slice(0, 500),
            text_chars: text.length,
            utterance_ms: utteranceMs,
            pause_ms: pauseMs,
          });
        }
        void voiceLoop.handleTranscript(text, isFinal, pauseMs, Math.min(utteranceMs, ENDPOINT_INCOMPLETE_MAX_MS))
          .then((events) => {
            // A semantic endpoint on a partial is the whole point of measuring `pauseMs`, and it
            // is otherwise invisible: it produces no envelope and no speech of its own (the answer
            // comes from the final transcript the relay returns). Log it so a real device run can
            // show the endpoint actually firing, and how far ahead of VAD's 800ms it fired.
            if (!isFinal && events.some((event) => event.kind === 'relay_end_of_turn')) {
              devLogger.log('voice.semantic_endpoint', {
                session_id: sessionId,
                tenant_id: tenantId,
                transcript: text.slice(0, 500),
                pause_ms: pauseMs,
                utterance_ms: utteranceMs,
                saved_vs_vad_ms: Math.max(0, VAD_POST_ENDPOINT_CLOSE_MS - pauseMs),
              });
            }
            if (!isFinal) return;
            const envelopes = events.filter((event): event is Extract<typeof event, { kind: 'envelope' }> => event.kind === 'envelope');
            const response = envelopes.at(-1)?.envelope;
            devLogger.log('voice.response', {
              session_id: sessionId,
              tenant_id: tenantId,
              transcript: text.slice(0, 500),
              response_text: response?.speech?.text?.slice(0, 500) ?? '',
              response_source: response?.meta.source ?? 'none',
              session_state: response?.session.state ?? 'none',
              event_count: events.length,
            }, response ? 'info' : 'warn');
          })
          .catch((error) => {
            devLogger.log('voice.response_error', {
              session_id: sessionId,
              tenant_id: tenantId,
              transcript: text.slice(0, 500),
              error: error instanceof Error ? error.message : String(error),
            }, 'error');
          });
      },
      onSpeechStarting: () => {
        // Local `speak()` already sets this synchronously when this app requested the utterance;
        // this is belt-and-braces so barge-in detection reads "orb is speaking" off the relay's
        // own confirmation too, not only local intent, in case that flag was ever cleared (e.g. by
        // an error handler) between the request and this confirmation arriving.
        assistantSpeakingRef.current = true;
        setRelayStatus('ready');
        setSpeechStatus('speaking');
        console.info('focus-orb:relay-speech-starting');
      },
      onSpeechAudio: (chunk) => {
        // Relay-synthesized Fish TTS audio: schedule it onto the presence bed's voice branch via
        // RelayAudioPlayer. This is the only production speech playback path.
        relayAudioPlayer.current?.enqueueChunk(chunk);
      },
      onTransportError: (error) => {
        assistantSpeakingRef.current = false;
        setSpeechStatus('error');
        presenceEventsRef.current.cancelBy('error');
        if (error instanceof RelayUnexpectedCloseError && error.reconnectScheduled) {
          setRelayStatus('connecting');
          relayAudioPlayer.current?.handleClosing('provider_failure');
          devLogger.log('relay.transport_reconnecting', {
            tenant_id: tenantId,
            session_id: sessionId,
            endpoint: relayWsUrl(),
            error: error.message,
          }, 'warn');
          console.warn(`focus-orb:relay-reconnecting endpoint=${relayWsUrl()} ${error.message}`);
          return;
        }
        setRelayStatus('degraded');
        devLogger.log('relay.transport_error', {
          tenant_id: tenantId,
          session_id: sessionId,
          endpoint: relayWsUrl(),
          state: error.state,
          error: error.message,
        }, 'error');
        console.error(`focus-orb:relay-transport-error endpoint=${relayWsUrl()} ${error.message}`);
        void stopCapture();
      },
      onTransportReconnected: () => {
        setRelayStatus('ready');
        setSpeechStatus('idle');
        console.info('focus-orb:relay-reconnected attempt=1');
      },
      onTransportClosed: (event) => {
        // RelaySocket withholds this logical close while its one reconnect is pending. Reaching
        // here means the initial connection failed or that bounded attempt was exhausted, so the
        // native mic must stop instead of capturing indefinitely into a terminal transport.
        assistantSpeakingRef.current = false;
        setSpeechStatus('error');
        setRelayStatus('closed');
        presenceEventsRef.current.cancelBy('error');
        devLogger.log('relay.transport_closed', {
          tenant_id: tenantId,
          session_id: sessionId,
          endpoint: relayWsUrl(),
          code: event?.code,
          reason: event?.reason,
          was_clean: event?.wasClean,
        }, 'warn');
        void stopCapture();
      },
      onSpeechComplete: () => {
        console.info('focus-orb:relay-speech-complete');
        setSpeechStatus('idle');
        assistantSpeakingRef.current = false;
        relayAudioPlayer.current?.completeTurn();
        presenceBed.current?.restoreVoice();
        if (hasUserSpokenRef.current && appActiveRef.current && sessionStateRef.current !== 'SESSION_DONE') {
          presenceEventsRef.current.schedule(
            activePresenceEvent(Date.now(), ACTIVE_PRESENCE_REPEAT_DELAY_MS),
          );
        }
      },
      onAudioBedFallback: () => {
        // §5 audio-bed invariant: relay-rs kept the session open (Phase::Degraded) instead of
        // closing it, so this handler must NOT do what onClosing/onProtocolError do for a real
        // session end — no presence cancellation, no stopCapture(), no transport close (RelayClient
        // does not close the socket for this frame either). The in-flight speech attempt will never
        // get a `speech_complete` now, so treat it the same way as `onClosing('provider_failure')`
        // for the pieces that only need "this turn's TTS is over": stop tracking it as still
        // speaking, restore the bed to nominal, and reuse RelayAudioPlayer's existing
        // provider_failure apology (fires once; also unblocks any suppressed turnEpoch).
        assistantSpeakingRef.current = false;
        setSpeechStatus('idle');
        setRelayStatus('degraded');
        presenceBed.current?.restoreVoice();
        console.info('focus-orb:relay-audio-bed-fallback');
        devLogger.log('relay.audio_bed_fallback', { tenant_id: tenantId, session_id: sessionId }, 'warn');
        relayAudioPlayer.current?.handleClosing('provider_failure');
      },
      onClosing: (reason) => {
        assistantSpeakingRef.current = false;
        setSpeechStatus('error');
        setRelayStatus(reason === 'provider_failure' ? 'degraded' : 'closed');
        // The relay is no longer listening to anything; a stale receipt from before this closing
        // must not keep the orb showing "listening" against a session that is over.
        setRelayListeningConfirmed(false);
        presenceEventsRef.current.cancelBy('error');
        console.info(`focus-orb:relay-closing reason=${reason}`);
        relayAudioPlayer.current?.handleClosing(reason);
        void stopCapture();
      },
      onProtocolError: (raw) => {
        setSpeechStatus('error');
        setRelayStatus('degraded');
        presenceEventsRef.current.cancelBy('error');
        console.error(`focus-orb:relay-protocol-error ${JSON.stringify(raw)}`);
      },
    }, devLogger);
    relayClientRef.current = client;

    void (async () => {
      const granted = await requestMicPermission();
      micGranted.current = granted;
      if (!granted) {
        console.info('focus-orb:mic-permission-denied');
        setMicStatus('denied');
        return;
      }
      try {
        await startCapture((frame, audio) => {
          const orbIsSpeaking = assistantSpeakingRef.current || (relayAudioPlayer.current?.isPlaying() ?? false);
          // Observe every frame, including silent-orb frames: the latter reset the coordinator so
          // a later reply gets its own cancellation edge instead of inheriting the prior turn's
          // one-shot latch.
          const bargeInEvents = voiceLoop.handleBargeInObservation({
            at_ms: frame.at_ms,
            speech_probability: frame.speech_probability,
            orb_is_speaking: orbIsSpeaking,
            is_echo_residue: false,
          });
          if (orbIsSpeaking) {
            // iOS `voiceChat` configures communication processing/AEC before capture starts. Keep
            // raw frames out of STT until the orb yields so its own TTS cannot self-loop.
            if (!bargeInEvents.some((event) => event.kind === 'barge_in_yield')) return;

            relayAudioPlayer.current?.handleClosing('user_pause');
            presenceEventsRef.current.cancelBy('user_speaks');
            assistantSpeakingRef.current = false;
            setSpeechStatus('idle');
            presenceBed.current?.restoreVoice();
            devLogger.log('voice.barge_in_cancel_dispatched', {
              tenant_id: tenantId,
              session_id: sessionId,
              budget_ms: BARGE_IN_YIELD_BUDGET_MS,
              vad_source: frame.vad_source ?? 'unknown',
            });
          }
          if (!micFrameSeenRef.current) {
            micFrameSeenRef.current = true;
            setMicStatus('active');
          }
          // Mirrors `VADGate`'s own "is this still voice?" test frame for frame (census-checked
          // in PauseClock.test.ts), so the semantic endpointer and the audio gate measure the same
          // silence and differ only in how long they wait for it: 150ms against VAD's 800ms.
          if (isVoiceFrame(frame.speech_probability)) {
            lastSpeechAtMsRef.current = Date.now();
          }
          void voiceLoop.handleAudioFrame(frame, audio);
        });
      } catch (error) {
        console.error(`focus-orb:mic-start-error ${error instanceof Error ? error.message : String(error)}`);
        setMicStatus('error');
        // startCapture registers its native-frame listener before awaiting module.start(). If
        // start() rejects after partially acquiring the AVAudioSession/audio engine (plausible —
        // NativeMicPort's own comments document AVAudioSession category races), leaving that
        // listener registered and never calling module.stop() would orphan the native resource
        // exactly like the SIGTERM/recordVideo bug: the thing that should release on failure
        // doesn't, because nothing calls the stop path.
        void stopCapture();
      }
    })();

    return () => {
      void stopCapture();
      if (!client.isClosed) client.pause();
      relayClientRef.current = null;
    };
    // tenantId/sessionId are stable module-level-equivalent constants for this T0 slice; the
    // effect only needs to re-run when the voice loop it feeds changes.
  }, [voiceLoop, devLogger]);

  useEffect(() => {
    if (!autoStart || started.current) return;
    started.current = true;
    let cancelled = false;
    void actions.start().then(() => {
      if (!cancelled) presenceEventsRef.current.schedule(activePresenceEvent(Date.now()));
    });
    return () => {
      cancelled = true;
      void speech?.stop?.();
    };
  }, [actions, autoStart, speech]);

  useEffect(() => {
    if (presenceRuntime.current !== localRuntime) {
      presenceRuntime.current = localRuntime;
      const bootStartedAtMs = Date.now();
      presenceBed.current = getOrBootPresenceBed(localRuntime, audioContextFactory);
      // `audioContextFactory` runs synchronously inside `getOrBootPresenceBed` on first boot, so
      // `presenceAudioContext.current` is populated by the time we get here.
      if (presenceAudioContext.current) {
        relayAudioPlayer.current = new RelayAudioPlayer(
          presenceAudioContext.current,
          presenceBed.current.voiceGain,
          undefined,
          speech,
          devLogger,
        );
      }
      console.info(
        `focus-orb:presence-ready elapsed_ms=${elapsedSinceMountMs()} boot_ms=${Date.now() - bootStartedAtMs}`,
      );
    }
  }, [audioContextFactory, devLogger, localRuntime, speech]);

  useEffect(() => {
    if (currentEnvelope.session.state === 'INTERRUPTED') {
      presenceBed.current?.pause();
      return;
    }
    // B6 (delayed silent resume): a proactive check-in/presence nudge is not a user action even
    // though it changes `session.state` — it must not be the thing that un-pauses the bed. Read
    // the source once per envelope change, then reset to the default so the NEXT change (which
    // may be a real user action) isn't wrongly suppressed too.
    const source = envelopeUpdateSourceRef.current;
    envelopeUpdateSourceRef.current = 'user_action';
    if (!shouldRecoverOnSessionStateChange(currentEnvelope.session.state, source)) return;
    presenceBed.current?.recover();
    // Color-morph (brown<->pink) is NOT driven from session-envelope state here. Per
    // `presence/ColorMorph.ts`'s own header and blueprint 02 §7, "thinking" is scoped to the
    // crew-forming/computation window, not to STEP_PRESENT (which AppModel.ts's own `voiceState()`
    // maps to 'speaking' — an already-computed step being presented, the opposite moment).
    // `runtime/VoiceLoopController.ts`'s `schedulePresenceMorph` already owns that real window
    // (turn-end-signaled -> 'thinking', next user speech -> 'idle') and calls `morphColor` directly
    // on this same presence bed graph. A second driver keyed off session state here previously
    // forced the bed pink for the entire STEP_PRESENT/speaking duration — exactly the "permanently
    // brighter after a step is presented" case ColorMorph.ts's header says must never happen.
  }, [currentEnvelope.session.state]);

  useEffect(() => {
    const subscription = bindPresenceToAppLifecycle(AppState, () => presenceBed.current, (state) => {
      appActiveRef.current = state === 'active';
      if (state === 'active') {
        presenceEventsRef.current.schedule(proactiveCheckInEvent(Date.now()));
      } else {
        presenceEventsRef.current.cancelBy('background');
      }
    });
    return () => subscription.remove();
  }, []);

  // Proactive re-engagement: the orb checks in on its own, not only when spoken to (the
  // companion behavior the app promises — see cognitive/Policy.ts's CHECK_IN guards). One
  // foreground timer drains a cancellable queue; `T0FocusSession.checkIn` itself stays pure
  // (`now` is the only clock input), and the queue's guards make silence the default.
  useEffect(() => {
    const queue = presenceEventsRef.current;
    queue.schedule(proactiveCheckInEvent(Date.now()));
    const intervalId = setInterval(() => {
      const now = Date.now();
      setStimulationElapsedMs(now - mountedAtMs.current);
      const dueEvents = queue.drain(now, {
        appActive: appActiveRef.current,
        userTurn: userTurnRef.current || assistantSpeakingRef.current,
        muted: mutedRef.current,
        goalActive: sessionStateRef.current !== 'IDLE_PRESENT' && sessionStateRef.current !== 'SESSION_DONE',
        hyperfocus: false,
      });
      for (const event of dueEvents) {
        if (event.kind === 'gentle_presence') {
          void localRuntime.proactivePresence().then((nextEnvelope) => {
            if (!nextEnvelope) return;
            sessionStateRef.current = nextEnvelope.session.state;
            envelopeUpdateSourceRef.current = 'proactive';
            setLocalEnvelope(nextEnvelope);
            if (appActiveRef.current && nextEnvelope.speech) {
              relay.speak(nextEnvelope.speech.text, nextEnvelope.speech.voice.voice_id, nextEnvelope.orb.emotion);
            }
          }).catch((error) => {
            console.error(`focus-orb:active-presence-error ${error instanceof Error ? error.message : String(error)}`);
          }).finally(() => {
            const state = sessionStateRef.current;
            if (appActiveRef.current && state !== 'IDLE_PRESENT' && state !== 'SESSION_DONE') {
              queue.schedule(activePresenceEvent(Date.now(), ACTIVE_PRESENCE_REPEAT_DELAY_MS));
            }
          });
          continue;
        }
        if (event.kind !== 'stall_assist') continue;
        void localRuntime.checkIn(now).then((nextEnvelope) => {
          if (!nextEnvelope) return;
          sessionStateRef.current = nextEnvelope.session.state;
          envelopeUpdateSourceRef.current = 'proactive';
          setLocalEnvelope(nextEnvelope);
          if (appActiveRef.current && nextEnvelope.speech) {
            relay.speak(nextEnvelope.speech.text, nextEnvelope.speech.voice.voice_id, nextEnvelope.orb.emotion);
          }
        }).catch((error) => {
          console.error(`focus-orb:presence-event-error ${error instanceof Error ? error.message : String(error)}`);
        }).finally(() => {
          const state = sessionStateRef.current;
          if (appActiveRef.current && state !== 'IDLE_PRESENT' && state !== 'SESSION_DONE') {
            queue.schedule(proactiveCheckInEvent(Date.now()));
          }
        });
      }
    }, 1_000);
    return () => clearInterval(intervalId);
  }, [localRuntime, relay]);

  // The mic keeps streaming frames continuously once capture starts — VAD decides what's
  // meaningful, not the FSM state — so "really listening" is just "mic active and not paused".
  const micListening = micStatus === 'active' && currentEnvelope.session.state !== 'INTERRUPTED';
  // Written for the person holding the phone, not the person reading the logs. A driven run on the
  // simulator (no mic hardware) showed the previous copy — "Mic error — check console" — as the
  // ONLY thing on screen: a developer instruction, addressed to a user who has no console, with no
  // way forward. The orb is a voice product, so losing the mic is the one failure a user is most
  // likely to hit (permission denied, mic in use by another app, no hardware), and it must read as
  // a recoverable state rather than a crash report.
  //
  // Deliberately NOT fixed here: with no mic there is still no way to talk to the orb at all — the
  // whole session is gated on capture and the app has no typed-input path (zero `TextInput` in
  // apps/mobile/src). Honest copy is not a working fallback; see docs/UX-HUMAN-REVIEW.md.
  // --- Typed input: the path that exists so losing the mic is not the end of the app -------------
  //
  // A driven simulator run (evidence/ui-drive-070313/) showed the whole product dead-ending on
  // `OrbMic: unusable_input_format`: a static orb, one status line, no affordance, and **zero
  // requests reaching the relay across four launches**. The same thing happens to a real user who
  // denies mic permission or whose mic is held by another app — for a voice product, the single
  // most likely failure there is.
  //
  // It appears ONLY when the mic is actually unusable. An earlier version of this also offered a
  // permanent "Type instead" affordance; the owner's direction while testing was *"it should detect
  // automatically as soon as mic is active"* — i.e. when the mic works the orb must need nothing
  // from the user, and a standing button is exactly the visible clutter `AppSurface.test.tsx`
  // ("renders only the cloud orb on a dark background") exists to prevent. So: no control while the
  // mic is fine, and a way through the moment it is not.
  //
  // Routed through `voiceLoop.handleTranscript(text, isFinal=true, ...)` — the exact seam a final
  // STT transcript uses — so there is ONE conversation path, not two. A parallel path is how `mode`
  // came to be computed everywhere and transmitted nowhere, silently breaking three test paths at
  // once; this must never become the second one.
  const micUnusable = micStatus === 'denied' || micStatus === 'error';
  const [typedDraft, setTypedDraft] = useState('');
  const [typedSending, setTypedSending] = useState(false);
  const [lastReplyText, setLastReplyText] = useState<string | null>(null);
  const [typedError, setTypedError] = useState<string | null>(null);
  const showTyped = micUnusable;

  const submitTyped = React.useCallback(async () => {
    const text = typedDraft.trim();
    // Empty submit is a no-op, never a turn: `completeTurn` already treats an empty transcript as
    // "carried no speech" and stays silent, and unasked-for speech is what the 0ms-silence rule
    // forbids. Cheaper and clearer to stop here.
    if (text.length === 0 || typedSending) return;
    setTypedSending(true);
    setTypedError(null);
    devLogger.log('input.typed_turn', {
      tenant_id: tenantId,
      session_id: sessionId,
      text_chars: text.length,
      mic_status: micStatus,
      opened_because: 'mic_unusable',
    });
    try {
      // pauseMs/utteranceMs = 0 is the documented "no voice frame observed this turn" value
      // (PauseClock.ts), which is exactly true for typed text — not a placeholder.
      const events = await voiceLoop.handleTranscript(text, true, 0, 0);
      const spoken = [...events].reverse().find((event) => event.kind === 'relay_speak');
      const spokenText =
        spoken && 'text' in spoken && typeof spoken.text === 'string' ? spoken.text : null;
      // `T0FocusSession` can reject an utterance locally — contentless, non-ASCII in WORKING, or a
      // phonetic-Devanagari ASR artefact — and answer with UNRECOGNIZED_SPEECH_TEXT without ever
      // calling the relay. That path logs NOTHING today, so from the outside a local rejection and a
      // real model reply look identical: the user just hears "say it again".
      //
      // That blindness is why a reported "the orb can't understand me" took a whole session to pin
      // down, and it misled me once here too: a rejected turn's message was still on screen when I
      // screenshotted a DIFFERENT turn that had actually succeeded, and I reported a discarded-reply
      // bug that did not exist. One log line makes the two distinguishable at a glance.
      //
      // Diagnostics only — no content. Character CLASSES are what identify the branch (the
      // simulator's EN/HI keyboard silently prepends predictive text and can inject Devanagari,
      // which is a real way to trip the non-ASCII guard with input that looked fine while typing).
      if (spokenText !== null && spokenText === UNRECOGNIZED_SPEECH_TEXT) {
        devLogger.log(
          'input.locally_rejected',
          {
            tenant_id: tenantId,
            session_id: sessionId,
            text_chars: text.length,
            has_letter_or_digit: /[\p{L}\p{N}]/u.test(text),
            has_non_ascii: [...text].some((c) => c.charCodeAt(0) > 127),
            has_devanagari: [...text].some((c) => c.charCodeAt(0) >= 0x0900 && c.charCodeAt(0) <= 0x097f),
            reached_relay: false,
          },
          'warn',
        );
      }
      setLastReplyText(spokenText);
      setTypedDraft('');
    } catch (error) {
      // Never swallow this: with no mic, a silent failure here leaves the user with a dead app and
      // no way to know why — the exact state this whole path exists to remove.
      const detail = error instanceof Error ? error.message : String(error);
      setTypedError('That didn’t send. Check the connection and try again.');
      devLogger.log(
        'input.typed_turn_failed',
        { tenant_id: tenantId, session_id: sessionId, detail: detail.slice(0, 200) },
        'error',
      );
    } finally {
      setTypedSending(false);
    }
  }, [devLogger, micStatus, micUnusable, sessionId, tenantId, typedDraft, typedSending, voiceLoop]);

  const micStatusText =
    micStatus === 'denied'
      ? 'I need microphone access to hear you — you can turn it on in Settings.'
      : micStatus === 'error'
        ? 'I can’t reach your microphone right now. It may be in use by another app.'
        : micStatus === 'pending'
          ? 'Getting ready to listen…'
          : micListening
            ? 'Listening'
            : null;

  return (
    <View style={styles.screen} testID="focus-orb-screen">
      <CloudOrb
        state={model.voiceState}
        volume={model.orbVolume}
        color={model.orbStyle.backgroundColor}
        opacity={model.orbStyle.opacity}
        scale={model.orbStyle.scale}
        visual={model.orbVisual}
        micListening={micListening}
      />
      {/*
        Deliberate divergence from this component's original "no visible text" design (see the
        file docstring above) — per the user's explicit 2026-08-06 /goal: they could not tell
        whether the mic was actually capturing, which the orb's animation alone did not
        communicate reliably. This label is minimal and only ever reflects real mic hardware
        status (`micStatus`), never a guess.
      */}
      {micStatusText && (
        <Text testID="focus-orb-mic-status" style={styles.micStatusText}>
          {micStatusText}
        </Text>
      )}
      {/*
        The orb's reply, shown as text. Normally the orb speaks and shows nothing — but when someone
        is typing they may also be somewhere they cannot listen, so the answer has to be readable.
        Only rendered once a typed turn has produced one, so the voice-only experience is unchanged.
      */}
      {showTyped && lastReplyText && (
        <Text testID="focus-orb-reply-text" style={styles.replyText}>
          {lastReplyText}
        </Text>
      )}
      {showTyped && (
        <View style={styles.typedRow}>
          <TextInput
            testID="focus-orb-typed-input"
            accessibilityLabel="Type a message to the orb"
            style={styles.typedInput}
            value={typedDraft}
            onChangeText={setTypedDraft}
            onSubmitEditing={() => void submitTyped()}
            placeholder="Type instead…"
            placeholderTextColor="rgba(255,255,255,0.35)"
            editable={!typedSending}
            returnKeyType="send"
            multiline={false}
            // Deliberately NOT autoFocus: opening a keyboard over the orb the moment the app
            // launches takes the choice away from the user, and on the mic-unusable path they have
            // not asked to type yet — they have only been told the mic is unavailable.
            autoFocus={false}
            autoCorrect
            blurOnSubmit={false}
          />
          <Pressable
            testID="focus-orb-typed-send"
            accessibilityRole="button"
            accessibilityLabel="Send message"
            accessibilityState={{ disabled: typedSending || typedDraft.trim().length === 0 }}
            disabled={typedSending || typedDraft.trim().length === 0}
            onPress={() => void submitTyped()}
            style={({ pressed }) => [
              styles.typedSend,
              (typedSending || typedDraft.trim().length === 0) && styles.typedSendDisabled,
              pressed && styles.typedSendPressed,
            ]}
          >
            <Text style={styles.typedSendLabel}>{typedSending ? '…' : 'Send'}</Text>
          </Pressable>
        </View>
      )}
      {typedError && (
        <Text testID="focus-orb-typed-error" style={styles.typedErrorText}>
          {typedError}
        </Text>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 24,
    backgroundColor: '#05070d',
  },
  micStatusText: {
    marginTop: 16,
    color: 'rgba(255,255,255,0.55)',
    fontSize: 13,
    letterSpacing: 0.5,
  },
  replyText: {
    marginTop: 20,
    color: 'rgba(255,255,255,0.88)',
    fontSize: 16,
    lineHeight: 23,
    textAlign: 'center',
    maxWidth: 320,
  },
  typedRow: {
    marginTop: 22,
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    width: '100%',
    maxWidth: 340,
  },
  typedInput: {
    flex: 1,
    minHeight: 44, // 44pt is the platform minimum touch target; smaller is not tappable.
    paddingHorizontal: 14,
    borderRadius: 22,
    backgroundColor: 'rgba(255,255,255,0.08)',
    color: '#ffffff',
    fontSize: 15,
  },
  typedSend: {
    minHeight: 44,
    minWidth: 64,
    paddingHorizontal: 16,
    borderRadius: 22,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: 'rgba(255,255,255,0.16)',
  },
  typedSendDisabled: { opacity: 0.4 },
  typedSendPressed: { backgroundColor: 'rgba(255,255,255,0.28)' },
  typedSendLabel: { color: '#ffffff', fontSize: 15, fontWeight: '600' },
  typedErrorText: {
    marginTop: 10,
    color: 'rgba(255,170,150,0.9)',
    fontSize: 13,
    textAlign: 'center',
    maxWidth: 320,
  },
});
