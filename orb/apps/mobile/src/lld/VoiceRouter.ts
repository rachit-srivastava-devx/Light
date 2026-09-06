/**
 * Deterministic voice routing (`docs/BUILD-DIGEST.md` §2, doc 12 §8).
 *
 * The route is selected once and pinned into the session envelope. No randomness, no latency probe,
 * and no provider call belongs here.
 */

export type VoiceProvider = 'fish' | 'sarvam' | 'cartesia';

export interface VoiceRoute {
  readonly provider: VoiceProvider;
  readonly voice_id: string;
  readonly locale: string;
  readonly region: string;
}

const DEFAULT_ROUTE: VoiceRoute = {
  provider: 'fish',
  voice_id: 'orb.warm.v1',
  locale: 'en-US',
  region: 'global',
};

const INDIA_ROUTE: VoiceRoute = {
  provider: 'sarvam',
  voice_id: 'orb.warm.hinglish.v1',
  locale: 'en-IN',
  region: 'IN',
};

export function selectVoice(region: string, locale: string): VoiceRoute {
  const normalizedRegion = region.trim().toUpperCase();
  const normalizedLocale = locale.trim();
  if (normalizedRegion === 'IN' || normalizedLocale.toLowerCase() === 'en-in') {
    return { ...INDIA_ROUTE, locale: normalizedLocale || INDIA_ROUTE.locale };
  }
  return { ...DEFAULT_ROUTE, locale: normalizedLocale || DEFAULT_ROUTE.locale };
}

export function pinVoiceForSession(
  existing: VoiceRoute | null,
  region: string,
  locale: string,
): VoiceRoute {
  return existing ?? selectVoice(region, locale);
}
