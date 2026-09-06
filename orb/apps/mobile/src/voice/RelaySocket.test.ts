import { describe, expect, it, vi } from 'vitest';

import { RelayClient, type RelayHandlers } from './RelayClient';
import {
  RelayOpenTimeoutError,
  RelaySocket,
  RelayUnexpectedCloseError,
  type SocketLike,
} from './RelaySocket';

function fakeSocket(readyState: number): SocketLike & {
  readonly sent: (string | ArrayBuffer)[];
  closeCalls: number;
  failSends: boolean;
} {
  return {
    readyState,
    sent: [],
    closeCalls: 0,
    failSends: false,
    send(data) {
      if (this.failSends) throw new Error('write refused');
      this.sent.push(data);
    },
    close() {
      this.closeCalls += 1;
    },
    onmessage: null,
    onopen: null,
    onerror: null,
    onclose: null,
  };
}

function handlers(): RelayHandlers & {
  readonly onTransportError: ReturnType<typeof vi.fn>;
  readonly onTransportClosed: ReturnType<typeof vi.fn>;
  readonly onTransportReconnected: ReturnType<typeof vi.fn>;
  readonly onTransportDiagnostic: ReturnType<typeof vi.fn>;
} {
  return {
    onTranscript: vi.fn(),
    onSpeechStarting: vi.fn(),
    onSpeechAudio: vi.fn(),
    onSpeechComplete: vi.fn(),
    onClosing: vi.fn(),
    onProtocolError: vi.fn(),
    onTransportError: vi.fn(),
    onTransportClosed: vi.fn(),
    onTransportReconnected: vi.fn(),
    onTransportDiagnostic: vi.fn(),
  };
}

