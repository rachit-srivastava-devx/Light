import type { SttBackend } from './stt/types.js';
import type { TtsBackend } from './tts/types.js';
import { FakeSttBackend } from './stt/fake.js';
import { FishSttBackend } from './stt/fish.js';
import { SarvamSttBackend } from './stt/sarvam.js';
import { DeepgramSttBackend } from './stt/deepgram.js';
import { MuseSttBackend } from './stt/muse.js';
import { FakeTtsBackend } from './tts/fake.js';
import { FishTtsBackend } from './tts/fish.js';
import { SarvamTtsBackend } from './tts/sarvam.js';
import { CartesiaTtsBackend } from './tts/cartesia.js';

export type SttProviderName = 'fake' | 'fish' | 'sarvam' | 'deepgram' | 'muse';
export type TtsProviderName = 'fake' | 'fish' | 'sarvam' | 'cartesia';

const STT_PROVIDERS: readonly SttProviderName[] = ['fake', 'fish', 'sarvam', 'deepgram', 'muse'];
const TTS_PROVIDERS: readonly TtsProviderName[] = ['fake', 'fish', 'sarvam', 'cartesia'];

/**
 * Thrown when a provider env var (`ORB_STT_PROVIDER` / `ORB_TTS_PROVIDER`) was never set at all.
 *
 * There is deliberately no implicit default. A missing var used to silently resolve to `fake`,
 * which meant a deployment that forgot to configure a real provider would boot and produce
 * synthetic audio indistinguishable (to an operator watching it work) from real speech. `fake` is
 * still available for local dev/testing, but it must be requested explicitly
 * (`ORB_STT_PROVIDER=fake` / `ORB_TTS_PROVIDER=fake`) so the choice shows up in config/logs.
 */
export class MissingProviderConfigError extends Error {
  constructor(
    public readonly envVar: string,
    public readonly validValues: readonly string[],
  ) {
    super(
      `${envVar} must be set explicitly — there is no implicit default (expected one of ` +
        `${validValues.join('|')}). Set ${envVar}=fake for local dev/testing.`,
    );
    this.name = 'MissingProviderConfigError';
  }
}

export function sttProviderFromEnv(env: NodeJS.ProcessEnv = process.env): SttProviderName {
  const value = env['ORB_STT_PROVIDER'];
  if (value === undefined) throw new MissingProviderConfigError('ORB_STT_PROVIDER', STT_PROVIDERS);
  if ((STT_PROVIDERS as readonly string[]).includes(value)) return value as SttProviderName;
  throw new Error(`unsupported ORB_STT_PROVIDER: ${value} (expected one of ${STT_PROVIDERS.join('|')})`);
}

export function ttsProviderFromEnv(env: NodeJS.ProcessEnv = process.env): TtsProviderName {
  const value = env['ORB_TTS_PROVIDER'];
  if (value === undefined) throw new MissingProviderConfigError('ORB_TTS_PROVIDER', TTS_PROVIDERS);
  if ((TTS_PROVIDERS as readonly string[]).includes(value)) return value as TtsProviderName;
  throw new Error(`unsupported ORB_TTS_PROVIDER: ${value} (expected one of ${TTS_PROVIDERS.join('|')})`);
}

/**
 * Fails fast (throws synchronously at boot, mirroring gateway-sidecar's
 * `ANTHROPIC_API_KEY is required when...` pattern) when the selected provider has no key. A
 * missing key must never become a silent no-op or a fallback to the fake — the caller needs to
 * know the sidecar cannot do real work with this configuration.
 */
export function createSttBackendFromEnv(env: NodeJS.ProcessEnv = process.env): SttBackend {
  const provider = sttProviderFromEnv(env);
  if (provider === 'fake') {
    // eslint-disable-next-line no-console
    console.warn('voice-provider-sidecar: ORB_STT_PROVIDER=fake selected — transcripts are SYNTHETIC, not real STT.');
    return new FakeSttBackend();
  }
  if (provider === 'fish') {
    const key = requireKey(env, 'FISH_API_KEY', 'ORB_STT_PROVIDER=fish');
    return new FishSttBackend(key);
  }
  if (provider === 'sarvam') {
    const key = requireKey(env, 'SARVAM_API_KEY', 'ORB_STT_PROVIDER=sarvam');
    return new SarvamSttBackend(key);
  }
  if (provider === 'deepgram') {
    const key = requireKey(env, 'DEEPGRAM_API_KEY', 'ORB_STT_PROVIDER=deepgram');
    return new DeepgramSttBackend(key);
  }
  const key = requireKey(env, 'MUSE_API_KEY', 'ORB_STT_PROVIDER=muse');
  return new MuseSttBackend(key);
}

export function createTtsBackendFromEnv(env: NodeJS.ProcessEnv = process.env): TtsBackend {
  const provider = ttsProviderFromEnv(env);
  if (provider === 'fake') {
    // eslint-disable-next-line no-console
    console.warn('voice-provider-sidecar: ORB_TTS_PROVIDER=fake selected — audio produced is SYNTHETIC, not real speech.');
    return new FakeTtsBackend();
  }
  if (provider === 'fish') {
    const key = requireKey(env, 'FISH_API_KEY', 'ORB_TTS_PROVIDER=fish');
    return new FishTtsBackend(key);
  }
  if (provider === 'sarvam') {
    const key = requireKey(env, 'SARVAM_API_KEY', 'ORB_TTS_PROVIDER=sarvam');
    return new SarvamTtsBackend(key);
  }
  const key = requireKey(env, 'CARTESIA_API_KEY', 'ORB_TTS_PROVIDER=cartesia');
  return new CartesiaTtsBackend(key);
}

function requireKey(env: NodeJS.ProcessEnv, varName: string, because: string): string {
  const key = env[varName];
  if (!key) {
    throw new Error(`${varName} is required when ${because}`);
  }
  return key;
}
