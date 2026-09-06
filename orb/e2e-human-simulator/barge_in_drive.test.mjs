#!/usr/bin/env node
// barge_in_drive.test.mjs
//
// Tests for e2e-human-simulator/barge_in_drive.mjs: the pure helper functions against hand-built
// inputs, PLUS one real subprocess run of the actual CLI's fail-closed path (relay unreachable),
// proving the shipped script's exit-code contract rather than a friendlier internal API — same
// precedent as scripts/voice-ux-gate.test.mjs and scripts/orb-shape-gate.test.mjs.
//
// This is a plain Node script, NOT a vitest suite, and that is deliberate, not an oversight:
// vitest.config.ts's `test.include` is scoped to apps/mobile/, backend/gateway-sidecar/, and
// backend/voice-provider-sidecar/ only — there is no `e2e-human-simulator/**` or `scripts/**` glob
// in it. scripts/voice-ux-gate.test.mjs already documented and verified this; not re-trusted on
// the comment alone here either — see this file's own subprocess test below, and the task report
// for the direct `node e2e-human-simulator/barge_in_drive.test.mjs` run.
//
// Deliberately NOT tested here: the money-spending happy path (real Fish TTS, real relay-rs
// trials). That is barge_in_drive.mjs's actual job and is exercised for real, against the live
// services, as part of the task this file was built for — not re-run on every test invocation,
// which would spend real credit every time this suite runs.
//
// Usage: node e2e-human-simulator/barge_in_drive.test.mjs

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  percentile, fmt, envInt, shortSid, UTTERANCE, BARGE_IN_YIELD_BUDGET_MS, MIN_USABLE_SAMPLES,
  deliveryUnderway, firstAckAfter, countTextTypes,
} from './barge_in_drive.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SCRIPT_PATH = join(__dirname, 'barge_in_drive.mjs');

let passCount = 0;
let failCount = 0;
function test(name, fn) {
  try {
    fn();
    passCount++;
    console.log(`  ok - ${name}`);
  } catch (err) {
    failCount++;
    console.log(`  FAIL - ${name}`);
    console.log(`    ${err.message}`);
  }
}

// --- percentile(): same nearest-rank method as scripts/voice-ux-gate.mjs's own G5 computation ---

test('percentile of an empty array is null (never a fabricated number)', () => {
  assert.equal(percentile([], 0.5), null);
  assert.equal(percentile([], 0.95), null);
});

test('percentile of a single-element array returns that element for any p', () => {
  assert.equal(percentile([42], 0), 42);
  assert.equal(percentile([42], 0.5), 42);
  assert.equal(percentile([42], 0.95), 42);
  assert.equal(percentile([42], 1), 42);
});

test('percentile uses nearest-rank on a sorted-ascending array, matching G5\'s own formula', () => {
  // Ten samples 1..10: p50 -> index floor(0.5*10)=5 -> value 6; p95 -> index floor(0.95*10)=9 -> 10.
  const arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
  assert.equal(percentile(arr, 0.5), 6);
  assert.equal(percentile(arr, 0.95), 10);
});

test('percentile never indexes past the array end (clamped, matching G5\'s Math.min guard)', () => {
  const arr = [10, 20, 30];
  assert.equal(percentile(arr, 0.999), 30);
  assert.equal(percentile(arr, 1), 30);
});

test('percentile does not require sortedness itself — caller\'s job, documented behavior', () => {
  // Not a claim it sorts for you: this just pins the actual (index-based) behavior on unsorted
  // input so a future refactor cannot silently change it without this test noticing.
  const arr = [5, 1, 9];
  assert.equal(percentile(arr, 0), 5);
});

test('with n=10 (this file\'s own default sample size), p95 and max always coincide',
  () => {
    // floor(0.95 * 10) === 9 === the last index of a 10-element array, every time. Documented here
    // because the real run's report shows p95===max for every n=10 frame type and that is this,
    // not a bug or a suspiciously perfect tail.
    const arr = Array.from({ length: 10 }, (_, i) => i * 137.3); // arbitrary but n=10
    assert.equal(percentile(arr, 0.95), arr[9]);
  });

// --- fmt() ----------------------------------------------------------------------------------

test('fmt renders null/undefined/NaN as "n/a", never as 0 or a fabricated number', () => {
  assert.equal(fmt(null), 'n/a');
  assert.equal(fmt(undefined), 'n/a');
  assert.equal(fmt(NaN), 'n/a');
});

