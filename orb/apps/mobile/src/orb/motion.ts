/**
 * motion.ts — named, evidence-tagged constants and pure decision math for the Focus Orb's ambient
 * animation. Every timing/amplitude number the orb animates with lives here, not inline in
 * CloudOrb.tsx or AuroraMist.tsx, so a number and its justification can never drift apart.
 *
 * Tags follow ~/.claude/skills/adhd-conversation-design/SKILL.md's convention exactly:
 *   [strong]   replicated experimental / meta-analytic support, or a normative spec (WCAG).
 *   [moderate] consistent findings, narrower base, or adjacent-population evidence.
 *   [thin]     design heuristic or theory — informs judgment, never a hard control.
 *
 * This file is intentionally free of `react` / `react-native` VALUE imports so every function in
 * it is plain-Node testable under vitest with no RN shimming (see motion.test.ts). The one type-only
 * import below (`CloudVoiceState`) is erased at build time and pulls in no runtime module. CloudOrb
 * .tsx is the only consumer; it owns the RN-specific plumbing (AccessibilityInfo, Animated.Value).
 */

import type { CloudVoiceState } from '../AppModel';

// ---------------------------------------------------------------------------
// Breathing pace — replaces the old PULSE_PERIOD_MS (1400ms, ~43 cycles/min)
// ---------------------------------------------------------------------------

/** Converts a breathing rate to a full-cycle period in milliseconds. Pulled out on its own so the
 * provenance of BREATH_PULSE_PERIOD_MS is checkable in code, not just asserted in a comment. */
export function breathsPerMinuteToPeriodMs(breathsPerMinute: number): number {
  return 60_000 / breathsPerMinute;
}

/**
 * One full pulse cycle (dim-to-bright-to-dim), in ms, for the "mic is listening" ambient pulse.
 *
 * [strong] Resonance/coherent breathing — slow paced breathing at ~4.5-7 breaths/min (commonly
 * centered on ~5.5-6/min) maximizes respiratory sinus arrhythmia and produces measurable autonomic
 * shifts in controlled studies: Steffen et al., "The Impact of Resonance Frequency Breathing on
 * Measures of HRV, Blood Pressure, and Mood" (PMC5575449,
 * https://www.ncbi.nlm.nih.gov/pmc/articles/PMC5575449/); a resonance-breathing RCT on HRV and
 * cognition (PMC8924557, https://www.ncbi.nlm.nih.gov/pmc/articles/PMC8924557/). We pick 6
 * breaths/min — the fast edge of that band — so the pulse still reads as "alive" rather than
 * glacial, while remaining a rate a human can actually entrain to.
 *
 * A slower alternative considered: box breathing (4-4-4-4s = 16s/cycle ≈ 3.75 breaths/min) is
 * [moderate] — physiologically consistent with the same slow-paced-breathing mechanism, but most
 * sourcing for the specific 4-4-4-4 count is practitioner/wellness writing, not RCTs.
 *
 * This REPLACES a prior 1400ms constant (`PULSE_PERIOD_MS`) that was ~43 cycles/min: not a
 * physiologically possible breath rate at all, just a fast decorative tick that a code comment
 * mislabeled as "breathing." At 43/min it also maximized on-screen motion during exactly the
 * moments (long mic-open, low-volume stretches) an ADHD user is most likely trying to hold focus,
 * not be pulled from it — see the attentional-capture evidence below.
 */
export const BREATH_PULSE_PERIOD_MS = breathsPerMinuteToPeriodMs(6);

// ---------------------------------------------------------------------------
// Pulse amplitude — carried over from the pre-existing implementation
// ---------------------------------------------------------------------------

/**
 * [thin] Design judgment, not a literature-derived number. Carried over unchanged from the
 * pre-existing implementation because defect review found no evidence this specific amplitude
 * (a 0.25 absolute swing in opacity, off a floor of 0.5) needed reducing — unlike AuroraMist's
 * ~25%-of-radius puff drift (see MIST_DRIFT_RADIUS_FRACTION_MAX below), this was never flagged as
 * visually excessive. Kept modest and named here so it's no longer a bare magic number.
 */
export const PULSE_OPACITY_FLOOR = 0.5;
export const PULSE_OPACITY_DEPTH = 0.25;

/** [thin] Same provenance as the opacity constants above: a small (6%), pre-existing, unflagged
 * amplitude, now named instead of inlined. */
export const PULSE_SCALE_DEPTH = 0.06;

// ---------------------------------------------------------------------------
// AuroraMist puff-drift amplitude — CloudOrb does not render this; AuroraMist.tsx does.
// ---------------------------------------------------------------------------

