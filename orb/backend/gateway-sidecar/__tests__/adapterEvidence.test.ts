import { describe, expect, it } from 'vitest';
import {
  FAKE_ADAPTER_MODEL_MARKER,
  FAKE_ADAPTER_TEXT_MARKER,
  adapterLabelMatchesEvidence,
  isFakeAdapterInvocation,
} from '../src/adapterEvidence';

/**
 * E1 (Track E) — "a run on fakes can never be mistaken for a real run."
 *
 * These are pure unit tests of the one predicate this whole telemetry feature stands on. The
 * scar this guards against is not "the memory adapter answers" (T0 without keys must still work)
 * — it is "a caller believes/claims a real model answered when it did not." So the interesting
 * cases here are not just memory-in/fake-out and anthropic-in/real-out; the load-bearing ones are
 * the label/evidence MISMATCHES: an adapter that claims to be real while carrying the memory
 * adapter's own literal markers.
 */
describe('isFakeAdapterInvocation — the memory adapter must be detectable', () => {
  it('flags the memory adapter honestly labeled as memory', () => {
    expect(
      isFakeAdapterInvocation({
        adapter: 'memory',
        model: FAKE_ADAPTER_MODEL_MARKER,
        responseText: FAKE_ADAPTER_TEXT_MARKER,
      }),
    ).toBe(true);
  });

  it('flags adapter="memory" even if the model/text markers were somehow absent', () => {
    // Defends against a future memory-adapter refactor that changes its fake constants without
    // this file being updated: the adapter *label* alone is sufficient to veto.
    expect(isFakeAdapterInvocation({ adapter: 'memory', model: 'anything', responseText: 'anything' })).toBe(
      true,
    );
  });

  it('does NOT flag a real anthropic response with a real model id and real text', () => {
    expect(
      isFakeAdapterInvocation({
        adapter: 'anthropic',
        model: 'claude-haiku-4-5-20251001',
        responseText: 'Mango is sweeter and more floral; pineapple is sharper and more acidic.',
      }),
    ).toBe(false);
  });

  it('does NOT flag a real gemini response with a real model id and real text', () => {
    expect(
      isFakeAdapterInvocation({
        adapter: 'gemini',
        model: 'gemini-2.5-flash',
        responseText: 'Black holes are regions where gravity is so strong not even light escapes.',
      }),
    ).toBe(false);
  });

  it('flags a response labeled "anthropic" that carries the memory model marker (a lying label)', () => {
    // This is the actual property under test: detectability must not depend on trusting the
    // caller's own `adapter` field. If the evidence disagrees with the label, that disagreement
    // itself is the fake signal.
    expect(
      isFakeAdapterInvocation({
        adapter: 'anthropic',
        model: FAKE_ADAPTER_MODEL_MARKER,
        responseText: 'this looks like a real reply',
      }),
    ).toBe(true);
  });

  it('flags a response labeled "gemini" whose text is exactly the memory adapter\'s canned string', () => {
    expect(
      isFakeAdapterInvocation({
        adapter: 'gemini',
        model: 'gemini-2.5-flash',
        responseText: FAKE_ADAPTER_TEXT_MARKER,
      }),
    ).toBe(true);
  });

  it('flags a response whose text merely CONTAINS the fake marker (not just an exact match)', () => {
    expect(
      isFakeAdapterInvocation({
        adapter: 'anthropic',
        model: 'claude-haiku-4-5-20251001',
        responseText: `prefix ${FAKE_ADAPTER_TEXT_MARKER} suffix`,
      }),
    ).toBe(true);
  });
});

describe('adapterLabelMatchesEvidence — distinguishes honest-fake from lying-about-real', () => {
  it('is true for an honestly labeled memory response', () => {
    expect(
      adapterLabelMatchesEvidence({
        adapter: 'memory',
        model: FAKE_ADAPTER_MODEL_MARKER,
        responseText: FAKE_ADAPTER_TEXT_MARKER,
      }),
    ).toBe(true);
  });

  it('is true for a real anthropic response', () => {
    expect(
      adapterLabelMatchesEvidence({
        adapter: 'anthropic',
        model: 'claude-haiku-4-5-20251001',
        responseText: 'a real answer',
      }),
    ).toBe(true);
  });

  it('is false when adapter="anthropic" but the evidence is the memory adapter\'s own marker', () => {
    expect(
      adapterLabelMatchesEvidence({
        adapter: 'anthropic',
        model: FAKE_ADAPTER_MODEL_MARKER,
        responseText: 'a real answer',
      }),
    ).toBe(false);
  });
});
