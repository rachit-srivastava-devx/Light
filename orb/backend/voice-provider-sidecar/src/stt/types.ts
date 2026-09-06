export interface SttResult {
  text: string | null;
  is_final: boolean;
}

/**
 * A session-scoped STT backend. `pushAudio` is called once per audio frame the client streamed;
 * `endTurn` is called once when the client signals end-of-turn and must return a non-null final
 * transcript (the contract guarantees `is_final: true` there — see `contract.ts`).
 *
 * Sarvam and Deepgram emit one provider-generated partial from the first second of a multi-second
 * utterance, before `endTurn`, then transcribe the full buffered utterance for the authoritative
 * final. The single prefix request bounds duplicate provider work while unblocking the mobile
 * semantic endpointer. Fish ASR remains batch-only because its public contract has no streaming
 * result surface.
 */
export interface SttBackend {
  readonly label: string;
  pushAudio(sessionId: string, frame: Uint8Array): Promise<SttResult>;
  endTurn(sessionId: string): Promise<SttResult>;
}
