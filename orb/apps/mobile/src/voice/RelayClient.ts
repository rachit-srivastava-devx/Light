/**
 * WebSocket client to `backend/relay-rs`. The only network surface the client has for voice —
 * it never talks to an STT/TTS provider directly (the audio-plane mirror of C9).
 */

import { noopDevLogger, truncate, type DevLogger } from '../shared/DevLogger';
import type { ClientFrame, ServerFrame } from './contracts';
import {
  RelaySocket,
  type RelaySendMetadata,
  type RelaySocketDiagnostic,
  type RelaySocketOptions,
  type RelaySocketState,
  RelayTransportError,
  type SocketCloseEvent,
  type SocketLike,
} from './RelaySocket';

export { RelaySocket, RelayTransportError } from './RelaySocket';
export type {
  RelaySocketDiagnostic,
  RelaySocketOptions,
  RelaySocketState,
  SocketCloseEvent,
  SocketLike,
} from './RelaySocket';

export interface RelayHandlers {
  /** Delivery receipt for `start_listening` — the relay accepted the session and opened the STT
   * stream. This, not local mic-open, is what a "listening" UI state must gate on. */
  readonly onListeningConfirmed?: () => void;
  readonly onTranscript: (text: string, isFinal: boolean) => void;
  readonly onSpeechStarting: () => void;
  readonly onSpeechAudio?: (chunk: ArrayBuffer) => void;
  readonly onSpeechComplete: () => void;
  /** `user_pause` and `provider_failure` remain distinct presence-plane decisions. */
  readonly onClosing: (reason: 'user_pause' | 'provider_failure') => void;
  /**
   * The voice provider failed but relay-rs kept the session open (§5 audio-bed invariant; mirrors
   * `ServerFrame::AudioBedFallback` in `backend/relay-rs/src/protocol.rs`). Distinct from
   * `onClosing`: the transport is NOT closed here, so the handler must not cancel presence or stop
   * capture the way a real session end does.
   */
  readonly onAudioBedFallback?: () => void;
  /** A frame that did not parse. Reported, never guessed at. */
  readonly onProtocolError: (raw: unknown) => void;
  /** Native transport failure, separate from a relay `closing` protocol frame. */
  readonly onTransportError?: (error: RelayTransportError) => void;
  /** Native transport close, including refused connection or remote drop. */
  readonly onTransportClosed?: (event?: SocketCloseEvent) => void;
  /** The one bounded reconnect opened and the relay session was re-established. */
  readonly onTransportReconnected?: () => void;
  /** Structured evidence for diagnosing whether mic frames reached native send(). */
  readonly onTransportDiagnostic?: (diagnostic: RelaySocketDiagnostic) => void;
}

interface AudioBatchStats {
  frames: number;
  bytes: number;
  first_frame_seq: number;
  last_frame_seq: number;
}

/** Mic frames are still captured/uploaded as 16 kHz PCM; TTS playback uses SpeechAudio's 24 kHz target. */
const MIC_SAMPLE_RATE_HZ = 16_000;

export class RelayClient {
  private readonly transport: RelaySocket;
  private readonly tenantId: string;
  private readonly sessionId: string;
  private readonly handlers: RelayHandlers;
  private readonly devLogger: DevLogger;
  private audioFrameSequence = 0;
  private listeningStarted = false;
  private audioBatch: AudioBatchStats | null = null;
  /** Prevent a dead relay from producing one error log per mic frame. */
  private blockedSendState: RelaySocketState | null = null;
  /** Fallback ordering chain for RN shims that still deliver binary WebSocket frames as Blob. */
  private binaryReceiveQueue: Promise<void> = Promise.resolve();

