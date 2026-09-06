/**
 * Sweeps the lowpass cutoff to morph the bed brown↔pink as an ambient status channel, per
 * `docs/BUILD-DIGEST.md` §1: "Sweeps the lowpass cutoff to morph brown↔pink bed color as an
 * ambient status channel (idle/thinking/step-ready)."
 *
 * Only the `BiquadFilterNode`'s cutoff moves (§2: "the sweep is ColorMorph.ts's brown↔pink status
 * channel, so `type` is fixed at lowpass and only `frequency_hz` moves") — no buffer swap, no new
 * source node, so this never risks INV1's "bed never stops."
 *
 * Blueprint 02 §7 is the authority (the digest paraphrases it and drops two details, so read §7):
 *
 *   | Idle / working            | brown, low, steady   | the default line-open tone            |
 *   | Thinking (crew forming)   | morph toward pink    | sweep the lowpass cutoff up over ~400ms|
 *   | Step ready / win          | brief swell + chime  | scheduled gain bump + mixed cue        |
 *
 * Two consequences for this file:
 *  - **Only two cutoff positions exist.** "Step ready / win" is a *gain-domain* cue — a swell plus
 *    a mixed chime — not a third cutoff. Modelling it as a brighter cutoff would leave the bed
 *    permanently brighter after a step is presented, contradicting §7's "idle / working: brown"
 *    and §1's default. The swell belongs to a cue scheduler on the gain node, not here.
 *  - **The ~400 ms sweep is a spec number**, quoted from §7 — not this file's choice.
 *
 * REMAINING SPEC GAP (reviewer-accepted): §7 gives no cutoff *frequencies*. Brown noise energy
 * concentrates well under 1 kHz and pink is flat across the spectrum, so opening the cutoff reads
 * as "brightening toward pink" without swapping the source buffer. The two Hz values below are this
 * file's judgment; the sweep duration is not.
 */

import type { AudioTimeSeconds } from './contracts';
import type { AudioParamLike } from './AudioGraph';

/** §7 — the two colour positions of the bed. `step_ready` is a gain swell, not a colour (header). */
export type BedStatus = 'idle' | 'thinking';

/** SPEC GAP (see file header) — frequencies are judgment; ordering idle < thinking is §7. */
export const BED_STATUS_CUTOFF_HZ: Readonly<Record<BedStatus, number>> = {
  idle: 400,
  thinking: 1200,
};

/** §7 — "sweep the lowpass cutoff up over ~400 ms". A quoted spec number, not a guess. */
export const COLOR_MORPH_SWEEP_MS = 400;

/** Revised Track J hard deadline: stop-of-user-speech to the thinking morph being scheduled. */
export const THINKING_MORPH_ENGAGE_BUDGET_MS = 250;

/**
 * §2 scheduling primitive: `setTargetAtTime` (exponential approach), appropriate here because the
 * digest describes this as a continuous ambient "sweep," not a hard-bounded ramp like the duck.
 * Time constant is `sweepMs` scaled so the sweep is ~95% complete by `sweepMs` (3 time constants).
 */
export function morphTo(
  filterFrequency: AudioParamLike,
  status: BedStatus,
  atSeconds: AudioTimeSeconds,
  sweepMs: number = COLOR_MORPH_SWEEP_MS,
): AudioTimeSeconds {
  const target = BED_STATUS_CUTOFF_HZ[status];
  const timeConstant = sweepMs / 1000 / 3;
  filterFrequency.setTargetAtTime(target, atSeconds, timeConstant);
  return atSeconds + sweepMs / 1000;
}