test('fmt renders a real number to one decimal place with an "ms" suffix', () => {
  assert.equal(fmt(1897.73), '1897.7ms');
  assert.equal(fmt(0), '0.0ms');
  assert.equal(fmt(-1.3), '-1.3ms');
});

// --- envInt() ---------------------------------------------------------------------------------

test('envInt falls back when the env var is unset', () => {
  delete process.env.__BARGE_IN_TEST_VAR__;
  assert.equal(envInt('__BARGE_IN_TEST_VAR__', 7), 7);
});

test('envInt falls back on a non-numeric or non-positive value rather than producing 0 samples', () => {
  process.env.__BARGE_IN_TEST_VAR__ = 'not-a-number';
  assert.equal(envInt('__BARGE_IN_TEST_VAR__', 7), 7);
  process.env.__BARGE_IN_TEST_VAR__ = '0';
  assert.equal(envInt('__BARGE_IN_TEST_VAR__', 7), 7);
  process.env.__BARGE_IN_TEST_VAR__ = '-5';
  assert.equal(envInt('__BARGE_IN_TEST_VAR__', 7), 7);
  delete process.env.__BARGE_IN_TEST_VAR__;
});

test('envInt parses a valid positive integer from the environment', () => {
  process.env.__BARGE_IN_TEST_VAR__ = '25';
  assert.equal(envInt('__BARGE_IN_TEST_VAR__', 7), 25);
  delete process.env.__BARGE_IN_TEST_VAR__;
});

// --- shortSid() -------------------------------------------------------------------------------

test('shortSid strips this file\'s own session-id prefix/suffix scheme for table display', () => {
  // Mirrors the literal template runTrial() builds: `bargein-drive-${label}-${timing}-${trialIndex}-${RUN_ID}`.
  const runId = '1787931438836';
  const sid = `bargein-drive-barge_in-immediate-3-${runId}`;
  // shortSid only knows the module's OWN RUN_ID (fixed at import time), so this test checks the
  // shape it actually promises: the fixed 'bargein-drive-' prefix is gone either way.
  assert.ok(!shortSid(sid).startsWith('bargein-drive-'));
});

// --- module-level constants — pin the values the report and the budget comparison depend on ----

test('BARGE_IN_YIELD_BUDGET_MS is 100 — the product\'s own documented barge-in budget', () => {
  // blueprints/ADHD-Focus-Orb-L8-Deep-Dive/03-VOICE-LATENCY-PIPELINE.md:167,
  // backend/relay-rs/src/latency_budget.rs:18, AGENTS.md:88, CLAUDE.md:43 — all four agree: 100ms.
  assert.equal(BARGE_IN_YIELD_BUDGET_MS, 100);
});

test('MIN_USABLE_SAMPLES is 10 — the task\'s own fail-closed floor', () => {
  assert.equal(MIN_USABLE_SAMPLES, 10);
});

test('UTTERANCE is long enough to be a multi-second reply but short enough to stay cheap', () => {
  // Cheap per this product's own configured rate (143 paise/1000 chars,
  // backend/relay-py/src/orb_relay/app.py:229): assert the ACTUAL bound, not just describe it in a
  // comment. 80-400 chars is comfortably "a few sentences," not a paragraph.
  assert.ok(UTTERANCE.length >= 80, `UTTERANCE too short to produce a multi-second reply: ${UTTERANCE.length} chars`);
  assert.ok(UTTERANCE.length <= 400, `UTTERANCE long enough to get expensive at scale: ${UTTERANCE.length} chars`);
});

// --- subprocess: the shipped CLI's fail-closed contract, exercised for real, spending no money --

// --- delivery gating + yield-ack helpers (added 2026-08-28 with the relay barge-in fix) ---------
// `deliveryUnderway` is the fix for a real mis-measurement, not a nicety: when it did not exist,
// the trial loop opened its "burst ended" quiet window on ANY frame, and the fixed relay's
// `speech_starting` (now sent BEFORE synthesis, per protocol.rs:55-57) tripped it at t~0. The
// script then closed the socket mid-synthesis and reported "no audio arrived" for three whole
// suites — a green relay scored red by the instrument.

const text = (obj) => ({ ts: 1, kind: 'text', text: JSON.stringify(obj) });
const binary = (ts = 1) => ({ ts, kind: 'binary', bytes: 320 });

test('deliveryUnderway is false on an empty transcript', () => {
  assert.equal(deliveryUnderway([]), false);
});

