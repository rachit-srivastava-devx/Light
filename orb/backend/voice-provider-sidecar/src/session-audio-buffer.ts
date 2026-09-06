/**
 * In-memory per-session audio accumulator for real STT backends. Streaming-capable adapters can
 * snapshot the accumulated prefix for an early partial while retaining the full utterance for the
 * authoritative `endTurn` transcript.
 *
 * Deliberately process-local state — the same trade-off `backend/relay-rs`'s socket session
 * already makes for one connection's audio. A session's buffer is dropped once `endTurn` reads
 * it, but a client can push audio and then vanish (app crash, network drop) without ever calling
 * `endTurn`. Without eviction that would grow this Map by real audio bytes for the life of the
 * process — the same "the stop path never actually released it" shape as the SIGTERM/recordVideo
 * bug. A lazy sweep on each push bounds memory to sessions active within the idle TTL, without
 * needing a background timer (consistent with this sidecar's zero-added-infra T0 design).
 */
export class SessionAudioBuffer {
  private readonly frames = new Map<string, Uint8Array[]>();
  private readonly byteLengths = new Map<string, number>();
  private readonly partialClaimed = new Set<string>();
  private readonly lastPushedAtMs = new Map<string, number>();
  private readonly idleTtlMs: number;
  private readonly now: () => number;

  constructor(options: { idleTtlMs?: number; now?: () => number } = {}) {
    this.idleTtlMs = options.idleTtlMs ?? 60 * 60 * 1000;
    this.now = options.now ?? Date.now;
  }

  push(sessionId: string, frame: Uint8Array): void {
    this.sweepIdleSessions();
    const existing = this.frames.get(sessionId);
    if (existing) {
      existing.push(frame);
    } else {
      this.frames.set(sessionId, [frame]);
    }
    this.byteLengths.set(sessionId, (this.byteLengths.get(sessionId) ?? 0) + frame.byteLength);
    this.lastPushedAtMs.set(sessionId, this.now());
  }

  byteLength(sessionId: string): number {
    return this.byteLengths.get(sessionId) ?? 0;
  }

  /** Concatenates without clearing, so an adapter can transcribe an in-flight prefix. */
  snapshot(sessionId: string): Uint8Array {
    return merge(this.frames.get(sessionId) ?? [], this.byteLength(sessionId));
  }

  /** Atomically grants one early-partial request after enough audio exists for that session. */
  claimFirstPartial(sessionId: string, minimumBytes: number): boolean {
    if (this.partialClaimed.has(sessionId) || this.byteLength(sessionId) < minimumBytes) return false;
    this.partialClaimed.add(sessionId);
    return true;
  }

  /** Concatenates and clears the session's buffered audio. Returns an empty buffer if none was pushed. */
  takeAll(sessionId: string): Uint8Array {
    const parts = this.frames.get(sessionId) ?? [];
    const total = this.byteLength(sessionId);
    this.frames.delete(sessionId);
    this.byteLengths.delete(sessionId);
    this.partialClaimed.delete(sessionId);
    this.lastPushedAtMs.delete(sessionId);
    return merge(parts, total);
  }

  private sweepIdleSessions(): void {
    const now = this.now();
    for (const [sessionId, lastPushedAtMs] of this.lastPushedAtMs) {
      if (now - lastPushedAtMs > this.idleTtlMs) {
        this.frames.delete(sessionId);
        this.byteLengths.delete(sessionId);
        this.partialClaimed.delete(sessionId);
        this.lastPushedAtMs.delete(sessionId);
      }
    }
  }
}

function merge(parts: readonly Uint8Array[], total: number): Uint8Array {
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    merged.set(part, offset);
    offset += part.length;
  }
  return merged;
}
