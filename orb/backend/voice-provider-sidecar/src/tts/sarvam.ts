import type { TtsBackend } from './types.js';
import { PROVIDER_REQUEST_TIMEOUT_MS } from '../provider-timeout.js';

const ENDPOINT = 'https://api.sarvam.ai/text-to-speech';

interface SarvamTtsResponse {
  audios?: string[]; // base64-encoded WAV clips, one per input
}

/**
 * Sarvam Bulbul TTS.
 *
 * CONFIDENCE NOTE: lowest confidence of the three TTS integrations. Sarvam's TTS API returns
 * base64-encoded audio inside a JSON envelope (`audios: string[]`) rather than a binary body,
 * which this implementation decodes. The exact accepted `speaker` values, `target_language_code`
 * default, and whether `emotion` maps to a real request field (Bulbul's documented controls are
 * pitch/pace/loudness, not a free-text emotion) are unverified here — `emotion` is passed through
 * as best-effort context only. Verify against https://docs.sarvam.ai before trusting this live.
 */
/** The product's provider-agnostic voice id, mirroring `fish.ts`'s `resolveReferenceId`. */
const LOGICAL_VOICE_ID = 'orb.warm.v1';
const SPEAKER_ENV_VAR = 'SARVAM_SPEAKER_ORB_WARM_V1';

/**
 * Map the product's provider-agnostic voice id to a real Sarvam `speaker`.
 *
 * THE DEFECT THIS FIXES (measured 2026-08-28): a provider-matrix probe returned Fish=PASS,
 * **Sarvam=FAIL HTTP 400**, because `synthesize` passed `speaker: voiceId` verbatim — so Sarvam
 * received the literal string `"orb.warm.v1"`, which is not one of its speakers. `fish.ts` has had
 * a `resolveReferenceId()` mapping for exactly this since it was written; Sarvam never got the
 * symmetric treatment. The owner's constraint is explicit ("I may use fish audio or sarvam
 * tomorrow"), so a broken Sarvam path is a release blocker, not a nice-to-have.
 *
 * Why this THROWS instead of defaulting to some speaker name: Sarvam's accepted `speaker` values
 * are not verified anywhere in this repo (see the CONFIDENCE NOTE above — even `style` is admitted
 * to be unverified). Inventing a plausible-looking speaker id would just relocate the same opaque
 * 400 while *looking* fixed. Failing loudly with the exact variable to set is honest and one env
 * var away from working.
 *
 * A caller-supplied value that is NOT the logical id passes through untouched, so anyone who knows
 * a real Sarvam speaker can use it directly.
 *
 * STRONGER FORM (not done here): validate this at config time in `config.ts` alongside the API-key
 * check, so a misconfigured Sarvam deployment fails at startup rather than on a user's first turn.
 */
export function resolveSarvamSpeaker(
  voiceId: string,
  env: NodeJS.ProcessEnv = process.env,
): string {
  if (voiceId !== LOGICAL_VOICE_ID) return voiceId;
  const configured = env[SPEAKER_ENV_VAR];
  if (configured && configured.trim().length > 0) return configured.trim();
  throw new Error(
    `sarvam tts: no speaker configured for the logical voice id "${LOGICAL_VOICE_ID}". ` +
      `Set ${SPEAKER_ENV_VAR} to a real Sarvam Bulbul speaker name (see https://docs.sarvam.ai). ` +
      'Passing the logical id through unmapped makes Sarvam reject the request with HTTP 400.',
  );
}

export class SarvamTtsBackend implements TtsBackend {
  readonly label = 'sarvam';

  constructor(private readonly apiKey: string, private readonly fetchImpl: typeof fetch = fetch) {}

  async synthesize(text: string, voiceId: string, emotion: string): Promise<Uint8Array> {
    const response = await this.fetchImpl(ENDPOINT, {
      method: 'POST',
      headers: {
        'api-subscription-key': this.apiKey,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        inputs: [text],
        target_language_code: 'en-IN',
        // Mapped, never passed through raw — see resolveSarvamSpeaker above.
        speaker: resolveSarvamSpeaker(voiceId),
        // Best-effort context, not a verified Bulbul parameter name — see CONFIDENCE NOTE above.
        style: emotion,
      }),
      signal: AbortSignal.timeout(PROVIDER_REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) {
      throw new Error(`sarvam tts request failed: ${response.status} ${await response.text()}`);
    }
    const body = (await response.json()) as SarvamTtsResponse;
    const first = body.audios?.[0];
    if (!first) {
      throw new Error('sarvam tts response omitted audio');
    }
    return new Uint8Array(Buffer.from(first, 'base64'));
  }
}