test('deliveryUnderway is FALSE on speech_starting alone — audio is coming, not ended', () => {
  assert.equal(deliveryUnderway([text({ type: 'speech_starting' })]), false);
});

test('deliveryUnderway is true once a binary audio frame has arrived', () => {
  assert.equal(deliveryUnderway([text({ type: 'speech_starting' }), binary()]), true);
});

test('deliveryUnderway is true on speech_complete (a turn that ended without audio)', () => {
  assert.equal(deliveryUnderway([text({ type: 'speech_complete' })]), true);
});

test('deliveryUnderway is true on closing (a paused session)', () => {
  assert.equal(deliveryUnderway([text({ type: 'closing', reason: 'user_pause' })]), true);
});

test('deliveryUnderway ignores unparseable frames rather than throwing on them', () => {
  assert.equal(deliveryUnderway([{ ts: 1, kind: 'text', text: 'not json' }]), false);
});

test('firstAckAfter returns null when the ack never arrives', () => {
  assert.equal(firstAckAfter([text({ type: 'speech_starting' })], ['speech_complete'], 0), null);
});

test('firstAckAfter ignores acks that arrived BEFORE the interrupt', () => {
  const events = [{ ts: 5, kind: 'text', text: JSON.stringify({ type: 'speech_complete' }) }];
  assert.equal(firstAckAfter(events, ['speech_complete'], 10), null);
});

test('firstAckAfter returns the FIRST matching ack strictly after the interrupt', () => {
  const events = [
    { ts: 5, kind: 'text', text: JSON.stringify({ type: 'speech_complete' }) },
    { ts: 20, kind: 'text', text: JSON.stringify({ type: 'speech_complete' }) },
    { ts: 30, kind: 'text', text: JSON.stringify({ type: 'speech_complete' }) },
  ];
  assert.equal(firstAckAfter(events, ['speech_complete'], 10), 20);
});

test('firstAckAfter matches on the parsed type, never on a transcript containing the word', () => {
  // A transcript whose TEXT contains "speech_complete" must not be mistaken for the ack — this is
  // why the helper parses instead of substring-matching.
  const events = [
    { ts: 20, kind: 'text', text: JSON.stringify({ type: 'transcript', text: 'speech_complete' }) },
    { ts: 40, kind: 'text', text: JSON.stringify({ type: 'speech_complete' }) },
  ];
  assert.equal(firstAckAfter(events, ['speech_complete'], 10), 40);
});

test('firstAckAfter honours the frame type list (pause acknowledges with closing, not complete)', () => {
  const events = [{ ts: 20, kind: 'text', text: JSON.stringify({ type: 'closing' }) }];
  assert.equal(firstAckAfter(events, ['speech_complete'], 10), null);
  assert.equal(firstAckAfter(events, ['closing'], 10), 20);
});

test('countTextTypes tallies by type and buckets unparseable frames separately', () => {
  const counts = countTextTypes([
    text({ type: 'speech_starting' }),
    text({ type: 'speech_complete' }),
    text({ type: 'speech_complete' }),
    binary(),
    { ts: 1, kind: 'text', text: '{' },
  ]);
  assert.deepEqual(counts, { speech_starting: 1, speech_complete: 2, unparseable: 1 });
});

test('CLI fails closed (exit 6) when the relay is unreachable, without spending any TTS credit', () => {
  // Point at a port nothing listens on. This exercises preflightRelayReachable()'s real fail path
  // in the actual shipped script — no Fish TTS call is ever reached, so this costs nothing and is
  // fully deterministic, unlike the real measurement run this file does not attempt to replicate.
  let exitCode = null;
  let stdout = '';
  try {
    stdout = execFileSync('node', [SCRIPT_PATH], {
      encoding: 'utf8',
      timeout: 15000,
      env: { ...process.env, ORB_RELAY_WS_URL: 'ws://127.0.0.1:1' },
    });
    exitCode = 0;
  } catch (err) {
    exitCode = err.status;
    stdout = (err.stdout ?? '') + (err.stderr ?? '');
  }
  assert.equal(exitCode, 6, `expected exit 6 (fail-closed on unreachable relay), got ${exitCode}`);
  assert.match(stdout, /PREFLIGHT FAIL|relay unreachable/i);
});

console.log(`\n${passCount} passed, ${failCount} failed`);
process.exit(failCount > 0 ? 1 : 0);
