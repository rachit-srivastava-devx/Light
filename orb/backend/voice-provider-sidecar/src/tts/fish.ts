import type { TtsBackend } from './types.js';
import { PROVIDER_REQUEST_TIMEOUT_MS } from '../provider-timeout.js';

const ENDPOINT = 'https://api.fish.audio/v1/tts';

/**
 * The relay contract is currently headerless PCM, so its sample rate must remain explicit and
 * consistent with `RelayAudioPlayer`. The quality fix is the voice/model/prosody selection below;
 * changing this rate requires a versioned relay audio contract.
 */
const RELAY_AUDIO_SAMPLE_RATE_HZ = 24_000;

/** Fish recommends `s2-pro` for new TTS integrations and documents it as the quality model. */
const TTS_MODEL = process.env['FISH_TTS_MODEL']?.trim() || 's2-pro';

/**
 * `orb.warm.v1` is this product's provider-agnostic voice id (referenced across
 * apps/mobile/src/runtime/T0FocusSession.ts and lld/VoiceRouter.ts) — swapping TTS providers
 * should not mean editing every call site. This maps it to a real Fish `reference_id` when one is
 * known to exist on the configured account.
 *
 * `orb.warm.v1` maps to a public-library voice chosen for clear, conversational, energetic speech.
 * Override it with `FISH_REFERENCE_ID_ORB_WARM_V1` when a different Fish voice is deliberately
 * selected.
 */
const DEFAULT_FISH_REFERENCE_ID = '59e9dc1cb20c452584788a2690c80970'; // ALLE: clear, energetic, friendly library voice

interface FishProsody {
  readonly speed: number;
  readonly volume: number;
  readonly normalize_loudness: true;
}

// Speaking rate, and it was biased the wrong way: every entry except `gentle`/`calm` sat ABOVE 1.0,
// with the default `warm` at 1.04. Measured consequence, on real Fish output through this sidecar:
// **3.4 words/sec**, against a natural conversational 2.2–3.0. The owner's report while testing was
// "I can not make that out clearly", and a too-fast rate is exactly that symptom.
//
// Two other explanations were measured and FALSIFIED first, so this is not a guess:
//   * a 16 kHz-vs-24 kHz playback mismatch — refuted: the returned audio carries real energy at
//     9–10 kHz, which a 16 kHz stream cannot (its Nyquist is 8 kHz);
//   * Fish reading the `[sigh]`/`[short pause]` tags aloud — refuted: tagged vs untagged differ by
//     only +8%, consistent with an inserted pause rather than spoken words.
//
// Why slower is the right default rather than a preference: listener processing time is one of the
// better-supported comprehension findings, and this product's listener is short of exactly the
// working memory that keeping up with fast speech consumes (see
// ~/.claude/skills/adhd-conversation-design/SKILL.md §5 — chunking and pacing).
//
// Everything is now at or below 1.0. Emotion still varies the rate — that contrast is what keeps
// the voice from sounding flat — but it varies DOWNWARD from natural, never upward.
const PROSODY_BY_EMOTION: Readonly<Record<string, FishProsody>> = {
  warm: { speed: 0.94, volume: 1, normalize_loudness: true },
  calm: { speed: 0.88, volume: 0, normalize_loudness: true },
  curious: { speed: 0.97, volume: 1, normalize_loudness: true },
  upbeat: { speed: 1.0, volume: 2, normalize_loudness: true },
  celebratory: { speed: 1.0, volume: 2, normalize_loudness: true },
  gentle: { speed: 0.9, volume: 0, normalize_loudness: true },
  matter_of_fact: { speed: 0.95, volume: 1, normalize_loudness: true },
};

/**
 * No emotion may speak faster than natural. Exported so a test asserts the INVARIANT rather than
 * seven magic numbers — the table can be retuned freely, but never back above 1.0.
 */
export const MAX_SPEAKING_RATE = 1.0;