  constructor(
    socket: SocketLike,
    tenantId: string,
    sessionId: string,
    handlers: RelayHandlers,
    devLoggerOrOptions?: DevLogger | RelaySocketOptions,
  );
  /** Compatibility overload for the original `(socket, sessionId, handlers)` API. */
  constructor(
    socket: SocketLike,
    sessionId: string,
    handlers: RelayHandlers,
    devLoggerOrOptions?: DevLogger | RelaySocketOptions,
  );
  constructor(
    socket: SocketLike,
    tenantOrSessionId: string,
    sessionOrHandlers: string | RelayHandlers,
    handlersOrLogger?: RelayHandlers | DevLogger | RelaySocketOptions,
    maybeLoggerOrOptions?: DevLogger | RelaySocketOptions,
  ) {
    const legacy = typeof sessionOrHandlers !== 'string';
    this.tenantId = legacy ? '' : tenantOrSessionId;
    this.sessionId = legacy ? tenantOrSessionId : sessionOrHandlers;
    this.handlers = (legacy ? sessionOrHandlers : handlersOrLogger) as RelayHandlers;

    const supplied = legacy ? handlersOrLogger : maybeLoggerOrOptions;
    const options = toSocketOptions(supplied);
    this.devLogger = options.logger ?? noopDevLogger;

    const diagnostic = (entry: RelaySocketDiagnostic): void => {
      const enriched = {
        ...entry,
        tenant_id: this.tenantId,
        session_id: this.sessionId,
      };
      if (entry.channel === 'audio' && entry.event === 'send_accepted') {
        this.recordAcceptedAudio(enriched);
      } else if (entry.event === 'reconnected') {
        this.handleReconnected();
        this.devLogger.log('relay_client.reconnected', enriched, 'info');
      } else if (
        entry.channel === 'audio' &&
        (entry.event === 'send_queued' || entry.event === 'send_failed')
      ) {
        // Queued mic frames are represented by the accepted batch once the socket opens. A failed
        // transport already emits its single error/close event; logging every subsequent frame
        // would recreate the same 10 Hz scroll during connection failure.
      } else {
        this.devLogger.log(
          `relay_client.${entry.event}`,
          enriched,
          entry.error ? 'error' : 'info',
        );
      }
      options.onDiagnostic?.(enriched);
      this.handlers.onTransportDiagnostic?.(enriched);
    };

    this.transport =
      socket instanceof RelaySocket
        ? socket
        : new RelaySocket(socket, { ...options, logger: noopDevLogger, onDiagnostic: diagnostic });
    this.transport.onDiagnostic = diagnostic;

    this.transport.onmessage = (event) => this.receive(event.data);
    this.transport.onerror = (error) => {
      this.handlers.onTransportError?.(
        error instanceof RelayTransportError
          ? error
          : new RelayTransportError('Relay socket error', 'failed', error),
      );
    };
    this.transport.onclose = (event) => {
      this.flushAudioBatch('socket_closed');
      this.handlers.onTransportClosed?.(event);
    };
  }

  get connectionState(): RelaySocketState {
    return this.transport.state;
  }

  /** True once the relay closed the session, the socket failed, or the socket dropped. */
  get isClosed(): boolean {
    return this.transport.state === 'failed' || this.transport.state === 'closed';
  }

  private send(frame: ClientFrame): boolean {
    // A deliberate local pause is idempotent. A failed transport is terminal for this client: do
    // not let the mic loop turn one connection failure into an exception on every following frame.
    if (this.transport.state === 'failed' || this.transport.state === 'closed') {
      this.reportBlockedSend(frame.type);
      return false;
    }
    const metadata: RelaySendMetadata = { channel: 'control', frame_type: frame.type };
    try {
      this.transport.send(JSON.stringify(frame), metadata);
      return true;
    } catch (error) {
      this.reportBlockedSend(frame.type, error);
      return false;
    }
  }

  startListening(): void {
    if (this.listeningStarted) return;
    this.listeningStarted = this.send({
      type: 'start_listening',
      tenant_id: this.tenantId,
      session_id: this.sessionId,
    });
  }

  endOfTurn(): void {
    this.flushAudioBatch('end_of_turn');
    this.send({ type: 'end_of_turn', tenant_id: this.tenantId, session_id: this.sessionId });
  }

  speak(text: string, voiceId: string, emotion: string): void {
    // relay-rs accepts `speak` only after a session enters listening state. This matters for the
    // launch greeting: it is intentionally sent before mic permission and before the normal voice
    // loop starts, so establish the session first and let RelaySocket preserve this order while
    // the WebSocket is still connecting.
    this.startListening();
    this.devLogger.log('relay_client.speak', {
      session_id: this.sessionId,
      text: truncate(text),
      voice_id: voiceId,
      emotion,
    });
    this.send({
      type: 'speak',
      tenant_id: this.tenantId,
      session_id: this.sessionId,
      text,
      voice_id: voiceId,
      emotion,
    });
  }

  bargeIn(): void {
    this.send({ type: 'barge_in', tenant_id: this.tenantId, session_id: this.sessionId });
  }

  /** Sends pause when possible, then closes immediately so the mic is released without a round trip. */
  pause(): void {
    this.flushAudioBatch('pause');
    if (this.transport.state === 'connecting' || this.transport.state === 'open') {
      this.send({ type: 'pause', tenant_id: this.tenantId, session_id: this.sessionId });
    }
    this.transport.close();
  }

  /** Mic audio stays binary; accepted sends emit sequence/byte diagnostics without raw audio. */
  sendAudio(frame: ArrayBuffer): void {
    if (this.transport.state === 'failed' || this.transport.state === 'closed') {
      this.reportBlockedSend('audio');
      return;
    }
    const frameSequence = ++this.audioFrameSequence;
    try {
      this.transport.send(frame, {
        channel: 'audio',
        frame_seq: frameSequence,
        bytes: frame.byteLength,
      });
    } catch (error) {
      this.reportBlockedSend('audio', error);
    }
  }

  private reportBlockedSend(frameType: string, error?: unknown): void {
    const state = this.transport.state;
    if (this.blockedSendState === state) return;
    this.blockedSendState = state;
    this.devLogger.log('relay_client.send_blocked', {
      tenant_id: this.tenantId,
      session_id: this.sessionId,
      frame_type: frameType,
      transport_state: state,
      error: error instanceof Error ? error.message : error ? String(error) : undefined,
    }, 'error');
  }

