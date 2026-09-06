//made with a0.dev
import React, { useEffect, useRef, useState } from 'react';
import {
  Canvas,
  Circle,
  LinearGradient,
  BlurMask,
  Group,
  rect,
  rrect,
  vec,
} from '@shopify/react-native-skia';
import { StyleSheet, View, ViewStyle } from 'react-native';

import { MIST_DRIFT_RADIUS_FRACTION_MAX } from './motion';

/**
 * Rotates (x, y) by `angle` radians around (cx, cy). Used to carry the gradient direction and the
 * cloud puffs around the circle instead of rotating the `<Canvas>` view itself — see the note on
 * `rotation` inside the component for why that distinction matters.
 */
function rotateAround(x: number, y: number, cx: number, cy: number, angle: number): { x: number; y: number } {
  const dx = x - cx;
  const dy = y - cy;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  return { x: cx + dx * cos - dy * sin, y: cy + dx * sin + dy * cos };
}

interface AuroraMistProps {
  /** Diameter of the circular mist in pixels */
  size: number;
  /** Global multiplier – higher => faster drift & rotation */
  speed?: number;
  /** Slow state-specific breath period, in milliseconds. */
  breathPeriodMs?: number;
  /** State-specific breath depth as a fractional scale delta. */
  breathDepth?: number;
  /** Soft aura strength, kept low for calm degraded/error states. */
  glow?: number;
  /** [deepBlue, midBlue, white] */
  colors?: readonly [string, string, string];
  style?: ViewStyle;
}

/**
 * AuroraMist – procedural, blurred gradient that approximates
 * the ethereal cloud-in-a-circle reference. Pure Skia primitives, no custom shader.
 */