const SAMPLING_BY_EMOTION: Readonly<Record<string, { readonly temperature: number; readonly top_p: number }>> = {
  warm: { temperature: 0.78, top_p: 0.8 },
  calm: { temperature: 0.72, top_p: 0.75 },
  curious: { temperature: 0.82, top_p: 0.84 },
  upbeat: { temperature: 0.84, top_p: 0.86 },
  celebratory: { temperature: 0.86, top_p: 0.88 },
  gentle: { temperature: 0.74, top_p: 0.78 },
  matter_of_fact: { temperature: 0.76, top_p: 0.8 },
};
const DEFAULT_SAMPLING = { temperature: 0.78, top_p: 0.8 } as const;

const INLINE_FISH_TAG = /\[[^\]]+\]/;

/**
 * Deterministic T0/local responses do not pass through the LLM speech envelope. Give those bare
 * phrases the same small amount of human pacing without overriding a model-authored Fish plan.
 * This stays intentionally conservative: one breath, one mood, and at most one sentence beat.
 */
function addBareTextProsody(text: string, emotion: string): string {
  const trimmed = text.trim();
  if (!trimmed || INLINE_FISH_TAG.test(trimmed)) return text;
  const normalizedEmotion = emotion.trim().toLowerCase();
  const mood = normalizedEmotion === 'celebratory' || normalizedEmotion === 'upbeat'
    ? 'excited'
    : normalizedEmotion === 'curious'
      ? 'curious'
      : normalizedEmotion === 'repair' || normalizedEmotion === 'supportive'
        ? 'empathetic'
        : normalizedEmotion === 'calm' || normalizedEmotion === 'gentle'
          ? 'gentle'
          : 'friendly';
  const paced = trimmed.replace(/([.!?])\s+(?=[A-Z])/g, '$1 [short pause] ');
  return `[inhale] [${mood}] ${paced}`;
}

function resolveReferenceId(voiceId: string): string | undefined {
  if (voiceId !== 'orb.warm.v1') return voiceId;
  const envOverride = process.env['FISH_REFERENCE_ID_ORB_WARM_V1'];
  return envOverride && envOverride.trim().length > 0 ? envOverride : DEFAULT_FISH_REFERENCE_ID;
}

/**
 * Fish Audio S2-Pro TTS.
 *
 * The response is raw binary audio; the sidecar requests WAV and strips its container before the
 * relay sends the explicit PCM contract to mobile.
 */
export class FishTtsBackend implements TtsBackend {
  readonly label = 'fish';

  constructor(private readonly apiKey: string, private readonly fetchImpl: typeof fetch = fetch) {}

  async synthesize(text: string, voiceId: string, emotion: string): Promise<Uint8Array> {
    const sampling = SAMPLING_BY_EMOTION[emotion] ?? DEFAULT_SAMPLING;
    const response = await this.fetchImpl(ENDPOINT, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${this.apiKey}`,
        'Content-Type': 'application/json',
        model: TTS_MODEL,
      },
      body: JSON.stringify({
        text: addBareTextProsody(text, emotion),
        reference_id: resolveReferenceId(voiceId),
        format: 'wav',
        // Keep sampling in Fish's expressive-but-stable middle band. The value is deliberately
        // emotion-aware: celebration can vary more, while repair/calm speech stays intelligible.
        temperature: sampling.temperature,
        top_p: sampling.top_p,
        prosody: PROSODY_BY_EMOTION[emotion] ?? PROSODY_BY_EMOTION.warm,
        // MUST match apps/mobile/src/voice/RelayAudioPlayer.ts's RELAY_AUDIO_SAMPLE_RATE_HZ. The
        // 24 kHz relay target preserves more consonant/high-frequency detail than the old 16 kHz
        // TTS path while the client still resamples exactly once to the native output rate.
        sample_rate: RELAY_AUDIO_SAMPLE_RATE_HZ,
        latency: 'normal',
        chunk_length: 300,
        normalize: true,
        max_new_tokens: 1024,
        repetition_penalty: 1.2,
        min_chunk_length: 50,
        condition_on_previous_chunks: true,
        early_stop_threshold: 1,
      }),
      signal: AbortSignal.timeout(PROVIDER_REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) {
      throw new Error(`fish audio tts request failed: ${response.status} ${await response.text()}`);
    }
    return new Uint8Array(await response.arrayBuffer());
  }
}
