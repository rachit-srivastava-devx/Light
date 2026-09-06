import { describe, expect, it } from 'vitest';
import { InvalidAtomizerOutputError, StepGate, type AtomizerOutput } from './StepGate';

function makeOutput(n: number): AtomizerOutput {
  return {
    steps_total: n,
    steps: Array.from({ length: n }, (_, i) => ({
      step_text: `step ${i + 1}`,
      est_min: 5,
      done_signal: `did step ${i + 1}`,
    })),
  };
}

describe('StepGate construction', () => {
  it('accepts a valid AtomizerOutput and starts before the first step', () => {
    const gate = new StepGate(makeOutput(3));
    expect(gate.steps_total).toBe(3);
    expect(gate.currentIndex).toBe(0);
    expect(gate.currentStep).toBeNull();
  });

  it('rejects steps_total outside [1, 12]', () => {
    expect(() => new StepGate(makeOutput(0))).toThrow(InvalidAtomizerOutputError);
    expect(() => new StepGate({ steps_total: 13, steps: makeOutput(13).steps })).toThrow(
      InvalidAtomizerOutputError,
    );
  });

  it('rejects a steps array whose length does not match steps_total', () => {
    const bad = makeOutput(3);
    expect(
      () => new StepGate({ steps_total: 4, steps: bad.steps }),
    ).toThrow(InvalidAtomizerOutputError);
  });
});

describe('StepGate — index-only navigation (INV2)', () => {
  it('goTo presents the step at that index by reference, never generating text', () => {
    const output = makeOutput(3);
    const gate = new StepGate(output);
    const step = gate.goTo(2);
    expect(step).toBe(output.steps[1]); // same object reference — not regenerated
    expect(gate.currentIndex).toBe(2);
    expect(gate.currentStep).toBe(output.steps[1]);
  });

  it('advance moves exactly one index forward', () => {
    const gate = new StepGate(makeOutput(3));
    gate.goTo(1);
    const next = gate.advance();
    expect(gate.currentIndex).toBe(2);
    expect(next.step_text).toBe('step 2');
  });

  it('rewind moves exactly one index backward', () => {
    const gate = new StepGate(makeOutput(3));
    gate.goTo(2);
    const prev = gate.rewind();
    expect(gate.currentIndex).toBe(1);
    expect(prev.step_text).toBe('step 1');
  });

  it('isLastStep is true only at steps_total', () => {
    const gate = new StepGate(makeOutput(3));
    gate.goTo(2);
    expect(gate.isLastStep).toBe(false);
    gate.goTo(3);
    expect(gate.isLastStep).toBe(true);
  });

  it('goTo rejects index 0, negative, non-integer, and above steps_total', () => {
    const gate = new StepGate(makeOutput(3));
    expect(() => gate.goTo(0)).toThrow(RangeError);
    expect(() => gate.goTo(-1)).toThrow(RangeError);
    expect(() => gate.goTo(1.5)).toThrow(RangeError);
    expect(() => gate.goTo(4)).toThrow(RangeError);
  });

  it('advance throws past the last step rather than wrapping or clamping', () => {
    const gate = new StepGate(makeOutput(2));
    gate.goTo(2);
    expect(() => gate.advance()).toThrow(RangeError);
  });

  it('rewind throws below the first step rather than wrapping or clamping', () => {
    const gate = new StepGate(makeOutput(2));
    gate.goTo(1);
    expect(() => gate.rewind()).toThrow(RangeError);
  });
});
