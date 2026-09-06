import React, { useEffect, useRef, useState } from 'react';
import {
  AccessibilityInfo,
  Animated,
  Easing,
  StyleSheet,
  View,
  type StyleProp,
  type ViewStyle,
} from 'react-native';

import type { CloudVoiceState, OrbVisualModel } from '../AppModel';
import { AuroraMist } from './AuroraMist';
import {
  BASE_SPEED_BY_STATE,
  BREATH_PULSE_PERIOD_MS,
  DEFAULT_BREATH_DEPTH,
  DEFAULT_BREATH_PERIOD_MS,
  DEFAULT_GLOW,
  effectiveBreathDepth,
  effectiveMistSpeed,
  pulseOpacityRange,
  pulseScaleRange,
  resolvePulseMotionState,
  type PulseMotionState,
} from './motion';

export interface CloudOrbProps {
  readonly state: CloudVoiceState;
  readonly volume: number;
  readonly color: string;
  readonly opacity: number;
  readonly scale: number;
  readonly visual?: OrbVisualModel;
  readonly style?: StyleProp<ViewStyle>;
  /**
   * True only when the native mic is actually capturing real audio frames (see App.tsx's
   * `micStatus` — gated on the first real frame received, not on permission grant or engine
   * start alone). Drives a brighter, pulsing look independent of `state`/`volume`, which come
   * from the session FSM and server-set intensity and previously gave no signal a user could
   * trust as "the mic is really on right now."
   */
  readonly micListening?: boolean;
}

/**
 * True while the OS accessibility "reduce motion" setting is on. Event-driven (subscribe once,
 * update only when the user actually flips the setting) — not a repeat of defect #1, which was
 * about updating React state at *animation* rate, not at user-setting-change rate.
 *
 * API grounded directly in the installed react-native@0.83 type declarations
 * (node_modules/react-native/types_generated/.../AccessibilityInfo.d.ts): both
 * `isReduceMotionEnabled()` and the `'reduceMotionChanged'` event exist on this version, and
 * `addEventListener` returns an `EventSubscription` with a `.remove()` method.
 */
function useReduceMotionEnabled(): boolean {
  const [reduceMotionEnabled, setReduceMotionEnabled] = useState(false);

  useEffect(() => {
    let mounted = true;
    void AccessibilityInfo.isReduceMotionEnabled().then((enabled) => {
      if (mounted) setReduceMotionEnabled(enabled);
    });
    const subscription = AccessibilityInfo.addEventListener('reduceMotionChanged', (enabled) => {
      setReduceMotionEnabled(enabled);
    });
    return () => {
      mounted = false;
      subscription.remove();
    };
  }, []);

  return reduceMotionEnabled;
}

/**
 * Drives the "mic is listening" breathing pulse as a single Animated.Value on the native thread,
 * instead of a React-state variable rewritten from a JS requestAnimationFrame loop.
 *
 * This is the fix for defect #1: the previous `usePulse` called `setPulse(...)` from a JS timer at
 * up to 20fps, so every tick re-rendered CloudOrb's whole tree (recomputing style objects and
 * handing AuroraMist new props) purely to animate a value React never needed for layout or
 * conditional rendering. `Animated.loop` + `useNativeDriver: true` moves the oscillation itself
 * onto the native side; this component only re-renders when `motionState` changes (mic starts or
 * stops listening, or the OS reduce-motion setting flips) — a handful of times per session, not
 * ~20 times a second.
 *
 * `react-native-reanimated` and Skia's own clock/useValue hooks were both considered first per the
 * task brief, and both were ruled out by checking, not assuming: `react-native-reanimated` is not
 * in node_modules (it is declared only as an *optional* peer dependency of
 * @shopify/react-native-skia, ">=4.0.0" with `peerDependenciesMeta.react-native-reanimated.optional
 * = true`, so npm does not install it — confirmed absent). The installed
 * @shopify/react-native-skia@2.10.1's own `animation` export is pure math helpers only
 * (interpolate/interpolateColors/interpolateVector/interpolatePaths) — the legacy
 * useValue/useClockValue/useComputedValue/useLoop hooks from Skia v1 do not exist in this version.
 * `Animated` from `react-native` core is the only animation driver actually present in this
 * toolchain today; it is not new to the app (react-native itself already is a dependency), just
 * not previously used here.
 */
function useBreathPulse(motionState: PulseMotionState): Animated.Value {
  const pulseAnim = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    if (motionState !== 'animating') {
      pulseAnim.stopAnimation();
      pulseAnim.setValue(0);
      return undefined;
    }
    const halfPeriodMs = BREATH_PULSE_PERIOD_MS / 2;
    const loop = Animated.loop(
      Animated.sequence([
        Animated.timing(pulseAnim, {
          toValue: 1,
          duration: halfPeriodMs,
          easing: Easing.inOut(Easing.sin),
          useNativeDriver: true,
        }),
        Animated.timing(pulseAnim, {
          toValue: 0,
          duration: halfPeriodMs,
          easing: Easing.inOut(Easing.sin),
          useNativeDriver: true,
        }),
      ]),
    );
    loop.start();
    return () => {
      loop.stop();
    };
  }, [motionState, pulseAnim]);

  return pulseAnim;
}

const MIST_SIZE = 220;

function clampVolume(volume: number): number {
  return Math.max(0, Math.min(volume, 1));
}