export const AuroraMist: React.FC<AuroraMistProps> = ({
  size = 200,
  speed = 5,
  breathPeriodMs = 6_000,
  breathDepth = 0.05,
  glow = 0.24,
  colors = ['#0066FF', '#80CCFF', '#FFFFFF'],
  style,
}) => {
  const half = size / 2;

  /**
   * Local time state – updated via requestAnimationFrame.
   * We purposely avoid Reanimated / Skia value hooks to keep the
   * implementation compatible with the minimal runtime.
   */
  const [time, setTime] = useState(0);
  const raf = useRef<number | undefined>(undefined);

  useEffect(() => {
    let start = Date.now();
    let lastUpdateMs = 0;
    // Throttled to ~30fps (not the native 60fps rAF rate): this orb is a slow ambient drift, not
    // motion that needs 60fps, and the extra re-render pressure measurably raises the odds of
    // hitting a live-observed react-native-skia native crash (SkPicture ref-count abort in
    // RNSkPictureRenderer::performDraw, seen 2026-08-06) that stems from the JS-driven redraw rate
    // racing the native picture teardown. This halves that rate as a mitigation — the underlying
    // race lives in RNSkia's native code, not here, so this reduces rather than eliminates it.
    const FRAME_INTERVAL_MS = 33;

    const loop = () => {
      const now = Date.now();
      const elapsed = now - start;
      if (elapsed - lastUpdateMs >= FRAME_INTERVAL_MS) {
        lastUpdateMs = elapsed;
        setTime(elapsed);
      }
      raf.current = requestAnimationFrame(loop);
    };

    raf.current = requestAnimationFrame(loop);

    return () => {
      if (raf.current) cancelAnimationFrame(raf.current);
    };
  }, []);

  // Puff drift excursion capped at MIST_DRIFT_RADIUS_FRACTION_MAX (motion.ts) — was ~0.25/0.22/0.24
  // (independently chosen, ~25% of radius), which the motion agent flagged as a large-excursion
  // ambient motion that involuntarily captures visual attention (see motion.ts's evidence-tagged
  // rationale). All three puffs now share the same, smaller cap instead of three separate values.
  const drift1 = Math.sin(time * 0.00025 * speed) * half * MIST_DRIFT_RADIUS_FRACTION_MAX;
  const drift2 = Math.sin(time * 0.00018 * speed + 1) * half * MIST_DRIFT_RADIUS_FRACTION_MAX;
  const drift3 = Math.cos(time * 0.00022 * speed + 0.5) * half * MIST_DRIFT_RADIUS_FRACTION_MAX;

  // `rotation` carries the dark (bottom) area of the gradient around the circle. This used to be
  // applied as an RN `transform: rotate` on the whole <Canvas> below, which rotates the Skia
  // surface's own square bounding box relative to the *static* circular clip on the parent
  // `<View>` (`overflow: hidden` + `borderRadius: half`, in `styles.container` below). A square
  // and a circle of the same "radius" only coincide on the square's four axis-aligned edges — along
  // its rotating diagonals the square reaches farther than the circle. The blurred aura and cloud
  // puffs are sized to reach close to the canvas edge, so as the square rotated, its corners let a
  // sliver of that blur leak past the intended circular silhouette — a rotating, faceted, non-
  // spherical edge, confirmed on-device (see apps/mobile/evidence/). Rotating the gradient/puff
  // *positions* in math instead (via `rotateAround` above) reproduces the same motion while leaving
  // the canvas itself un-rotated and always axis-aligned with its static parent mask.
  const rotation = ((time * 0.00005) * speed) % (2 * Math.PI); // radians
  const breath = 1 + Math.sin((time / breathPeriodMs) * 2 * Math.PI) * breathDepth;
  const auraOpacity = Math.min(0.28, 0.08 + glow * 0.28);

  const gradientStart = rotateAround(half, 0, half, half, rotation);
  const gradientEnd = rotateAround(half, size, half, half, rotation);
  const puff1Center = rotateAround(half * 0.4 + drift1, half * 0.55 + drift2, half, half, rotation);
  const puff2Center = rotateAround(half * 1.2 + drift2, half * 0.45 + drift3, half, half, rotation);
  const puff3Center = rotateAround(half + drift3, half * 0.2 + drift1, half, half, rotation);

  return (
    <View
      style={[
        styles.container,
        {
          width: size,
          height: size,
          borderRadius: half,
        },
        style,
      ]}
    >
      <Canvas style={{ flex: 1, transform: [{ scale: breath }] }}>
        {/*
         * Measured defect (apps/mobile/evidence/burst1_predrift/, analyzed with
         * scripts/orb-shape-gate.mjs): without this clip, the blurred aura/puffs are only kept
         * circular by the parent `<View>`'s native `overflow:hidden` + `borderRadius:half` mask
         * (below). That works when this Canvas renders at its native 1:1 size, but the `breath`
         * scale above (up to +10% in production presets — see AppModel.ts's `listening`/`success`
         * breathDepth) transforms this *already-rendered* Canvas layer, and the OS's cross-layer
         * mask does not reliably re-clip a scaled hardware (Skia/Metal) child to the parent's
         * static, unscaled circle: a burst of 14 frames in one app state showed the shape gate
         * oscillating between ~0.1% deviation (round) and up to 4.0% (visibly faceted) in sync
         * with the breath cycle, with facets recurring at the SAME fixed screen angles every time
         * (not moving/rotating) — the signature of this mismatch, not of the rotation bug fixed
         * above.
         *
         * Fix: clip to a perfect circle *inside Skia's own vector rendering*, before the `breath`
         * transform ever runs. A circle clipped by another circle is still a circle at any scale,
         * so this holds regardless of how precisely the native layer mask tracks a transformed
         * child. This clip lands at exactly `half` — the same radius the parent's mask already
         * enforces at rest (breath≈1, where this file's own gate measurements read ~0.1%
         * deviation) — so it changes nothing about the intended look, only makes that look hold at
         * every breath phase, not just when the scale happens to be near 1.
         */}
        <Group clip={rrect(rect(0, 0, size, size), half, half)}>
          {/* A restrained outer aura makes state changes legible without alarm-style flashing. */}
          <Circle c={vec(half, half)} r={half * (1 + glow * 0.08)} color={colors[0]} opacity={auraOpacity}>
            <BlurMask blur={28 + glow * 18} style="normal" />
          </Circle>

          {/* Main circular gradient */}
          <Circle c={vec(half, half)} r={half}>
            <LinearGradient
              start={vec(gradientStart.x, gradientStart.y)}
              end={vec(gradientEnd.x, gradientEnd.y)}
              colors={[colors[2], colors[1], colors[0]]}
            />
          </Circle>

          {/* Overlay blurred white puffs to mimic clouds */}
          <Circle c={vec(puff1Center.x, puff1Center.y)} r={half * 0.65} color={colors[2]}>
            <BlurMask blur={25} style="normal" />
          </Circle>

          <Circle c={vec(puff2Center.x, puff2Center.y)} r={half * 0.7} color={colors[2]}>
            <BlurMask blur={28} style="normal" />
          </Circle>

          <Circle c={vec(puff3Center.x, puff3Center.y)} r={half * 0.85} color={colors[2]}>
            <BlurMask blur={22} style="normal" />
          </Circle>
        </Group>
      </Canvas>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    overflow: 'hidden', // ensures gradient is clipped to the circular container
    backgroundColor: 'transparent',
  },
});

export default AuroraMist;
