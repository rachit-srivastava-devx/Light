import { describe, expect, it } from 'vitest';

import { resolveSarvamSpeaker } from '../src/tts/sarvam.js';

// Regression guard for a measured defect (2026-08-28): a provider-matrix probe returned
// Fish=PASS / Sarvam=FAIL HTTP 400, because the adapter sent `speaker: "orb.warm.v1"` — the
// product's provider-agnostic voice id — straight through to Sarvam, which has no such speaker.
// `fish.ts` had a resolveReferenceId() mapping for exactly this; Sarvam never got the symmetric
// one. The owner's constraint is that the TTS provider is swappable ("I may use fish audio or
// sarvam tomorrow"), so this is a release blocker rather than a rough edge.
describe('resolveSarvamSpeaker', () => {
  it('maps the logical voice id to the configured Sarvam speaker', () => {
    expect(resolveSarvamSpeaker('orb.warm.v1', { SARVAM_SPEAKER_ORB_WARM_V1: 'some-speaker' }))
      .toBe('some-speaker');
  });

  it('trims a padded env value rather than sending whitespace to the provider', () => {
    expect(resolveSarvamSpeaker('orb.warm.v1', { SARVAM_SPEAKER_ORB_WARM_V1: '  spk  ' }))
      .toBe('spk');
  });

  it('passes a caller-supplied real speaker through untouched', () => {
    // Anyone who already knows a valid Sarvam speaker must not be forced through the env var.
    expect(resolveSarvamSpeaker('a-real-sarvam-speaker', {})).toBe('a-real-sarvam-speaker');
  });

  it('FAILS LOUDLY when the logical id has no mapping, instead of causing an opaque HTTP 400', () => {
    // The whole point: an unmapped logical id must never reach Sarvam. Before this, it did, and the
    // user found out via a 400 on their first spoken turn.
    expect(() => resolveSarvamSpeaker('orb.warm.v1', {})).toThrow(/SARVAM_SPEAKER_ORB_WARM_V1/);
  });

  it('treats a whitespace-only env value as unset', () => {
    expect(() => resolveSarvamSpeaker('orb.warm.v1', { SARVAM_SPEAKER_ORB_WARM_V1: '   ' }))
      .toThrow(/SARVAM_SPEAKER_ORB_WARM_V1/);
  });
});
