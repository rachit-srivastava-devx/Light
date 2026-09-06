/**
 * Client-side wiring for relay-rs's `audio_bed_fallback` frame (session.rs's `Phase::Degraded` +
 * `Action::SendFallback`, protocol.rs's `ServerFrame::AudioBedFallback`). Before this frame type
 * existed on the wire, a provider failure closed the socket via a `closing` frame with
 * `reason: "provider_failure"`; relay-rs now keeps the session open and announces the outage with
 * this new frame instead. The client must recognise it — falling through to `onProtocolError`
 * (the pre-existing "unknown type" path) is worse than the old behaviour: it would cancel presence
 * on an event that is specifically supposed to keep presence alive (§5, audio-bed invariant).
 */
import { describe, expect, it, vi } from 'vitest';

import { RelayClient, type RelayHandlers } from './RelayClient';
import type { SocketLike } from './RelaySocket';

function fakeSocket(): SocketLike {
  return {
    readyState: 1,
    send() {},
    close() {},
    onmessage: null,
    onopen: null,
    onerror: null,
    onclose: null,
  };
}

function handlers(): RelayHandlers & {
  readonly onClosing: ReturnType<typeof vi.fn>;
  readonly onProtocolError: ReturnType<typeof vi.fn>;
  readonly onAudioBedFallback: ReturnType<typeof vi.fn>;
} {
  return {
    onTranscript: vi.fn(),
    onSpeechStarting: vi.fn(),
    onSpeechComplete: vi.fn(),
    onClosing: vi.fn(),
    onProtocolError: vi.fn(),
    onAudioBedFallback: vi.fn(),
  };
}

describe('RelayClient: audio_bed_fallback frame', () => {
  it('is dispatched to its own handler, not onProtocolError', () => {
    const socket = fakeSocket();
    const h = handlers();
    const client = new RelayClient(socket, 'tenant-1', 'session-1', h);

    socket.onmessage?.({
      data: JSON.stringify({ type: 'audio_bed_fallback', tenant_id: 'tenant-1', session_id: 'session-1' }),
    });

    expect(h.onAudioBedFallback).toHaveBeenCalledTimes(1);
    expect(h.onProtocolError).not.toHaveBeenCalled();
    // Unlike `closing`, this frame does not end the session: the client must not react as though
    // the transport were torn down.
    expect(client.isClosed).toBe(false);
  });

  it('does not call onClosing for this frame (it is a distinct event from a provider-failure close)', () => {
    const socket = fakeSocket();
    const h = handlers();
    new RelayClient(socket, 'tenant-1', 'session-1', h);

    socket.onmessage?.({
      data: JSON.stringify({ type: 'audio_bed_fallback', tenant_id: 'tenant-1', session_id: 'session-1' }),
    });

    expect(h.onClosing).not.toHaveBeenCalled();
  });
});