/**
 * Native port of the orb-ui cloud template: passive visual surface, app-controlled state/volume,
 * listening shrinks with input, speaking grows with output, and idle fades back. The cloud shape
 * itself is `AuroraMist` — a procedural Skia gradient, not flat Views — so it reads as mist/cloud
 * rather than three overlapping circles.
 */
export function CloudOrb({
  state,
  volume,
  color,
  opacity,
  scale,
  visual,
  style,
  micListening = false,
}: CloudOrbProps): React.ReactElement {
  const reduceMotionEnabled = useReduceMotionEnabled();
  const normalizedVolume = clampVolume(volume);
  const animation = visual?.animation ?? {
    speed: BASE_SPEED_BY_STATE[state],
    breathPeriodMs: DEFAULT_BREATH_PERIOD_MS,
    breathDepth: DEFAULT_BREATH_DEPTH,
    glow: DEFAULT_GLOW,
    colors: [color, color, '#ffffff'] as const,
  };
  const visualOpacity = visual?.opacity ?? opacity;
  const visualScale = visual?.scale ?? scale;
  const visualState = visual?.state;
  const pulseScale = visualState === 'success'
    ? 1 + normalizedVolume * 0.12
    : state === 'speaking'
      ? 1 + normalizedVolume * 0.08
      : state === 'listening'
        ? 1 - normalizedVolume * 0.05
        : 1;

  const motionState = resolvePulseMotionState({ micListening, reduceMotionEnabled });
  const breathPulse = useBreathPulse(motionState);

  // The state-driven parts (visualOpacity/visualScale/pulseScale) are plain numbers recomputed
  // only when props actually change — that's cheap and not the defect; frame-rate churn was the
  // problem, not re-rendering on a real state/volume change a few times a second.
  const baseScale = visualScale * pulseScale;

  // Full opacity/scale value the mic-listening pulse should oscillate between (or sit at, frozen,
  // under reduced motion). `pulseOpacityRange`/`pulseScaleRange` reproduce the pre-existing
  // Math.max/multiply formulas exactly (see motion.ts) — evaluated once from props here instead of
  // once per animation frame from state.
  const opacityRange = pulseOpacityRange(visualOpacity);
  const scaleRange = pulseScaleRange();

  // 'off': mic isn't listening — no wrapper effect at all, identical to the pre-existing baseline.
  // 'frozen': reduced motion — hold the pulse's low (resting) end so the "listening" brightening
  //   stays legible with zero oscillation, satisfying WCAG 2.2 SC 2.3.3 / 2.2.2's intent (see
  //   motion.ts) and the task's "orb must remain fully legible in every state" requirement.
  // 'animating': interpolate off the native-driven pulse.
  const wrapperOpacity: number | Animated.AnimatedInterpolation<number> =
    motionState === 'off'
      ? visualOpacity
      : motionState === 'frozen'
        ? opacityRange.low
        : breathPulse.interpolate({ inputRange: [0, 1], outputRange: [opacityRange.low, opacityRange.high] });

  const wrapperScale: number | Animated.AnimatedInterpolation<number> =
    motionState === 'animating'
      ? breathPulse.interpolate({ inputRange: [0, 1], outputRange: [scaleRange.low, scaleRange.high] })
      : scaleRange.low;

  // CloudOrb can't stop AuroraMist's own internal Skia redraw loop (different file, different
  // owner — see the report for what still needs to change there) but it does own these two props.
  // Driving both to their reduced-motion values collapses AuroraMist's drift/rotation/breath
  // formulas to constants, so the *visible* output goes still even though that file's timer keeps
  // running underneath.
  //
  // Deliberate behavior change from the pre-existing implementation: `speed` no longer gets a
  // `+ micPulse * 1.5` term while listening. That term read the same per-frame `micPulse` React
  // state this change removes (defect #1) — reproducing it would mean sampling the new native-
  // driven Animated.Value back into JS every frame via `addListener`, i.e. re-introducing the
  // exact per-frame-JS-callback pattern being fixed, just relocated. It also cut against defect
  // #2's finding: `speed` (AuroraMist's drift/rotation rate) would still climb from the mere fact
  // that the mic is on, independent of any real signal, for as long as `micListening` is true —
  // which is most of a work session. The mic-listening cue is now carried by the wrapper's own
  // opacity/scale pulse alone; AuroraMist's drift/rotation speed responds only to `state` and
  // actual `volume`, both real signals.
  const rawSpeed = animation.speed + normalizedVolume * 2;
  const speed = effectiveMistSpeed(rawSpeed, reduceMotionEnabled);
  const breathDepth = effectiveBreathDepth(animation.breathDepth, reduceMotionEnabled);

  return (
    <View
      accessible
      accessibilityRole="image"
      accessibilityLabel={`Focus Orb ${visual?.state ?? state}${micListening ? ', mic listening' : ''}`}
      testID="focus-orb-cloud"
      style={[styles.stage, style]}
    >
      <Animated.View style={{ opacity: wrapperOpacity, transform: [{ scale: wrapperScale }] }}>
        <AuroraMist
          size={MIST_SIZE}
          speed={speed}
          breathPeriodMs={animation.breathPeriodMs}
          breathDepth={breathDepth}
          glow={animation.glow}
          colors={animation.colors}
          style={{ transform: [{ scale: baseScale }] }}
        />
      </Animated.View>
    </View>
  );
}

const styles = StyleSheet.create({
  stage: {
    width: 260,
    height: 220,
    alignItems: 'center',
    justifyContent: 'center',
  },
});
