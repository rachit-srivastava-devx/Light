/**
 * The router used to send every conversation-steering utterance to `task`.
 *
 * Live report from a real voice session (2026-08-28):
 *
 *   "this is ok for a demo, but it does not have context of what I spoke previously... when I asked
 *    to 'let's create anything' it simply says what is the smallest part to start. not able to stop
 *    the conversation in between, divert or have a conversation. right now its just one by one
 *    messaging not a conversation completely"
 *
 * Probing `classifyIntakeUtterance` with those exact phrasings produced this (before):
 *
 *   task      <- "let's create anything"
 *   task      <- "can we just chat"
 *   task      <- "actually let's talk about something else"
 *   task      <- "wait stop"
 *   task      <- "hold on"
 *   task      <- "no not that"
 *   task      <- "forget it, different topic"
 *   converse  <- "help me clean the kitchen"      <- and inverted on a real task
 *
 * `task` loads `focus-companion.v1.md`, whose job is to name a smallest step. So asking the orb to
 * STOP produced a task step; so did asking it to change the subject. Three separate causes:
 *
 *   1. `"let's"` sat in `task_request_prefixes`, making every collaborative opening a task;
 *   2. these phrasings matched no marker list at all;
 *   3. `classifyIntakeUtterance`'s fall-through default was `'task'` — the wrong default for a
 *      product whose owner's rule is that discussing is the default behaviour.
 *
 * Every case below is one of those phrasings or its guard.
 */

import { describe, expect, it } from 'vitest';

import { classifyConversationControl, classifyIntakeUtterance } from './IntentClassifier';

describe('conversation control (the owner’s verbatim phrasings)', () => {
  it.each([
    ["let's create anything", 'converse'],
    ["let's create something together", 'converse'],
    ['can we just chat', 'converse'],
    ["actually let's talk about something else", 'converse'],
    ['wait stop', 'converse'],
    ['hold on', 'converse'],
    ['no not that', 'converse'],
    ['forget it, different topic', 'converse'],
    ["i've been thinking about ai lately", 'converse'],
  ])('routes %j to %s, not task', (utterance, expected) => {
    expect(classifyIntakeUtterance(utterance)).toBe(expected);
  });

  it.each([
    ['wait stop', 'stop'],
    ['hold on', 'stop'],
    ['stop', 'stop'],
    ['no not that', 'reject'],
    ['nope', 'reject'],
    ['forget it, different topic', 'divert'],
    ['never mind', 'divert'],
    ["let's talk about something else", 'divert'],
    ['can we just chat', 'chat_invitation'],
    ['what should we talk about', 'chat_invitation'],
  ])('labels %j as %s so the session can act on it', (utterance, kind) => {
    expect(classifyConversationControl(utterance)).toBe(kind);
  });

  it.each([
    // Whole-utterance matching for stop/reject: these words are common INSIDE real sentences, and
    // a substring match would turn a task and a clarification into control frames.
    ['stop by the shop and get milk'],
    ['no, the kitchen one'],
    ['i need to stop procrastinating'],
  ])('does not treat %j as a control frame', (utterance) => {
    expect(classifyConversationControl(utterance)).toBeNull();
  });
});

describe('real tasks still reach the task path (the regression this change risks)', () => {
  it.each([
    ['clean the kitchen'],
    ['pay the electricity bill'],
    ['write the quarterly report'],
    ['fix the login bug'],
    ['i need to pay the bill'],
    // `let's` + a real task verb is still a task; `let's` + anything else is not.
    ["let's clean the kitchen"],
    // The inversion: "help me <activity>" is naming work, and used to route to converse because the
    // word "help" matched the `stuck` rule and vetoed the task prefix.
    ['help me clean the kitchen'],
    // A bare short noun phrase is how people name work, and it is what CLARIFY exists for.
    ['taxes'],
    ['quarterly report'],
  ])('routes %j to task', (utterance) => {
    expect(classifyIntakeUtterance(utterance)).toBe('task');
  });

  it('publishes the denominator for the task-side regression set', () => {
    const tasks = [
      'clean the kitchen',
      'pay the electricity bill',
      'write the quarterly report',
      'fix the login bug',
      'i need to pay the bill',
      "let's clean the kitchen",
      'help me clean the kitchen',
      'taxes',
      'quarterly report',
    ];
    const routed = tasks.filter((t) => classifyIntakeUtterance(t) === 'task');
    expect(routed.length, `${routed.length}/${tasks.length} task utterances reached the task path`)
      .toBe(tasks.length);
  });
});

describe('the unclassifiable-script gap is explicit, not silent', () => {
  it('keeps non-Latin intake on the prior task path rather than silently taking the new default', () => {
    // Every marker list in this file is English or Latin-transliterated Hinglish, so Devanagari
    // matches nothing. Routing it to the new `converse` default would be a different guess, not a
    // better one, and it would have silently changed behaviour for a Hindi-speaking user.
    expect(classifyIntakeUtterance('मुझे टैक्स भरना है')).toBe('task');
  });
});

describe('only `stop` is an interruption', () => {
  /**
   * `App.tsx`'s transcript handler cuts audio on `stop` alone. This pins the discrimination,
   * because widening it later would make ordinary disagreement feel like an error: cutting playback
   * on "no, not that" or "let's talk about something else" turns a conversational move into an
   * interruption.
   */
  it.each([
    ['wait stop', 'stop', true],
    ['hold on', 'stop', true],
    ['stop', 'stop', true],
    ['no not that', 'reject', false],
    ['nope', 'reject', false],
    ['forget it, different topic', 'divert', false],
    ['can we just chat', 'chat_invitation', false],
  ])('%j is %s — interrupts playback: %s', (utterance, kind, interrupts) => {
    const control = classifyConversationControl(utterance);
    expect(control).toBe(kind);
    expect(control === 'stop').toBe(interrupts);
  });

  it('an utterance with no control label never interrupts', () => {
    for (const utterance of ['my kitchen is a disaster', 'clean the kitchen', 'stop by the shop']) {
      expect(classifyConversationControl(utterance)).not.toBe('stop');
    }
  });
});
