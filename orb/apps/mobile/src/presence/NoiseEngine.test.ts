import { describe, expect, it } from 'vitest';
import {
  brownNoise,
  buildNoiseBuffer,
  crossfadeLoop,
  generateWhiteNoise,
  pinkNoise,
  seededRandom,
} from './NoiseEngine';

describe('seededRandom / generateWhiteNoise', () => {
  it('is deterministic for a given seed', () => {
    const a = generateWhiteNoise(1000, 42);
    const b = generateWhiteNoise(1000, 42);
    expect(Array.from(a)).toEqual(Array.from(b));
  });

  it('produces different sequences for different seeds', () => {
    const a = generateWhiteNoise(1000, 1);
    const b = generateWhiteNoise(1000, 2);
    expect(Array.from(a)).not.toEqual(Array.from(b));
  });

  it('stays within [-1, 1]', () => {
    const rand = seededRandom(7);
    for (let i = 0; i < 10_000; i++) {
      const v = rand();
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
    }
  });
});

describe('brownNoise', () => {
  it('never exceeds the [-1, 1] clamp (§1 formula)', () => {
    const white = generateWhiteNoise(50_000, 3);
    const brown = brownNoise(white);
    for (const y of brown) {
      expect(y).toBeGreaterThanOrEqual(-1);
      expect(y).toBeLessThanOrEqual(1);
    }
  });

  it('is smoother (lower sample-to-sample variance) than the white noise feeding it', () => {
    const white = generateWhiteNoise(20_000, 5);
    const brown = brownNoise(white);
    const diffVariance = (arr: Float64Array) => {
      let sum = 0;
      for (let i = 1; i < arr.length; i++) {
        const d = (arr[i] as number) - (arr[i - 1] as number);
        sum += d * d;
      }
      return sum / (arr.length - 1);
    };
    expect(diffVariance(brown)).toBeLessThan(diffVariance(white));
  });
});

describe('pinkNoise', () => {
  it('is deterministic and finite for a fixed input', () => {
    const white = generateWhiteNoise(5000, 9);
    const pink1 = pinkNoise(white);
    const pink2 = pinkNoise(white);
    expect(Array.from(pink1)).toEqual(Array.from(pink2));
    for (const v of pink1) {
      expect(Number.isFinite(v)).toBe(true);
    }
  });

  it('matches the first published Paul Kellett coefficient step from a zero state', () => {
    const white = new Float64Array([1, 0, 0]);
    const pink = pinkNoise(white);
    // b0..b6 all start at 0; first sample: b0=0.0555179, b1=0.0750759, b2=0.153852,
    // b3=0.3104856, b4=0.5329522, b5=-0.016898, b6(prev)=0, plus w*0.5362 = 0.5362.
    const expectedSum =
      0.0555179 + 0.0750759 + 0.153852 + 0.3104856 + 0.5329522 + -0.016898 + 0 + 0.5362;
    expect(pink[0]).toBeCloseTo(expectedSum * 0.11, 10);
  });
});

/**
 * Loop-seam helpers. The invariant blueprint 02 §2 states is that the wrap is "continuous in both
 * amplitude and spectrum" — i.e. the wrap must be *indistinguishable from an ordinary sample
 * transition*, not equal to some magic constant. `out[N-1] === out[0]` is NOT that invariant: it is
 * a scalar a degenerate blend can satisfy while leaving an audible cusp (LESSONS #1), so these
 * helpers measure the wrap against the buffer's own interior statistics instead.
 */
function meanAbsStep(a: Float64Array, from = 1, to = a.length): number {
  let s = 0;
  for (let i = from; i < to; i++) s += Math.abs((a[i] as number) - (a[i - 1] as number));
  return s / (to - from);
}
function wrapValueStep(a: Float64Array): number {
  return Math.abs((a[0] as number) - (a[a.length - 1] as number));
}
/** Change in slope across the wrap. A time-reversal cusp mirrors the slope and doubles this. */
function wrapSlopeJump(a: Float64Array): number {
  const before = (a[a.length - 1] as number) - (a[a.length - 2] as number);
  const after = (a[1] as number) - (a[0] as number);
  return Math.abs(after - before);
}
function rmsWindow(a: Float64Array, from: number, to: number): number {
  let s = 0;
  for (let i = from; i < to; i++) s += (a[i] as number) * (a[i] as number);
  return Math.sqrt(s / (to - from));
}

