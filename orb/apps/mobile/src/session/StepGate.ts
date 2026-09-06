/**
 * StepGate — holds the validated step list from a successful `AtomizerOutput` and exposes the
 * current step **by index only** (INV2: reference-not-generate, never regenerates or free-texts
 * a step).
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §2 (atomizer schema + INV2). The schema types now live
 * in `../shared/atomizer-schema.ts` — extracted at Tier 5 per `docs/adr/LESSONS.md` L5, once the
 * context pack and clarify protocol became the second and third consumers. They are re-exported
 * here so existing importers of this module keep working.
 *
 * `apps/mobile/src/session/contracts.ts`'s `SessionSnapshotBase.step_index` is 1-based ("no step
 * presented yet" is `0`); this gate mirrors that convention so the two stay in lockstep.
 */

export {
  EST_MIN_MAX,
  EST_MIN_MIN,
  STEP_TEXT_MAX_CHARS,
  STEPS_TOTAL_MAX,
  STEPS_TOTAL_MIN,
  type AtomizerOutput,
  type AtomizerStep,
} from '../shared/atomizer-schema';

import {
  STEPS_TOTAL_MAX,
  STEPS_TOTAL_MIN,
  type AtomizerOutput,
  type AtomizerStep,
} from '../shared/atomizer-schema';

/** Thrown when a `StepGate` is constructed from an `AtomizerOutput` that was never validated. */
export class InvalidAtomizerOutputError extends Error {
  constructor(detail: string) {
    super(`StepGate: invalid AtomizerOutput — ${detail}`);
    this.name = 'InvalidAtomizerOutputError';
  }
}

/**
 * Wraps an already-validated `AtomizerOutput` (the caller must have run it through
 * `validateAtomizerOutput` first — this class does not re-run schema validation, it only asserts
 * the invariants that make index arithmetic safe: `steps.length === steps_total` and
 * `steps_total` inside the published bounds).
 */
export class StepGate {
  private readonly steps: readonly AtomizerStep[];
  readonly steps_total: number;
  /** 1-based index into `steps`; `0` = no step presented yet (mirrors `SessionSnapshotBase`). */
  private index: number;

  constructor(output: AtomizerOutput) {
    if (output.steps_total < STEPS_TOTAL_MIN || output.steps_total > STEPS_TOTAL_MAX) {
      throw new InvalidAtomizerOutputError(
        `steps_total ${output.steps_total} outside [${STEPS_TOTAL_MIN}, ${STEPS_TOTAL_MAX}]`,
      );
    }
    if (output.steps.length !== output.steps_total) {
      throw new InvalidAtomizerOutputError(
        `steps.length ${output.steps.length} !== steps_total ${output.steps_total}`,
      );
    }
    this.steps = output.steps;
    this.steps_total = output.steps_total;
    this.index = 0;
  }

  /** 1-based; `0` before the first step has been presented. */
  get currentIndex(): number {
    return this.index;
  }

  /**
   * The step at `currentIndex`, by reference into the validated list — never generated or
   * free-texted (INV2). `null` before the first `advance()`/`goTo()` call, or once past the last
   * step.
   */
  get currentStep(): AtomizerStep | null {
    if (this.index < 1 || this.index > this.steps_total) return null;
    return this.steps[this.index - 1] ?? null;
  }

  /** Present a step by 1-based index. Rejects out-of-bounds indices rather than clamping — a
   * silent clamp would let a caller bug present the wrong step without ever noticing. */
  goTo(index: number): AtomizerStep {
    if (!Number.isInteger(index) || index < 1 || index > this.steps_total) {
      throw new RangeError(
        `StepGate.goTo: index ${index} out of bounds for steps_total ${this.steps_total}`,
      );
    }
    this.index = index;
    const step = this.steps[index - 1];
    if (!step) {
      throw new RangeError(`StepGate.goTo: no validated step at index ${index}`);
    }
    return step;
  }

  /** Advance to `currentIndex + 1`. Throws at the last step rather than wrapping/clamping — the
   * caller (StateMachine's `has_more_steps` guard) is responsible for checking first. */
  advance(): AtomizerStep {
    return this.goTo(this.index + 1);
  }

  /** Step back to `currentIndex - 1`. Throws below the first step. */
  rewind(): AtomizerStep {
    return this.goTo(this.index - 1);
  }

  /** `true` once `currentIndex` is on the last step — mirrors `has_more_steps`'s negation. */
  get isLastStep(): boolean {
    return this.index >= this.steps_total;
  }
}