describe('RelaySocket', () => {
  it('fails a refused connection and propagates both error and close', () => {
    const socket = fakeSocket(0);
    const h = handlers();
    const client = new RelayClient(socket, 'tenant-1', 'session-1', h, { openTimeoutMs: 1_000 });

    client.startListening();
    socket.onclose?.({ code: 1006, reason: 'connection refused', wasClean: false });

    expect(client.connectionState).toBe('failed');
    expect(h.onTransportError).toHaveBeenCalledWith(expect.objectContaining({ state: 'failed' }));
    expect(h.onTransportClosed).toHaveBeenCalledWith(
      expect.objectContaining({ code: 1006, reason: 'connection refused' }),
    );
    expect(() => client.sendAudio(new ArrayBuffer(8))).not.toThrow();
  });

  it('drops later sends safely and logs one blocked-state diagnostic', () => {
    const socket = fakeSocket(0);
    const h = handlers();
    const log = vi.fn();
    const client = new RelayClient(socket, 'tenant-1', 'session-1', h, { log });

    client.startListening();
    socket.onclose?.({ code: 1006, reason: 'connection refused', wasClean: false });
    client.sendAudio(new ArrayBuffer(8));
    client.sendAudio(new ArrayBuffer(8));
    client.speak('hello', 'voice', 'warm');

    expect(log.mock.calls.filter(([event]) => event === 'relay_client.send_blocked')).toHaveLength(1);
  });

  it('queues control and mic frames until open, then records accepted mic evidence', () => {
    const socket = fakeSocket(0);
    const h = handlers();
    const client = new RelayClient(socket, 'tenant-1', 'session-1', h, { openTimeoutMs: 1_000 });
    const audio = new ArrayBuffer(320);

    client.startListening();
    client.sendAudio(audio);
    expect(socket.sent).toHaveLength(0);

    socket.onopen?.();

    expect(socket.sent).toHaveLength(2);
    expect(socket.sent[1]).toBe(audio);
    expect(h.onTransportDiagnostic).toHaveBeenCalledWith(
      expect.objectContaining({
        event: 'send_accepted',
        channel: 'audio',
        frame_seq: 1,
        bytes: 320,
        tenant_id: 'tenant-1',
        session_id: 'session-1',
      }),
    );
  });

  it('aggregates accepted mic frames into one turn summary instead of one log per frame', () => {
    const socket = fakeSocket(1);
    const h = handlers();
    const log = vi.fn();
    const client = new RelayClient(socket, 'tenant-1', 'session-1', h, { log });

    client.startListening();
    client.sendAudio(new ArrayBuffer(640));
    client.sendAudio(new ArrayBuffer(640));
    client.endOfTurn();

    expect(log).toHaveBeenCalledWith(
      'relay_client.audio_batch',
      expect.objectContaining({ frames: 2, bytes: 1_280, duration_ms: 40, reason: 'end_of_turn' }),
      'info',
    );
    expect(log.mock.calls.filter(([event, fields]) => event === 'relay_client.send_accepted' && fields?.channel === 'audio')).toHaveLength(0);
  });

  it('surfaces a typed timeout error when the socket never opens within the deadline', () => {
    vi.useFakeTimers();
    try {
      const socket = fakeSocket(0);
      const onError = vi.fn();
      const transport = new RelaySocket(socket, { openTimeoutMs: 25, onDiagnostic: vi.fn() });
      transport.onerror = onError;

      vi.advanceTimersByTime(25);

      expect(transport.state).toBe('failed');
      expect(socket.closeCalls).toBe(1);
      expect(onError).toHaveBeenCalledWith(expect.any(RelayOpenTimeoutError));
      expect(onError).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'Relay socket open timeout after 25ms', timeoutMs: 25 }),
      );
      expect(() => transport.send('late')).toThrow(/failed state/);
    } finally {
      vi.useRealTimers();
    }
  });

  it('makes exactly one reconnect attempt with backoff after a mid-session close', () => {
    vi.useFakeTimers();
    try {
      const first = fakeSocket(1);
      const retry = fakeSocket(0);
      const reconnectFactory = vi.fn(() => retry);
      const onError = vi.fn();
      const onClose = vi.fn();
      const transport = new RelaySocket(first, {
        openTimeoutMs: 1_000,
        reconnectDelayMs: 50,
        reconnectFactory,
      });
      transport.onerror = onError;
      transport.onclose = onClose;

      first.onclose?.({ code: 1006, reason: 'network dropped', wasClean: false });

      expect(onError).toHaveBeenCalledWith(expect.any(RelayUnexpectedCloseError));
      expect(reconnectFactory).not.toHaveBeenCalled();
      expect(onClose).not.toHaveBeenCalled();

      vi.advanceTimersByTime(49);
      expect(reconnectFactory).not.toHaveBeenCalled();
      vi.advanceTimersByTime(1);
      expect(reconnectFactory).toHaveBeenCalledTimes(1);
      expect(transport.state).toBe('connecting');

      retry.onopen?.();
      expect(transport.state).toBe('open');

      retry.onclose?.({ code: 1006, reason: 'dropped again', wasClean: false });
      vi.advanceTimersByTime(5_000);

      expect(reconnectFactory).toHaveBeenCalledTimes(1);
      expect(transport.state).toBe('closed');
      expect(onClose).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('re-establishes listening before flushing audio queued during reconnect', () => {
    vi.useFakeTimers();
    try {
      const first = fakeSocket(1);
      const retry = fakeSocket(0);
      const reconnectFactory = vi.fn(() => retry);
      const h = handlers();
      const client = new RelayClient(first, 'tenant-1', 'session-1', h, {
        reconnectDelayMs: 25,
        reconnectFactory,
      });
      const audio = new ArrayBuffer(320);

      client.startListening();
      first.onclose?.({ code: 1006, reason: 'network dropped', wasClean: false });
      client.sendAudio(audio);
      vi.advanceTimersByTime(25);
      retry.onopen?.();

      expect(retry.sent).toHaveLength(2);
      expect(JSON.parse(retry.sent[0] as string)).toMatchObject({
        type: 'start_listening',
        tenant_id: 'tenant-1',
        session_id: 'session-1',
      });
      expect(retry.sent[1]).toBe(audio);
      expect(h.onTransportReconnected).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('propagates an open-socket error and the subsequent remote close separately', () => {
    const socket = fakeSocket(1);
    const h = handlers();
    const log = vi.fn();
    const client = new RelayClient(socket, 'tenant-1', 'session-1', h, { log });

    socket.onerror?.(new Error('relay crashed'));
    socket.onclose?.({ code: 1011, reason: 'relay crashed', wasClean: false });

    expect(client.connectionState).toBe('failed');
    expect(h.onTransportError).toHaveBeenCalledWith(
      expect.objectContaining({ message: expect.stringContaining('Relay socket error') }),
    );
    expect(h.onTransportClosed).toHaveBeenCalledWith(
      expect.objectContaining({ code: 1011, reason: 'relay crashed' }),
    );
    expect(log).toHaveBeenCalledWith(
      'relay_client.error',
      expect.objectContaining({
        tenant_id: 'tenant-1',
        session_id: 'session-1',
        error: expect.stringContaining('relay crashed'),
      }),
      'error',
    );
  });

  it('propagates a native send failure and never attempts a later send', () => {
    const socket = fakeSocket(1);
    socket.failSends = true;
    const h = handlers();
    const client = new RelayClient(socket, 'tenant-1', 'session-1', h);

    expect(() => client.startListening()).not.toThrow();
    expect(client.connectionState).toBe('failed');
    expect(h.onTransportError).toHaveBeenCalled();
    expect(() => client.speak('hello', 'voice', 'warm')).not.toThrow();
    expect(socket.closeCalls).toBe(1);
  });
});