describe('crossfadeLoop', () => {
  const SR = 48_000;
  const XF = Math.round(0.25 * SR); // §1 ~250ms

  it.each(['brown', 'pink'] as const)(
    'makes the %s wrap indistinguishable from an ordinary sample transition',
    (color) => {
      const white = generateWhiteNoise(SR + XF, 11);
      const raw = color === 'brown' ? brownNoise(white) : pinkNoise(white);
      const faded = crossfadeLoop(raw, XF);

      expect(faded.length).toBe(SR);
      // measure interior statistics away from the crossfade window
      const interior = meanAbsStep(faded, XF + 1, SR);

      // amplitude continuity: the wrap step is within a few ordinary steps, not a jump
      expect(wrapValueStep(faded)).toBeLessThan(interior * 6);
      // spectral/slope continuity: no cusp. A reversed-head blend mirrors the slope and lands
      // at >1.5x interior here; the correct overlap-add construction lands well under 1x.
      expect(wrapSlopeJump(faded)).toBeLessThan(interior);
    },
  );

  it.each(['brown', 'pink'] as const)(
    'holds %s bed level flat through the crossfade window (no periodic dip)',
    (color) => {
      const white = generateWhiteNoise(SR + XF, 12);
      const raw = color === 'brown' ? brownNoise(white) : pinkNoise(white);
      const faded = crossfadeLoop(raw, XF);

      const inFade = rmsWindow(faded, Math.floor(XF * 0.3), Math.floor(XF * 0.7));
      const outside = rmsWindow(faded, 2 * XF, SR);
      const dipDb = 20 * Math.log10(inFade / outside);
      // equal-power weights keep this near 0 dB. Linear weights over uncorrelated noise dip ~3 dB,
      // which is the 30s periodicity blueprint 02 §11.4's soak FFT is written to catch.
      expect(Math.abs(dipDb)).toBeLessThan(1.5);
    },
  );

  it('returns only the output length, discarding the generator overhang', () => {
    const raw = generateWhiteNoise(1100, 4);
    expect(crossfadeLoop(raw, 100).length).toBe(1000);
  });

  it('is a no-op when crossfadeSamples is 0', () => {
    const raw = generateWhiteNoise(100, 4);
    const faded = crossfadeLoop(raw, 0);
    expect(Array.from(faded)).toEqual(Array.from(raw));
  });

  it('leaves everything after the crossfade window untouched', () => {
    const raw = generateWhiteNoise(1100, 4);
    const faded = crossfadeLoop(raw, 100);
    // the write window is the HEAD [0,100) — the overhang is folded onto it; [100,1000) is raw.
    for (let i = 100; i < 1000; i++) {
      expect(faded[i]).toBe(raw[i]);
    }
  });
});

describe('buildNoiseBuffer', () => {
  it('builds a buffer of the requested duration and sample rate', () => {
    const buf = buildNoiseBuffer('brown', 1, 1, 8000, 100);
    expect(buf.sample_rate_hz).toBe(8000);
    expect(buf.samples.length).toBe(8000);
  });

  it('loops without an audible seam for both colours, at the real 30s/48kHz size', () => {
    for (const color of ['brown', 'pink'] as const) {
      const withFade = buildNoiseBuffer(color, 21); // contract defaults: 30s, 48kHz, 250ms
      const withoutFade = buildNoiseBuffer(color, 21, 30, 48_000, 0);
      const s = withFade.samples;
      const xf = Math.round(0.25 * 48_000);

      expect(s.length).toBe(30 * 48_000);
      // Sanity that skipping the crossfade leaves a real seam, checked across several seeds rather
      // than one: the buffer's two ends are uncorrelated samples, so for any single fixed seed the
      // wrap step can land small by chance (this is what happened at seed 21 for one colour) even
      // though a seam is present on average. A multi-seed majority is the honest version of this
      // check; a single-seed threshold is not a real product invariant, just test self-verification.
      const seeds = [7, 13, 21, 55, 89];
      const exceedsCount = seeds.filter((seed) => {
        const unfaded = buildNoiseBuffer(color, seed, 30, 48_000, 0);
        return (
          wrapValueStep(unfaded.samples) >
          meanAbsStep(unfaded.samples, xf + 1, unfaded.samples.length)
        );
      }).length;
      expect(exceedsCount).toBeGreaterThanOrEqual(Math.ceil(seeds.length / 2));

      const interior = meanAbsStep(s, xf + 1, s.length);
      expect(wrapValueStep(s)).toBeLessThan(interior * 6);
      expect(wrapSlopeJump(s)).toBeLessThan(interior);
    }
  });

  it.each(['brown', 'pink'] as const)('keeps every %s sample inside the [-1,1] buffer domain', (color) => {
    const { samples } = buildNoiseBuffer(color, 4242);
    let min = Infinity;
    let max = -Infinity;
    for (const v of samples) {
      if (v < min) min = v;
      if (v > max) max = v;
    }
    expect(min).toBeGreaterThanOrEqual(-1);
    expect(max).toBeLessThanOrEqual(1);
  });

  it('gives brown strictly less high-frequency energy than pink (the colours are real)', () => {
    // crude spectral proxy: mean squared first difference over mean square. Higher = brighter.
    const hf = (a: Float64Array) => {
      let d = 0;
      let e = 0;
      for (let i = 1; i < a.length; i++) {
        const x = (a[i] as number) - (a[i - 1] as number);
        d += x * x;
        e += (a[i] as number) * (a[i] as number);
      }
      return d / e;
    };
    const brown = hf(buildNoiseBuffer('brown', 1234).samples);
    const pink = hf(buildNoiseBuffer('pink', 1234).samples);
    expect(brown).toBeLessThan(pink);
    expect(pink / brown).toBeGreaterThan(10); // measured ~87x — not a marginal difference
  });

  it('is deterministic for a fixed seed', () => {
    const a = buildNoiseBuffer('pink', 99, 1, 8000, 100);
    const b = buildNoiseBuffer('pink', 99, 1, 8000, 100);
    expect(Array.from(a.samples)).toEqual(Array.from(b.samples));
  });
});
