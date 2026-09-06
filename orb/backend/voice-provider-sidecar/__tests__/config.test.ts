import { describe, expect, it, vi } from 'vitest';
import {
  createSttBackendFromEnv,
  createTtsBackendFromEnv,
  MissingProviderConfigError,
  sttProviderFromEnv,
  ttsProviderFromEnv,
} from '../src/config';

describe('provider selection fails closed with no explicit config (no silent fake fallback)', () => {
  it('STT throws a typed error when ORB_STT_PROVIDER is unset', () => {
    expect(() => sttProviderFromEnv({})).toThrow(MissingProviderConfigError);
    expect(() => sttProviderFromEnv({})).toThrow(/ORB_STT_PROVIDER must be set explicitly/);
  });

  it('TTS throws a typed error when ORB_TTS_PROVIDER is unset', () => {
    expect(() => ttsProviderFromEnv({})).toThrow(MissingProviderConfigError);
    expect(() => ttsProviderFromEnv({})).toThrow(/ORB_TTS_PROVIDER must be set explicitly/);
  });

  it('createSttBackendFromEnv/createTtsBackendFromEnv also fail closed with no config', () => {
    expect(() => createSttBackendFromEnv({})).toThrow(MissingProviderConfigError);
    expect(() => createTtsBackendFromEnv({})).toThrow(MissingProviderConfigError);
  });

  it('an explicit ORB_STT_PROVIDER=fake / ORB_TTS_PROVIDER=fake still opts in to the fake backend', () => {
    expect(sttProviderFromEnv({ ORB_STT_PROVIDER: 'fake' })).toBe('fake');
    expect(createSttBackendFromEnv({ ORB_STT_PROVIDER: 'fake' }).label).toBe('fake');
    expect(ttsProviderFromEnv({ ORB_TTS_PROVIDER: 'fake' })).toBe('fake');
    expect(createTtsBackendFromEnv({ ORB_TTS_PROVIDER: 'fake' }).label).toBe('fake');
  });

  it('explicit fake provider logs a SYNTHETIC marker; a real provider never does', () => {
    // Mutation-testing gate (G4) found this: the SYNTHETIC console.warn had zero test coverage
    // and could be silently deleted with no red test. Pin it.
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    try {
      createSttBackendFromEnv({ ORB_STT_PROVIDER: 'fake' });
      expect(warn).toHaveBeenCalledWith(expect.stringMatching(/SYNTHETIC/));
      warn.mockClear();

      createTtsBackendFromEnv({ ORB_TTS_PROVIDER: 'fake' });
      expect(warn).toHaveBeenCalledWith(expect.stringMatching(/SYNTHETIC/));
      warn.mockClear();

      createSttBackendFromEnv({ ORB_STT_PROVIDER: 'sarvam', SARVAM_API_KEY: 'k' });
      createTtsBackendFromEnv({ ORB_TTS_PROVIDER: 'cartesia', CARTESIA_API_KEY: 'k' });
      expect(warn).not.toHaveBeenCalled();
    } finally {
      warn.mockRestore();
    }
  });

  it('rejects an unknown STT provider name instead of silently falling back', () => {
    expect(() => sttProviderFromEnv({ ORB_STT_PROVIDER: 'whisper' })).toThrow(/unsupported ORB_STT_PROVIDER/);
  });

  it('rejects an unknown TTS provider name instead of silently falling back', () => {
    expect(() => ttsProviderFromEnv({ ORB_TTS_PROVIDER: 'elevenlabs' })).toThrow(/unsupported ORB_TTS_PROVIDER/);
  });
});

describe('fail-fast on a selected provider with no key (mirrors gateway-sidecar)', () => {
  it('sarvam STT requires SARVAM_API_KEY', () => {
    expect(() => createSttBackendFromEnv({ ORB_STT_PROVIDER: 'sarvam' })).toThrow(
      /SARVAM_API_KEY is required when ORB_STT_PROVIDER=sarvam/,
    );
  });

  it('deepgram STT requires DEEPGRAM_API_KEY', () => {
    expect(() => createSttBackendFromEnv({ ORB_STT_PROVIDER: 'deepgram' })).toThrow(
      /DEEPGRAM_API_KEY is required when ORB_STT_PROVIDER=deepgram/,
    );
  });

  it('fish STT requires FISH_API_KEY', () => {
    expect(() => createSttBackendFromEnv({ ORB_STT_PROVIDER: 'fish' })).toThrow(
      /FISH_API_KEY is required when ORB_STT_PROVIDER=fish/,
    );
  });

  it('muse STT requires MUSE_API_KEY', () => {
    expect(() => createSttBackendFromEnv({ ORB_STT_PROVIDER: 'muse' })).toThrow(
      /MUSE_API_KEY is required when ORB_STT_PROVIDER=muse/,
    );
  });

  it('fish TTS requires FISH_API_KEY', () => {
    expect(() => createTtsBackendFromEnv({ ORB_TTS_PROVIDER: 'fish' })).toThrow(
      /FISH_API_KEY is required when ORB_TTS_PROVIDER=fish/,
    );
  });

  it('sarvam TTS requires SARVAM_API_KEY', () => {
    expect(() => createTtsBackendFromEnv({ ORB_TTS_PROVIDER: 'sarvam' })).toThrow(
      /SARVAM_API_KEY is required when ORB_TTS_PROVIDER=sarvam/,
    );
  });

  it('cartesia TTS requires CARTESIA_API_KEY', () => {
    expect(() => createTtsBackendFromEnv({ ORB_TTS_PROVIDER: 'cartesia' })).toThrow(
      /CARTESIA_API_KEY is required when ORB_TTS_PROVIDER=cartesia/,
    );
  });

  it('a present key selects the real backend and does not throw', () => {
    expect(createSttBackendFromEnv({ ORB_STT_PROVIDER: 'fish', FISH_API_KEY: 'k' }).label).toBe('fish');
    expect(createSttBackendFromEnv({ ORB_STT_PROVIDER: 'sarvam', SARVAM_API_KEY: 'k' }).label).toBe('sarvam');
    expect(createSttBackendFromEnv({ ORB_STT_PROVIDER: 'muse', MUSE_API_KEY: 'k' }).label).toBe('muse');
    expect(createTtsBackendFromEnv({ ORB_TTS_PROVIDER: 'cartesia', CARTESIA_API_KEY: 'k' }).label).toBe(
      'cartesia',
    );
  });
});
