/**
 * WebSocket lifecycle wrapper for the realtime relay.
 *
 * Frames sent while the native socket is CONNECTING are queued and flushed in order. A previously
 * open session gets one delayed reconnect; terminal failures surface typed errors instead of
 * looking successful to the mic loop.
 */

import { noopDevLogger, type DevLogger } from '../shared/DevLogger';

export type RelaySocketState = 'connecting' | 'open' | 'failed' | 'closed';

export interface SocketCloseEvent {
  readonly code?: number;
  readonly reason?: string;
  readonly wasClean?: boolean;
}

export interface SocketLike {
  send(data: string | ArrayBuffer): void;
  close(): void;
  onmessage: ((event: { data: unknown }) => void) | null;
  onopen?: (() => void) | null;
  onerror?: ((event: unknown) => void) | null;
  onclose: ((event?: SocketCloseEvent) => void) | null;
  readonly readyState?: number;
}

/** Raw WebSocket shape accepted by `createQueuedSocket`, retained for the app's existing API. */
export interface WebSocketLike extends SocketLike {
  readonly readyState: number;
  /** Standard WebSocket binary mode; RN defaults vary, so the app pins ArrayBuffer explicitly. */
  binaryType?: 'blob' | 'arraybuffer';
  onopen: (() => void) | null;
  onerror: ((event?: unknown) => void) | null;
  onclose: ((event?: SocketCloseEvent) => void) | null;
}

export interface RelaySendMetadata {
  readonly channel?: 'control' | 'audio';
  readonly frame_seq?: number;
  readonly frame_type?: string;
  readonly bytes?: number;
}

export interface RelaySocketDiagnostic extends RelaySendMetadata {
  readonly event:
    | 'connecting'
    | 'open'
    | 'send_queued'
    | 'send_accepted'
    | 'send_failed'
    | 'error'
    | 'open_timeout'
    | 'reconnect_scheduled'
    | 'reconnect_attempt'
    | 'reconnected'
    | 'closed';
  readonly state: RelaySocketState;
  readonly queued_frames: number;
  readonly error?: string;
  readonly reconnect_attempt?: number;
  readonly backoff_ms?: number;
}

export interface RelaySocketOptions {
  readonly openTimeoutMs?: number;
  /** Creates the replacement native socket for the single allowed reconnect attempt. */
  readonly reconnectFactory?: () => SocketLike;
  readonly reconnectDelayMs?: number;
  readonly logger?: DevLogger;
  readonly onDiagnostic?: (diagnostic: RelaySocketDiagnostic) => void;
}

export class RelayTransportError extends Error {
  readonly state: RelaySocketState;
  readonly cause: unknown;

  constructor(message: string, state: RelaySocketState, cause?: unknown) {
    super(message);
    this.name = 'RelayTransportError';
    this.state = state;
    this.cause = cause;
  }
}

/** The native socket stayed CONNECTING past its explicit opening deadline. */
export class RelayOpenTimeoutError extends RelayTransportError {
  readonly timeoutMs: number;

  constructor(timeoutMs: number) {
    super(`Relay socket open timeout after ${timeoutMs}ms`, 'failed');
    this.name = 'RelayOpenTimeoutError';
    this.timeoutMs = timeoutMs;
  }
}

/** A previously-open voice session lost its transport without an explicit local pause. */
export class RelayUnexpectedCloseError extends RelayTransportError {
  readonly reconnectScheduled: boolean;

  constructor(
    event: SocketCloseEvent | undefined,
    reconnectScheduled: boolean,
    cause: unknown = event,
  ) {
    const causeMessage = cause instanceof Error ? cause.message : undefined;
    const detail = event?.reason ?? causeMessage;
    super(`Relay socket closed unexpectedly${detail ? `: ${detail}` : ''}`, 'closed', cause);
    this.name = 'RelayUnexpectedCloseError';
    this.reconnectScheduled = reconnectScheduled;
  }
}

interface PendingSend {
  readonly data: string | ArrayBuffer;
  readonly metadata?: RelaySendMetadata;
}

const DEFAULT_OPEN_TIMEOUT_MS = 10_000;
const DEFAULT_RECONNECT_DELAY_MS = 250;
const WS_CONNECTING = 0;
const WS_OPEN = 1;

export class RelaySocket implements SocketLike {
  public onmessage: ((event: { data: unknown }) => void) | null = null;
  public onopen: (() => void) | null = null;
  public onerror: ((event: unknown) => void) | null = null;
  public onclose: ((event?: SocketCloseEvent) => void) | null = null;
  public onDiagnostic: ((diagnostic: RelaySocketDiagnostic) => void) | undefined;

  private stateValue: RelaySocketState;
  private readonly queue: PendingSend[] = [];
  private readonly logger: DevLogger;
  private readonly options: RelaySocketOptions;
  private socket: SocketLike;
  private openTimer: ReturnType<typeof setTimeout> | undefined;
  private reconnectTimer: ReturnType<typeof setTimeout> | undefined;
  private closeNotified = false;
  private openedAtLeastOnce = false;
  private reconnectAttempted = false;
  private intentionallyClosed = false;