/**
 * Maximum puff-drift excursion, as a fraction of the mist's own radius, that AuroraMist.tsx's
 * drift1/drift2/drift3 should allow (its current inline values are ~0.25 / 0.22 / 0.24 — see the
 * "Allow puffs to traverse up to ~25% of the radius so motion is visible" comment on that file).
 *
 * This file cannot change AuroraMist.tsx's rendering (different owner — geometry work is in
 * progress there); this constant exists so that whoever wires it up imports one evidence-backed
 * number instead of three independently-chosen ones.
 *
 * Basis, and an honest note on its limits:
 * - [strong] Motion — especially larger, faster excursions — captures visual attention
 *   involuntarily and pre-attentively in the general population (Yantis & Jonides 1984;
 *   Jonides & Yantis 1988, "Uniqueness of abrupt visual onset in capturing attention"; replicated
 *   through Abrams & Christ 2003 "Motion onset captures attention"). This is not ADHD-specific —
 *   it is a baseline property of the visual system that a bigger excursion makes an ambient
 *   element more likely to pull focus, independent of diagnosis.
 * - [moderate] ADHD-specific distractibility to task-irrelevant stimuli is real but narrower and
 *   more mixed than popular framing suggests: some studies report heightened pull toward
 *   irrelevant on-screen regions (eye-tracking: PMC9599566); others find covert spatial attention
 *   (including involuntary orienting) intact in adults with ADHD (PMC5971124, "When attention is
 *   intact in adults with ADHD"), and a 2023 dominance-analysis study (PMC10421702, n=1289) found
 *   internally generated mind-wandering dominates over *external* distraction for most ADHD
 *   symptoms. Net: treat "ambient motion costs some attention" as plausible, not as a proven,
 *   ADHD-specific, large effect — hence reducing rather than eliminating drift.
 * - [thin] Calm Technology (Weiser & Brown 1995, "Designing Calm Technology") — peripheral
 *   information should stay low-amplitude and shift to the center of attention only when it
 *   matters. A design heuristic, not an experimental finding; used only as rationale, never as a
 *   hard control.
 *
 * The exact fraction below is a judgment call informed by the above, not a number any cited study
 * measured directly: reduced to well under half of the original ~0.25, comfortably away from
 * "clearly visible drift" while still reading as alive at normal (non-reduced-motion) speed.
 */
export const MIST_DRIFT_RADIUS_FRACTION_MAX = 0.1;

// ---------------------------------------------------------------------------
// Reduced motion — WCAG + the OS-level signal RN exposes for it
// ---------------------------------------------------------------------------

/**
 * [strong] WCAG 2.2 SC 2.3.3 "Animation from Interactions" (AAA) — non-essential motion should be
 * disableable: https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html
 * [strong] WCAG 2.2 SC 2.2.2 "Pause, Stop, Hide" (Level A, baseline conformance) — content that
 * moves, starts automatically, and runs longer than 5 seconds needs a pause/stop/hide mechanism
 * unless essential: https://www.w3.org/WAI/WCAG22/Understanding/pause-stop-hide.html. The orb's
 * ambient pulse/drift/breath is exactly this shape (auto-starting, indefinite, non-essential), so
 * honoring the OS-level reduce-motion signal is the pause mechanism we rely on (see "what this
 * does NOT do" in the accompanying report for the gap: no separate in-app toggle is implemented).
 * Both criteria are about non-essential *decorative* motion, not the seizure-risk flashing
 * criteria (WCAG 2.3.1, >3 flashes/sec) — our pulse, even before this change, was ~0.7Hz, nowhere
 * near that threshold; attention/calm is the actual concern here, not photosensitivity.
 *
 * This is a plain vestibular/cognitive-accessibility fact rather than an ADHD-specific finding —
 * vestibular disorders are a distinct population from ADHD and this file does not conflate them —
 * but honoring the OS setting costs nothing for users who don't need it and is table stakes for
 * users who do.
 */
export type PulseMotionState = 'animating' | 'frozen' | 'off';

export interface PulseMotionInput {
  /** True only while the mic is actually capturing (see CloudOrbProps.micListening). */
  readonly micListening: boolean;
  /** AccessibilityInfo.isReduceMotionEnabled() / the 'reduceMotionChanged' event, current value. */
  readonly reduceMotionEnabled: boolean;
}

/**
 * Decides whether the mic-listening pulse should animate, freeze at a legible resting value, or
 * not run at all. Pure and RN-free on purpose: this is "the reduce-motion decision" the task asks
 * to be unit-tested, independent of Animated/AccessibilityInfo plumbing.
 *
 * - 'off': mic isn't listening — no pulse, exactly as before this change (defect #2 was never
 *   "the orb always animates in every state"; usePulse's `active` gate on micListening was already
 *   real and is preserved here).
 * - 'frozen': mic is listening but the OS asked for reduced motion — hold at the resting/floor
 *   value (still legible: the state is still visually distinguishable) with zero oscillation.
 * - 'animating': mic is listening and motion is allowed — run the breathing pulse.
 */
