import React from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// This Node runtime has no requestAnimationFrame (browser/RN host API) — same gap
// AppSurface.test.tsx documents. setTimeout stands in so the effect's rAF loop can run; the
// `Date.now()` handed to the callback tracks vitest's fake clock installed in beforeEach below, so
// advancing fake time also advances what AuroraMist's loop sees as elapsed time.
(globalThis as typeof globalThis & { requestAnimationFrame?: (cb: (time: number) => void) => number }).requestAnimationFrame =
  (cb) => setTimeout(() => cb(Date.now()), 16) as unknown as number;
(globalThis as typeof globalThis & { cancelAnimationFrame?: (handle: number) => void }).cancelAnimationFrame =
  (handle) => clearTimeout(handle);

vi.mock('react-native', () => ({
  StyleSheet: {
    create: <T extends object>(styles: T) => styles,
  },
  View: 'View',
}));

// Same inert-stand-in mock as AppSurface.test.tsx / AppVoiceWiring.test.tsx (kept independent
// rather than shared, matching this repo's existing per-file mock convention). Group/rect/rrect
// are the Skia-level circular clip this file's breath-scale-vs-static-mask fix added.
vi.mock('@shopify/react-native-skia', () => ({
  Canvas: 'Canvas',
  Circle: 'Circle',
  LinearGradient: 'LinearGradient',
  BlurMask: 'BlurMask',
  Group: 'Group',
  rect: (x: number, y: number, width: number, height: number) => ({ x, y, width, height }),
  rrect: (r: unknown, rx: number, ry: number) => ({ rect: r, rx, ry }),
  vec: (x: number, y: number) => ({ x, y }),
}));

// eslint-disable-next-line import/first
import { AuroraMist } from './AuroraMist';

/**
 * Captures every prop that could visibly change frame to frame: the Canvas's own `breath` scale
 * transform, and every Circle's center/radius/opacity — the rotation-carried gradient direction
 * and the three puff drift positions all flow through Circle `c` props (see AuroraMist.tsx's
 * `rotateAround` and `drift1`/`drift2`/`drift3`). If any of those differ between two snapshots,
 * something visibly moved.
 */
function snapshot(renderer: TestRenderer.ReactTestRenderer) {
  const canvas = renderer.root.findByType('Canvas' as never);
  const circles = renderer.root.findAllByType('Circle' as never);
  return {
    canvasTransform: JSON.stringify((canvas.props as { style?: { transform?: unknown } }).style?.transform),
    // Only the scalar/vector props, not the whole `props` object: `props.children` holds nested
    // React elements (e.g. <BlurMask>) whose fiber `_owner` back-references are circular and
    // cannot be JSON.stringify'd, and are irrelevant to "did this circle move" anyway.
    circles: circles.map((c) => {
      const p = c.props as { c?: unknown; r?: unknown; color?: unknown; opacity?: unknown };
      return JSON.stringify({ c: p.c, r: p.r, color: p.color, opacity: p.opacity });
    }),
  };
}

describe('AuroraMist reduced-motion contract', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders a completely static image when speed=0 and breathDepth=0', () => {
    // This is the exact combination CloudOrb.tsx's effectiveMistSpeed/effectiveBreathDepth
    // (via motion.ts's REDUCED_MOTION_SPEED / REDUCED_MOTION_BREATH_DEPTH) drive in when the OS
    // reduce-motion setting is on — CloudOrb cannot stop this file's own rAF timer (different
    // owner), so the static-output guarantee has to hold here, in the formulas themselves.
    let renderer!: TestRenderer.ReactTestRenderer;
    act(() => {
      renderer = TestRenderer.create(
        <AuroraMist size={200} speed={0} breathDepth={0} breathPeriodMs={6_000} glow={0.24} />,
      );
    });

    const before = snapshot(renderer);

    // Advance well past many would-be animation frames (the file throttles state updates to
    // ~30fps) and past a full breath period, so a real leak would have every opportunity to show.
    act(() => {
      vi.advanceTimersByTime(10_000);
    });

    const after = snapshot(renderer);

    expect(after.canvasTransform).toBe(before.canvasTransform);
    expect(after.circles).toEqual(before.circles);
  });

  it('positive control: the same harness DOES detect motion when speed/breathDepth are non-zero', () => {
    // Proves the test above is a real assertion, not a false pass from the render tree being
    // frozen for some unrelated mocking reason (e.g. state updates never reaching the renderer).
    let renderer!: TestRenderer.ReactTestRenderer;
    act(() => {
      renderer = TestRenderer.create(
        <AuroraMist size={200} speed={5} breathDepth={0.05} breathPeriodMs={6_000} glow={0.24} />,
      );
    });

    const before = snapshot(renderer);

    act(() => {
      vi.advanceTimersByTime(10_000);
    });

    const after = snapshot(renderer);

    expect(after.circles).not.toEqual(before.circles);
  });
});
