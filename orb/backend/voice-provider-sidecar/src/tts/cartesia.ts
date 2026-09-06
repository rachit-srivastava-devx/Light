import type { TtsBackend } from './types.js';
import { PROVIDER_REQUEST_TIMEOUT_MS } from '../provider-timeout.js';

const ENDPOINT = 'https://api.cartesia.ai/tts/bytes';
const CARTESIA_VERSION = '2024-06-10';

/**
 * Cartesia Sonic TTS.
 *
 * CONFIDENCE NOTE: moderate confidence. `X-API-Key` auth, the required `Cartesia-Version` header,
 * `POST /tts/bytes` for a raw-binary response, and the `voice: { mode: "id", id }` /
 * `output_format` request shape match Cartesia's documented API; the specific `model_id` value
 * ("sonic-2") and whether the account's Cartesia-Version pin matches the one hardcoded here are
 * the parts most likely to have drifted — verify against https://docs.cartesia.ai before a real
 * key is used. `emotion` has no direct Cartesia field and is not sent; Cartesia expresses
 * emotion/style via `voice.__experimental_controls`, which is unverified here.
 */
export class CartesiaTtsBackend implements TtsBackend {
  readonly label = 'cartesia';

  constructor(private readonly apiKey: string, private readonly fetchImpl: typeof fetch = fetch) {}

  async synthesize(text: string, voiceId: string, _emotion: string): Promise<Uint8Array> {
    const response = await this.fetchImpl(ENDPOINT, {
      method: 'POST',
      headers: {
        'X-API-Key': this.apiKey,
        'Cartesia-Version': CARTESIA_VERSION,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model_id: 'sonic-2',
        transcript: text,
        voice: { mode: 'id', id: voiceId },
        output_format: { container: 'wav', encoding: 'pcm_s16le', sample_rate: 16000 },
        language: 'en',
      }),
      signal: AbortSignal.timeout(PROVIDER_REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) {
      throw new Error(`cartesia tts request failed: ${response.status} ${await response.text()}`);
    }
    return new Uint8Array(await response.arrayBuffer());
  }
}