export function resolvePulseMotionState({ micListening, reduceMotionEnabled }: PulseMotionInput): PulseMotionState {
  if (!micListening) return 'off';
  return reduceMotionEnabled ? 'frozen' : 'animating';
}

/** Endpoints (0 = resting, 1 = peak-of-pulse) that a single opacity/scale channel should
 * interpolate between. Pulling this into one function means CloudOrb hands Animated two numbers
 * once per prop-change instead of recomputing `Math.max` every animation frame from React state —
 * the actual fix for defect #1 (setPulse driving a re-render at animation rate). */
export interface PulseRange {
  readonly low: number;
  readonly high: number;
}

/** Reproduces the pre-existing `Math.max(visualOpacity, floor [+ depth])` floor-and-lift shape
 * exactly, just evaluated once from props rather than every frame from state. */
export function pulseOpacityRange(baseOpacity: number): PulseRange {
  return {
    low: Math.max(baseOpacity, PULSE_OPACITY_FLOOR),
    high: Math.max(baseOpacity, PULSE_OPACITY_FLOOR + PULSE_OPACITY_DEPTH),
  };
}

/** Reproduces the pre-existing `visualScale * (1 + micPulse * depth)` shape as a pair of
 * endpoints for a *multiplier* layered on top of the state-driven base scale via nested
 * transforms (which compose multiplicatively), so the base scale itself never needs to be
 * recomputed at animation rate either. */
export function pulseScaleRange(): PulseRange {
  return { low: 1, high: 1 + PULSE_SCALE_DEPTH };
}

// ---------------------------------------------------------------------------
// Reduced motion applied to the AuroraMist props CloudOrb controls
// ---------------------------------------------------------------------------

/** CloudOrb cannot stop AuroraMist's own internal Skia redraw loop (different file, different
 * owner) but it does control the `speed` and `breathDepth` props passed into it. Driving both to
 * zero collapses AuroraMist's drift/rotation formulas (`sin(time * const * speed)`) and its breath
 * scale (`1 + sin(...) * breathDepth`) to constants, so the *visible* output goes still even
 * though the underlying timer keeps ticking. This is a mitigation, not a fix for that file's own
 * lack of reduce-motion awareness — see the report for what still needs to change in AuroraMist.tsx
 * itself. */
export const REDUCED_MOTION_SPEED = 0;
export const REDUCED_MOTION_BREATH_DEPTH = 0;

export function effectiveMistSpeed(baseSpeed: number, reduceMotionEnabled: boolean): number {
  return reduceMotionEnabled ? REDUCED_MOTION_SPEED : baseSpeed;
}

export function effectiveBreathDepth(baseBreathDepth: number, reduceMotionEnabled: boolean): number {
  return reduceMotionEnabled ? REDUCED_MOTION_BREATH_DEPTH : baseBreathDepth;
}

// ---------------------------------------------------------------------------
// Remaining inline numbers moved out of CloudOrb.tsx for "no magic numbers inline" completeness.
// Neither of these was one of the 4 reported defects; both are carried over with their pre-existing
// values, not re-derived, and are tagged [thin] honestly rather than backdated to evidence that
// wasn't used to choose them.
// ---------------------------------------------------------------------------

/**
 * [thin] Fallback animation preset used only when CloudOrb is rendered without a `visual` prop.
 * As of this change, App.tsx always supplies `visual={model.orbVisual}` — this path has no
 * production caller today (a scaffold/defensive default, per the L8 anti-slop rule that a module
 * without a real caller isn't a completed feature). Kept at its pre-existing values rather than
 * retro-fit to the coherent-breathing evidence above, since it costs a live user nothing either
 * way; flagged here rather than silently left as a bare object literal.
 */
export const DEFAULT_BREATH_PERIOD_MS = 6_000;
export const DEFAULT_BREATH_DEPTH = 0.05;
export const DEFAULT_GLOW = 0.24;

/**
 * [thin] Drift/rotation speed multiplier AuroraMist receives, keyed by voice state — livelier
 * while the orb is actively doing something, calmer at idle. Pre-existing values, carried over
 * unchanged; there is no research pinning these particular multiplier units (an arbitrary
 * radians/ms scale internal to AuroraMist's formulas, not a physical rate like bpm) to a specific
 * number, so this is named/relocated for "no magic numbers inline" but not re-derived.
 */
export const BASE_SPEED_BY_STATE: Readonly<Record<CloudVoiceState, number>> = {
  idle: 2.5,
  connecting: 4,
  listening: 5,
  thinking: 6,
  speaking: 7,
  error: 3,
};