  constructor(socket: SocketLike, options: RelaySocketOptions = {}) {
    this.socket = socket;
    this.options = options;
    this.logger = options.logger ?? noopDevLogger;
    this.onDiagnostic = options.onDiagnostic;
    this.stateValue = this.initialState(socket.readyState);
    this.openedAtLeastOnce = this.stateValue === 'open';
    this.bindSocket(socket);

    if (this.stateValue === 'connecting') {
      this.emit({ event: 'connecting', state: this.stateValue });
      this.openTimer = setTimeout(
        () => this.handleOpenTimeout(),
        options.openTimeoutMs ?? DEFAULT_OPEN_TIMEOUT_MS,
      );
    } else if (this.stateValue === 'open') {
      this.emit({ event: 'open', state: this.stateValue });
    }
  }

  get state(): RelaySocketState {
    return this.stateValue;
  }

  send(data: string | ArrayBuffer, metadata?: RelaySendMetadata): void {
    if (this.stateValue === 'connecting') {
      this.queue.push({ data, metadata });
      this.emit({ event: 'send_queued', state: this.stateValue, ...metadata });
      return;
    }

    if (this.stateValue !== 'open') {
      throw this.rejectSend(metadata);
    }

    this.sendNow(data, metadata);
  }

  close(): void {
    if (this.stateValue === 'closed') return;
    this.intentionallyClosed = true;
    this.clearOpenTimer();
    this.clearReconnectTimer();
    this.queue.length = 0;
    this.stateValue = 'closed';
    this.notifyClose();
    this.unbindSocket(this.socket);
    try {
      this.socket.close();
    } catch (error) {
      this.logger.log('relay_socket.close_failed', { error: describeError(error) }, 'error');
    }
  }

  private initialState(readyState: number | undefined): RelaySocketState {
    // Older test doubles and injected sockets had no readyState and were immediately writable.
    if (readyState === undefined || readyState === WS_OPEN) return 'open';
    if (readyState === WS_CONNECTING) return 'connecting';
    return 'closed';
  }

  private handleOpen(): void {
    if (this.stateValue !== 'connecting') return;
    this.clearOpenTimer();
    this.stateValue = 'open';
    this.openedAtLeastOnce = true;
    this.emit({
      event: this.reconnectAttempted ? 'reconnected' : 'open',
      state: this.stateValue,
      ...(this.reconnectAttempted ? { reconnect_attempt: 1 } : {}),
    });
    this.onopen?.();

    const queued = this.queue.splice(0);
    for (const pending of queued) {
      try {
        this.sendNow(pending.data, pending.metadata);
      } catch {
        break;
      }
    }
  }

  private handleError(event: unknown): void {
    if (this.stateValue === 'closed') return;
    if (this.canReconnect()) {
      this.scheduleReconnect(undefined, true, event);
      return;
    }
    this.markFailed(this.asTransportError('Relay socket error', event), 'error');
  }

  private handleClose(event?: SocketCloseEvent): void {
    this.clearOpenTimer();
    if (this.canReconnect()) {
      this.scheduleReconnect(event, false, event);
      return;
    }
    if (this.stateValue === 'connecting') {
      this.markFailed(
        new RelayTransportError('Relay socket closed before opening', 'failed', event),
        'error',
        false,
      );
    } else if (this.stateValue === 'open') {
      this.surfaceError(new RelayUnexpectedCloseError(event, false));
      this.stateValue = 'closed';
      this.queue.length = 0;
    }
    this.notifyClose(event);
  }

  private handleOpenTimeout(): void {
    if (this.stateValue !== 'connecting') return;
    const timeoutMs = this.options.openTimeoutMs ?? DEFAULT_OPEN_TIMEOUT_MS;
    const error = new RelayOpenTimeoutError(timeoutMs);
    this.emit({ event: 'open_timeout', state: this.stateValue, error: error.message });
    this.markFailed(error, 'error');
  }

  private canReconnect(): boolean {
    return (
      !this.intentionallyClosed &&
      this.openedAtLeastOnce &&
      !this.reconnectAttempted &&
      this.options.reconnectFactory !== undefined &&
      this.stateValue === 'open'
    );
  }

  private scheduleReconnect(
    event: SocketCloseEvent | undefined,
    closeSocket: boolean,
    cause: unknown,
  ): void {
    const failedSocket = this.socket;
    this.clearOpenTimer();
    this.unbindSocket(failedSocket);
    this.stateValue = 'connecting';

    const error = new RelayUnexpectedCloseError(event, true, cause);
    this.surfaceError(error);
    const backoffMs = this.options.reconnectDelayMs ?? DEFAULT_RECONNECT_DELAY_MS;
    this.emit({
      event: 'reconnect_scheduled',
      state: this.stateValue,
      reconnect_attempt: 1,
      backoff_ms: backoffMs,
      error: error.message,
    });

    if (closeSocket) {
      try {
        failedSocket.close();
      } catch (closeError) {
        this.logger.log('relay_socket.close_failed', { error: describeError(closeError) }, 'error');
      }
    }

    this.reconnectTimer = setTimeout(() => this.attemptReconnect(), backoffMs);
  }

