/** A TTS backend synthesizes text to raw audio bytes; chunking into the contract's response shape happens in `contract.ts`. */
export interface TtsBackend {
  readonly label: string;
  synthesize(text: string, voiceId: string, emotion: string): Promise<Uint8Array>;
}