  private handleReconnected(): void {
    this.blockedSendState = null;
    let sessionReady = true;
    if (this.listeningStarted) {
      // A WebSocket reconnect creates a new relay-rs session task. Reassert the protocol state
      // before RelaySocket flushes any mic frames queued during the short backoff.
      sessionReady = this.send({
        type: 'start_listening',
        tenant_id: this.tenantId,
        session_id: this.sessionId,
      });
    }
    if (sessionReady) this.handlers.onTransportReconnected?.();
  }

  private recordAcceptedAudio(entry: RelaySocketDiagnostic & Record<string, unknown>): void {
    const frameBytes = typeof entry.bytes === 'number' ? entry.bytes : 0;
    const frameSequence = typeof entry.frame_seq === 'number' ? entry.frame_seq : this.audioFrameSequence;
    const stats = this.audioBatch ?? {
      frames: 0,
      bytes: 0,
      first_frame_seq: frameSequence,
      last_frame_seq: frameSequence,
    };
    stats.frames += 1;
    stats.bytes += frameBytes;
    stats.last_frame_seq = frameSequence;
    this.audioBatch = stats;
  }

  private flushAudioBatch(reason: 'end_of_turn' | 'pause' | 'socket_closed'): void {
    const stats = this.audioBatch;
    if (!stats) return;
    this.audioBatch = null;
    this.devLogger.log('relay_client.audio_batch', {
      tenant_id: this.tenantId,
      session_id: this.sessionId,
      reason,
      frames: stats.frames,
      bytes: stats.bytes,
      first_frame_seq: stats.first_frame_seq,
      last_frame_seq: stats.last_frame_seq,
      duration_ms: Math.round((stats.bytes / 2 / MIC_SAMPLE_RATE_HZ) * 1_000),
    }, 'info');
  }

  private receive(data: unknown): void {
    if (data instanceof ArrayBuffer) return this.handlers.onSpeechAudio?.(data);
    if (isBlobLike(data)) {
      this.binaryReceiveQueue = this.binaryReceiveQueue
        .then(async () => this.handlers.onSpeechAudio?.(await data.arrayBuffer()))
        .catch((error: unknown) => {
          this.devLogger.log(
            'relay_client.binary_decode_failed',
            { session_id: this.sessionId, error: error instanceof Error ? error.message : String(error) },
            'error',
          );
        });
      return;
    }
    if (typeof data !== 'string') return this.handlers.onProtocolError(data);

    let frame: ServerFrame;
    try {
      frame = JSON.parse(data) as ServerFrame;
    } catch {
      this.devLogger.log(
        'relay_client.unparseable',
        { session_id: this.sessionId, raw: truncate(data) },
        'error',
      );
      return this.handlers.onProtocolError(data);
    }

    switch (frame.type) {
      case 'listening_confirmed':
        this.devLogger.log('relay_client.listening_confirmed', {
          session_id: this.sessionId,
        });
        return this.handlers.onListeningConfirmed?.();
      case 'transcript':
        this.devLogger.log('stt.transcript.received', {
          provider: 'relay-rs',
          session_id: this.sessionId,
          is_final: frame.is_final,
          transcript_kind: frame.is_final ? 'final' : 'partial',
          text: truncate(frame.text),
          text_chars: frame.text.length,
        });
        return this.handlers.onTranscript(frame.text, frame.is_final);
      case 'speech_starting':
        return this.handlers.onSpeechStarting();
      case 'speech_complete':
        // A Blob conversion is asynchronous. Wait for every preceding binary frame so
        // completeTurn() cannot reset the scheduler before the final audio chunk is enqueued.
        void this.binaryReceiveQueue.then(() => this.handlers.onSpeechComplete());
        return;
      case 'audio_bed_fallback':
        this.devLogger.log(
          'relay_client.audio_bed_fallback',
          { session_id: this.sessionId },
          'error',
        );
        return this.handlers.onAudioBedFallback?.();
      case 'closing':
        this.devLogger.log(
          'relay_client.closing',
          { session_id: this.sessionId, reason: frame.reason },
          frame.reason === 'provider_failure' ? 'error' : 'info',
        );
        try {
          this.handlers.onClosing(frame.reason);
        } finally {
          this.transport.close();
        }
        return;
      default:
        this.devLogger.log(
          'relay_client.protocol_error',
          { session_id: this.sessionId, frame: truncate(JSON.stringify(frame)) },
          'error',
        );
        return this.handlers.onProtocolError(frame);
    }
  }
}

interface BlobLike {
  readonly arrayBuffer: () => Promise<ArrayBuffer>;
}

function isBlobLike(value: unknown): value is BlobLike {
  return typeof value === 'object' && value !== null && typeof (value as BlobLike).arrayBuffer === 'function';
}

function isDevLogger(value: unknown): value is DevLogger {
  return typeof (value as DevLogger | undefined)?.log === 'function';
}

function toSocketOptions(value: unknown): RelaySocketOptions {
  if (isDevLogger(value)) return { logger: value };
  return (value as RelaySocketOptions | undefined) ?? {};
}