  private attemptReconnect(): void {
    this.reconnectTimer = undefined;
    if (this.intentionallyClosed || this.stateValue !== 'connecting') return;
    this.reconnectAttempted = true;
    this.emit({ event: 'reconnect_attempt', state: this.stateValue, reconnect_attempt: 1 });

    let replacement: SocketLike;
    try {
      replacement = this.options.reconnectFactory!();
    } catch (cause) {
      this.markFailed(this.asTransportError('Relay socket reconnect failed', cause), 'error', false);
      this.notifyClose();
      return;
    }

    this.socket = replacement;
    const replacementState = this.initialState(replacement.readyState);
    this.stateValue = replacementState;
    this.bindSocket(replacement);
    if (replacementState === 'connecting') {
      this.openTimer = setTimeout(
        () => this.handleOpenTimeout(),
        this.options.openTimeoutMs ?? DEFAULT_OPEN_TIMEOUT_MS,
      );
    } else if (replacementState === 'open') {
      // Route an already-open test/native socket through the same reconnected callbacks.
      this.stateValue = 'connecting';
      this.handleOpen();
    } else {
      this.markFailed(
        new RelayTransportError('Relay reconnect socket was already closed', 'failed'),
        'error',
        false,
      );
      this.notifyClose();
    }
  }

  private sendNow(data: string | ArrayBuffer, metadata?: RelaySendMetadata): void {
    try {
      this.socket.send(data);
      this.emit({ event: 'send_accepted', state: this.stateValue, ...metadata });
    } catch (cause) {
      const error = this.asTransportError('Relay socket send failed', cause);
      this.emit({ event: 'send_failed', state: this.stateValue, error: error.message, ...metadata });
      this.markFailed(error, 'error');
      throw error;
    }
  }

  private rejectSend(metadata?: RelaySendMetadata): RelayTransportError {
    const error = new RelayTransportError(
      `Cannot send on relay socket in ${this.stateValue} state`,
      this.stateValue,
    );
    this.emit({ event: 'send_failed', state: this.stateValue, error: error.message, ...metadata });
    return error;
  }

  private markFailed(
    error: RelayTransportError,
    event: 'error' | 'send_failed',
    closeSocket = true,
  ): void {
    if (this.stateValue === 'failed' || this.stateValue === 'closed') return;
    this.clearOpenTimer();
    this.queue.length = 0;
    this.stateValue = 'failed';
    this.emit({ event, state: this.stateValue, error: error.message });
    this.onerror?.(error);
    if (closeSocket) {
      try {
        this.socket.close();
      } catch (closeError) {
        this.logger.log('relay_socket.close_failed', { error: describeError(closeError) }, 'error');
      }
    }
  }

  private notifyClose(event?: SocketCloseEvent): void {
    if (this.closeNotified) return;
    this.closeNotified = true;
    this.emit({ event: 'closed', state: this.stateValue });
    this.onclose?.(event);
  }

  private clearOpenTimer(): void {
    if (this.openTimer === undefined) return;
    clearTimeout(this.openTimer);
    this.openTimer = undefined;
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer === undefined) return;
    clearTimeout(this.reconnectTimer);
    this.reconnectTimer = undefined;
  }

  private bindSocket(socket: SocketLike): void {
    socket.onmessage = (event) => {
      if (this.socket === socket) this.onmessage?.(event);
    };
    socket.onopen = () => {
      if (this.socket === socket) this.handleOpen();
    };
    socket.onerror = (event) => {
      if (this.socket === socket) this.handleError(event);
    };
    socket.onclose = (event) => {
      if (this.socket === socket) this.handleClose(event);
    };
  }

  private unbindSocket(socket: SocketLike): void {
    socket.onmessage = null;
    socket.onopen = null;
    socket.onerror = null;
    socket.onclose = null;
  }

  private surfaceError(error: RelayTransportError): void {
    this.emit({ event: 'error', state: this.stateValue, error: error.message });
    this.onerror?.(error);
  }

  private asTransportError(message: string, cause: unknown): RelayTransportError {
    return cause instanceof RelayTransportError
      ? cause
      : new RelayTransportError(`${message}: ${describeError(cause)}`, 'failed', cause);
  }

  private emit(partial: Omit<RelaySocketDiagnostic, 'queued_frames'>): void {
    const diagnostic: RelaySocketDiagnostic = { ...partial, queued_frames: this.queue.length };
    this.logger.log(
      `relay_socket.${diagnostic.event}`,
      { ...diagnostic },
      diagnostic.error ? 'error' : 'info',
    );
    this.onDiagnostic?.(diagnostic);
  }
}

/** Existing app wiring entry point; now backed by the strict lifecycle wrapper. */
export function createQueuedSocket(ws: WebSocketLike, options: RelaySocketOptions = {}): RelaySocket {
  return new RelaySocket(ws, options);
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
