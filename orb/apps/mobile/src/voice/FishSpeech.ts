export interface FishSpeechTransport {
  readonly isClosed: boolean;
  readonly speak: (text: string, voiceId: string, emotion: string) => void;
}

/**
 * The production audio contract: speech is sent to Fish Audio through relay-rs or not sent at all.
 * In particular, this function must never grow a device-TTS fallback; that creates an audible
 * voice switch and hides a broken provider connection from the user.
 */
export function sendFishSpeech(
  transport: FishSpeechTransport | null | undefined,
  text: string,
  voiceId: string,
  emotion: string,
): boolean {
  if (!transport || transport.isClosed) return false;
  transport.speak(text, voiceId, emotion);
  return true;
}
