import type { SpeechPort } from './SpeechPort';

export type SpeechFailure = 'provider_failure' | 'invalid_tts_audio' | 'tts_playback_failure';

const SPOKEN_FAILURE_MESSAGES: Readonly<Record<SpeechFailure, string>> = {
  provider_failure: 'I’m having trouble with my voice connection, but I’m still here. Please try again.',
  invalid_tts_audio: 'I couldn’t play that response. Please try again.',
  tts_playback_failure: 'I couldn’t play that response. Please try again.',
};

export function spokenMessageForFailure(failure: SpeechFailure): string {
  return SPOKEN_FAILURE_MESSAGES[failure];
}

/** The lowest compatible seam for failure copy: the relay-backed injected SpeechPort. */
export async function speakFailure(speechPort: SpeechPort | undefined, failure: SpeechFailure): Promise<void> {
  if (!speechPort) return;
  try {
    await speechPort.speak(spokenMessageForFailure(failure), 'orb.warm.v1', 'supportive');
  } catch {
    // A failed voice path must not create an unhandled rejection; the caller owns degraded UI.
  }
}
