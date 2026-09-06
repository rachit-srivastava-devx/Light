import { describe, expect, it } from 'vitest';
import { detectCrisis } from './CrisisDetector';

describe('detectCrisis', () => {
  describe('true positives — clear, unambiguous crisis language', () => {
    it('detects first-person reflexive kill intent', () => {
      expect(detectCrisis('I want to kill myself')).toBe(true);
      expect(detectCrisis("I'm going to kill myself tonight")).toBe(true);
    });

    it('detects reflexive end-my-life / end-it-all phrasing', () => {
      expect(detectCrisis('I am going to end my life')).toBe(true);
      expect(detectCrisis('I just want to end it all')).toBe(true);
    });

    it('detects explicit means/plan statements', () => {
      expect(detectCrisis("I'm going to overdose")).toBe(true);
      expect(detectCrisis('I have pills to overdose')).toBe(true);
      expect(detectCrisis('I have a gun to end my life')).toBe(true);
    });

    it('detects "don\'t want to live/be alive anymore"', () => {
      expect(detectCrisis("I don't want to live anymore")).toBe(true);
      expect(detectCrisis("I don't want to be alive anymore")).toBe(true);
    });

    it('detects bare unqualified "I want to die"', () => {
      expect(detectCrisis('I want to die')).toBe(true);
      expect(detectCrisis('honestly I want to die')).toBe(true);
    });

    it('detects explicit suicide disclosures', () => {
      expect(detectCrisis('I tried to kill myself last week')).toBe(true);
      expect(detectCrisis("I'm suicidal")).toBe(true);
      expect(detectCrisis('I have a suicide plan')).toBe(true);
    });

    it('is case-insensitive', () => {
      expect(detectCrisis('I WANT TO KILL MYSELF')).toBe(true);
    });
  });

  describe('true negatives — hyperbolic/unrelated ADHD-frustration phrases', () => {
    it('does not trigger on task-attached "killing me" hyperbole', () => {
      expect(detectCrisis('this task is killing me')).toBe(false);
      expect(detectCrisis('this deadline is killing me')).toBe(false);
      expect(detectCrisis('ugh this essay is killing me')).toBe(false);
    });

    it('does not trigger on "I want to die" followed by a task/reason clause', () => {
      expect(detectCrisis('I want to die, this essay is never going to end')).toBe(false);
      expect(detectCrisis('I want to die because of this homework')).toBe(false);
    });

    it('does not trigger on non-reflexive or idiomatic kill/die phrasing', () => {
      expect(detectCrisis('I could kill this todo list')).toBe(false);
      expect(detectCrisis("I'm dying to finish this")).toBe(false);
    });

    it('does not trigger on ordinary avoidance/frustration language', () => {
      expect(detectCrisis("I'm so done with this")).toBe(false);
      expect(detectCrisis('I give up')).toBe(false);
      expect(detectCrisis('this is so hard I could scream')).toBe(false);
      expect(detectCrisis('I hate this task so much')).toBe(false);
    });

    it('does not trigger on empty or unrelated transcripts', () => {
      expect(detectCrisis('')).toBe(false);
      expect(detectCrisis('   ')).toBe(false);
      expect(detectCrisis('start with step one, open the document')).toBe(false);
      expect(detectCrisis('I am not done with this yet')).toBe(false);
    });
  });
});
