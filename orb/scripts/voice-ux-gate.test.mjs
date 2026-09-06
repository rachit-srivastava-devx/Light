#!/usr/bin/env node
// voice-ux-gate.test.mjs
//
// Tests for scripts/voice-ux-gate.mjs: the individual check functions against hand-built fixture
// rows, PLUS an end-to-end subprocess run of the actual CLI (`node scripts/voice-ux-gate.mjs
// --dir <tmp> --json`) against real fixture NDJSON files — proving the shipped script, not a
// friendlier internal API, matching orb-shape-gate.test.mjs's own precedent for gates in this
// directory.
//
// This is a plain Node script, NOT a vitest suite, and that is deliberate, not an oversight:
// vitest.config.ts's `test.include` is scoped to apps/mobile/, backend/gateway-sidecar/, and
// backend/voice-provider-sidecar/ only — there is no `scripts/**` glob in it. A `scripts/*.test.ts`
// file would silently never run under `npm run test` / `npx vitest run` regardless of which
// directory you invoke it from. orb-shape-gate.test.mjs already hit this and documented it in its
// own header; verified again here empirically (see the bottom of this file's run log / the task
// report) rather than re-trusted on the comment alone. Touching vitest.config.ts to add a
// scripts/** glob was considered and rejected: that file is shared test infrastructure this change
// does not own, and orb-shape-gate.test.mjs already set the precedent of NOT doing that for
// exactly this reason.
//
// Usage: node scripts/voice-ux-gate.test.mjs

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync, mkdirSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  readRows,
  windowRows,
  stripProsodyTags,
  checkDeadAir,
  checkNoVerbatimRepeat,
  checkOneBreath,
  checkNoBlackHole,
  checkBargeInDispatch,
  checkDegradedRate,
  checkReservationExceeded,
  findLatestE2ERun,
  buildReport,
  ONE_BREATH_CHARS,
  BARGE_IN_MIN_SAMPLE,
} from './voice-ux-gate.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const GATE_PATH = join(__dirname, 'voice-ux-gate.mjs');
const REPO_ROOT = join(__dirname, '..');

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

const now = () => Date.now() / 1000;

// -------------------------------------------------------------------------------------------
// stripProsodyTags — the one-breath test must measure what a listener hears, not the raw
// bracket-annotated string. This is the exact scenario that motivated the function: a reply
// that is short once prosody tags are removed but would wrongly fail the char limit if they
// were counted.
// -------------------------------------------------------------------------------------------

console.log('stripProsodyTags');
test('strips a single tag', () => {
  assert.equal(stripProsodyTags('[warm] Hi there.'), 'Hi there.');
});
test('strips multiple tags and collapses whitespace', () => {
  assert.equal(
    stripProsodyTags('[thinking] Hmm, [short pause] let me think. [breathing]'),
    'Hmm, let me think.',
  );
});
test('a reply that is only over 240 chars BECAUSE of tag text passes once stripped', () => {
  const padding = 'x'.repeat(200);
  const tagHeavy = `[a very long stage direction that pads this string past 240 characters all by itself ${padding}] short reply`;
  assert.ok(tagHeavy.length > ONE_BREATH_CHARS, 'fixture sanity: raw string must exceed the limit');
  assert.ok(stripProsodyTags(tagHeavy).length <= ONE_BREATH_CHARS, 'stripped string must be short');
});
test('handles null/undefined without throwing', () => {
  assert.equal(stripProsodyTags(undefined), '');
  assert.equal(stripProsodyTags(null), '');
});

// -------------------------------------------------------------------------------------------
// readRows — the NDJSON reader must skip corrupt lines (partial writes, or this repo's known
// adversarial-fuzz junk files, see docs/VOICE-UX-GUIDELINES.md §9 finding #1) without crashing
// or fabricating rows, and must read across multiple files in the directory.
// -------------------------------------------------------------------------------------------

console.log('\nreadRows');
{
  const tmpDir = mkdtempSync(join(tmpdir(), 'voice-ux-gate-readrows-'));
  writeFileSync(
    join(tmpDir, 'relay-py.ndjson'),
    [
      JSON.stringify({ ts: now(), event: 'conversation.request', session_id: 's1' }),
      JSON.stringify({ ts: now(), event: 'conversation.response', session_id: 's1', response_text: 'hi' }),
      'not even json{{{',
      '{"event": "truncated_mid_wri',
    ].join('\n') + '\n',
  );
  writeFileSync(
    join(tmpDir, 'relay-rs.ndjson'),
    JSON.stringify({ ts: now(), event: 'latency.budgets', barge_in_yield_ms: 100 }) + '\n',
  );
  // A file with a garbled name and garbage-but-valid-JSON content, mirroring the real files found
  // in dev-logs/ (e.g. K.ndjson, CMëÑ.ndjson) — must be read without crashing, and its nonsense
  // "event" value must never collide with a real check.
  writeFileSync(join(tmpDir, 'K.ndjson'), JSON.stringify({ ts: now(), service: 'K', event: '-' }) + '\n');

  test('reads valid rows across multiple files, skips corrupt lines', () => {
    const { rows, files } = readRows(tmpDir);
    assert.equal(files.length, 3);
    // 2 good rows in relay-py.ndjson + 1 in relay-rs.ndjson + 1 in K.ndjson = 4; the two corrupt
    // lines must be silently dropped, not thrown, not counted.
    assert.equal(rows.length, 4);
  });
  test('missing directory returns empty, not a throw', () => {
    const { rows, files } = readRows(join(tmpDir, 'does-not-exist'));
    assert.equal(rows.length, 0);
    assert.equal(files.length, 0);
  });
  rmSync(tmpDir, { recursive: true, force: true });
}

// -------------------------------------------------------------------------------------------
// windowRows — --hours filtering
// -------------------------------------------------------------------------------------------

console.log('\nwindowRows');
test('excludes rows older than the window, keeps recent ones', () => {
  const rows = [
    { ts: now() - 10 * 3600, event: 'old' },
    { ts: now() - 100, event: 'recent' },
  ];
  const windowed = windowRows(rows, 1);
  assert.equal(windowed.length, 1);
  assert.equal(windowed[0].event, 'recent');
});
test('no --hours means no filtering', () => {
  const rows = [{ ts: 0, event: 'ancient' }];
  assert.equal(windowRows(rows, null).length, 1);
});

// -------------------------------------------------------------------------------------------
// G1 — checkDeadAir
// -------------------------------------------------------------------------------------------

console.log('\nG1 checkDeadAir');
test('passes with requests and zero wait_failed', () => {
  const rows = [
    { event: 'conversation.request' },
    { event: 'conversation.request' },
  ];
  const c = checkDeadAir(rows);
  assert.equal(c.passed, true);
  assert.equal(c.denominator, 2);
  assert.equal(c.measured, 0);
});
test('fails when even one wait_failed fires (threshold is 0, not a rate)', () => {
  const rows = [
    { event: 'conversation.request' },
    { event: 'conversation.request' },
    { event: 'conversation.wait_failed', error: 'wait silence budget exceeded: 6000ms > 5000ms' },
  ];
  const c = checkDeadAir(rows);
  assert.equal(c.passed, false);
  assert.equal(c.measured, 1);
  assert.match(c.detail, /6000ms/);
});
test('measured nothing (0 requests) FAILS, does not pass silently', () => {
  const c = checkDeadAir([]);
  assert.equal(c.passed, false);
  assert.match(c.detail, /measured nothing/);
});

// -------------------------------------------------------------------------------------------
// G2 — checkNoVerbatimRepeat
// -------------------------------------------------------------------------------------------

console.log('\nG2 checkNoVerbatimRepeat');
test('passes when consecutive replies in a session differ', () => {
  const rows = [
    { event: 'conversation.response', session_id: 's1', ts: 1, response_text: 'One step: open the file.' },
    { event: 'conversation.response', session_id: 's1', ts: 2, response_text: 'Nice, one done.' },
  ];
  const c = checkNoVerbatimRepeat(rows);
  assert.equal(c.passed, true);
  assert.equal(c.denominator, 1);
});
test('fails on an exact consecutive repeat within the same session', () => {
  const rows = [
    { event: 'conversation.response', session_id: 's1', ts: 1, response_text: 'I did not catch that.' },
    { event: 'conversation.response', session_id: 's1', ts: 2, response_text: 'I did not catch that.' },
  ];
  const c = checkNoVerbatimRepeat(rows);
  assert.equal(c.passed, false);
  assert.equal(c.measured, 1);
});
test('does not compare across different sessions', () => {
  const rows = [
    { event: 'conversation.response', session_id: 's1', ts: 1, response_text: 'same text' },
    { event: 'conversation.response', session_id: 's2', ts: 1, response_text: 'same text' },
  ];
  const c = checkNoVerbatimRepeat(rows);
  assert.equal(c.passed, false, 'insufficient sample (0 pairs), not a false pass');
  assert.match(c.detail, /measured nothing/);
});
test('measured nothing (no session has 2+ replies) FAILS', () => {
  const rows = [{ event: 'conversation.response', session_id: 's1', ts: 1, response_text: 'only one' }];
  const c = checkNoVerbatimRepeat(rows);
  assert.equal(c.passed, false);
  assert.match(c.detail, /measured nothing/);
});
test('a repeated DETERMINISTIC/templated source (e.g. safety_fallback) is excluded, not flagged — '
  + 'repeating a fixed canned line by design is not the H8/U3 defect', () => {
  const rows = [
    { event: 'conversation.response', session_id: 's1', ts: 1, source: 'safety_fallback', response_text: 'canned line' },
    { event: 'conversation.response', session_id: 's1', ts: 2, source: 'safety_fallback', response_text: 'canned line' },
  ];
  const c = checkNoVerbatimRepeat(rows);
  assert.equal(c.denominator, 0, 'both rows should be filtered out before pairing');
  assert.equal(c.passed, false); // insufficient sample, not a false pass
  assert.match(c.detail, /measured nothing/);
});
test('a repeated GENERATIVE source (model) two rows apart in time within the gap bound IS flagged', () => {
  const rows = [
    { event: 'conversation.response', session_id: 's1', ts: 1, source: 'model', response_text: 'exact same reply' },
    { event: 'conversation.response', session_id: 's1', ts: 60, source: 'model', response_text: 'exact same reply' },
  ];
  const c = checkNoVerbatimRepeat(rows);
  assert.equal(c.denominator, 1);
  assert.equal(c.passed, false);
  assert.equal(c.measured, 1);
});
test('same-session pairs more than 30 minutes apart are excluded as near-certain session_id reuse '
  + 'across unrelated test runs, not one real conversation', () => {
  const rows = [
    { event: 'conversation.response', session_id: 's1', ts: 0, source: 'model', response_text: 'same text' },
    { event: 'conversation.response', session_id: 's1', ts: 3600, source: 'model', response_text: 'same text' }, // 1h later
  ];
  const c = checkNoVerbatimRepeat(rows);
  assert.equal(c.denominator, 0, 'the >30min pair must be excluded, not counted');
  assert.match(c.detail, /excluded as/);
});

// -------------------------------------------------------------------------------------------
// G3 — checkOneBreath
// -------------------------------------------------------------------------------------------

console.log('\nG3 checkOneBreath');
test('passes short replies', () => {
  const rows = [{ event: 'conversation.response', response_text: '[warm] One step: open the file.' }];
  const c = checkOneBreath(rows);
  assert.equal(c.passed, true);
  assert.equal(c.threshold, ONE_BREATH_CHARS);
});
test('fails a reply over the limit even after stripping tags', () => {
  const rows = [{ event: 'conversation.response', response_text: 'x'.repeat(ONE_BREATH_CHARS + 1) }];
  const c = checkOneBreath(rows);
  assert.equal(c.passed, false);
  assert.equal(c.measured, 1);
});
test('measured nothing (no response_text at all) FAILS', () => {
  const c = checkOneBreath([{ event: 'conversation.response' }]);
  assert.equal(c.passed, false);
  assert.match(c.detail, /measured nothing/);
});

// -------------------------------------------------------------------------------------------
// G4 — checkNoBlackHole
// -------------------------------------------------------------------------------------------

console.log('\nG4 checkNoBlackHole');
test('passes when every request is accounted for', () => {
  const rows = [
    { event: 'conversation.request' },
    { event: 'conversation.response' },
    { event: 'conversation.request' },
    { event: 'conversation.reservation_exceeded' },
  ];
  const c = checkNoBlackHole(rows);
  assert.equal(c.passed, true);
  assert.equal(c.denominator, 2);
});
test('fails when requests substantially outnumber outcomes', () => {
  const rows = [
    { event: 'conversation.request' },
    { event: 'conversation.request' },
    { event: 'conversation.request' },
    { event: 'conversation.request' },
    { event: 'conversation.response' }, // only 1 of 4 accounted for
  ];
  const c = checkNoBlackHole(rows);
  assert.equal(c.passed, false);
});
test('measured nothing (0 requests) FAILS', () => {
  const c = checkNoBlackHole([]);
  assert.equal(c.passed, false);
  assert.match(c.detail, /measured nothing/);
});

// -------------------------------------------------------------------------------------------
// G5 — checkBargeInDispatch — the flagship "measured nothing must FAIL, not pass silently" case.
// This corpus's real denominator (see the real-evidence run at the bottom of this file) is far
// below BARGE_IN_MIN_SAMPLE, so this exact behavior is not hypothetical.
// -------------------------------------------------------------------------------------------

console.log('\nG5 checkBargeInDispatch');
test(`insufficient sample (below BARGE_IN_MIN_SAMPLE=${BARGE_IN_MIN_SAMPLE}) FAILS, not a silent 100%`, () => {
  const rows = [
    { event: 'latency.budgets', barge_in_yield_ms: 100 },
    { event: 'frame.processed', in: { type: 'barge_in' }, latency_ms: 0.08 },
  ];
  const c = checkBargeInDispatch(rows);
  assert.equal(c.passed, false);
  assert.equal(c.denominator, 1);
  assert.match(c.detail, /measured nothing|insufficient/);
});
test('passes with >=MIN_SAMPLE frames all under the budget', () => {
  const rows = [{ event: 'latency.budgets', barge_in_yield_ms: 100 }];
  for (let i = 0; i < BARGE_IN_MIN_SAMPLE; i++) {
    rows.push({ event: 'frame.processed', in: { type: 'barge_in' }, latency_ms: 1 + i * 0.1 });
  }
  const c = checkBargeInDispatch(rows);
  assert.equal(c.passed, true);
});
test('fails when p95 dispatch latency exceeds the budget', () => {
  const rows = [{ event: 'latency.budgets', barge_in_yield_ms: 100 }];
  for (let i = 0; i < BARGE_IN_MIN_SAMPLE; i++) {
    rows.push({ event: 'frame.processed', in: { type: 'barge_in' }, latency_ms: 500 });
  }
  const c = checkBargeInDispatch(rows);
  assert.equal(c.passed, false);
});
test('non-barge_in frames are not counted toward the sample', () => {
  const rows = [{ event: 'latency.budgets', barge_in_yield_ms: 100 }];
  for (let i = 0; i < 20; i++) rows.push({ event: 'frame.processed', in: { type: 'mic_audio' }, latency_ms: 1 });
  const c = checkBargeInDispatch(rows);
  assert.equal(c.denominator, 0);
  assert.equal(c.passed, false);
});

// -------------------------------------------------------------------------------------------
// O1/O2 — OBSERVE checks must never report a boolean verdict
// -------------------------------------------------------------------------------------------

console.log('\nO1/O2 OBSERVE checks');
test('checkDegradedRate is never gated (passed is always null)', () => {
  const rows = [
    { event: 'conversation.response', degraded: true, source: 'safety_fallback' },
    { event: 'conversation.response' },
  ];
  const c = checkDegradedRate(rows);
  assert.equal(c.gated, false);
  assert.equal(c.passed, null);
  assert.equal(c.denominator, 2);
  assert.match(c.measured, /1\/2/);
});
test('checkReservationExceeded is never gated and separates turn-scoped from cost-line-level counts', () => {
  const rows = [
    { event: 'conversation.response' },
    { event: 'conversation.reservation_exceeded' },
    { event: 'cost.reservation_exceeded' },
    { event: 'cost.reservation_exceeded' },
  ];
  const c = checkReservationExceeded(rows);
  assert.equal(c.gated, false);
  assert.equal(c.passed, null);
  assert.equal(c.denominator, 2); // 1 response + 1 conversation.reservation_exceeded, NOT the cost.* rows
  assert.match(c.detail, /cost\.reservation_exceeded=2/);
});

// -------------------------------------------------------------------------------------------
// buildReport — overall assembly and all_gated_passed
// -------------------------------------------------------------------------------------------

console.log('\nbuildReport');
test('all_gated_passed is false if any gated check fails', () => {
  const rows = [
    { event: 'conversation.request', ts: now() },
    { event: 'conversation.response', ts: now(), session_id: 's1', response_text: 'ok' },
    { event: 'conversation.wait_failed', ts: now(), error: 'wait silence budget exceeded: 9000ms > 5000ms' },
  ];
  const report = buildReport(rows, {});
  assert.equal(report.all_gated_passed, false);
  const g1 = report.gated.find((c) => c.id === 'G1');
  assert.equal(g1.passed, false);
});
test('--hours window is applied before every check runs', () => {
  const rows = [
    { event: 'conversation.request', ts: now() - 100000 }, // old, outside a 1h window
    { event: 'conversation.wait_failed', ts: now() - 100000, error: 'old failure, outside window' },
  ];
  const report = buildReport(rows, { hours: 1 });
  assert.equal(report.rows_in_window, 0);
  const g1 = report.gated.find((c) => c.id === 'G1');
  assert.match(g1.detail, /measured nothing/); // the old wait_failed must NOT be visible in-window
});

// -------------------------------------------------------------------------------------------
// findLatestE2ERun — reads real run-result.json shape, never fabricates one
// -------------------------------------------------------------------------------------------

console.log('\nfindLatestE2ERun');
{
  const tmpDir = mkdtempSync(join(tmpdir(), 'voice-ux-gate-e2e-'));
  const runDir = join(tmpDir, 'opus-drive', '20260101T000000Z');
  mkdirSync(runDir, { recursive: true });
  writeFileSync(join(runDir, 'run-result.json'), JSON.stringify({
    p50_inject_to_first_audio_ms: 2058.867, turns_expected: 3, turns_complete: 3,
  }));
  test('finds and parses a real run-result.json', () => {
    const found = findLatestE2ERun(tmpDir);
    assert.ok(found);
    assert.equal(found.data.turns_complete, 3);
  });
  test('returns null when nothing exists', () => {
    assert.equal(findLatestE2ERun(join(tmpDir, 'nope')), null);
  });
  rmSync(tmpDir, { recursive: true, force: true });
}

// -------------------------------------------------------------------------------------------
// End-to-end: run the ACTUAL shipped CLI as a subprocess against fixture files — proves the
// shipped script, not just the internal functions above (same principle as
// orb-shape-gate.test.mjs's PNG-fixture subprocess run).
// -------------------------------------------------------------------------------------------

console.log('\nCLI subprocess (the shipped script, not the internal API)');

function runCLI(args, opts = {}) {
  try {
    const stdout = execFileSync('node', [GATE_PATH, ...args], { encoding: 'utf8', ...opts });
    return { status: 0, stdout };
  } catch (err) {
    return { status: err.status ?? 1, stdout: err.stdout ?? '' };
  }
}

{
  const tmpDir = mkdtempSync(join(tmpdir(), 'voice-ux-gate-cli-'));

  test('empty directory: exit 6, "read 0 rows"', () => {
    const emptyDir = join(tmpDir, 'empty');
    mkdirSync(emptyDir);
    const result = runCLI(['--dir', emptyDir]);
    assert.equal(result.status, 6);
  });

  const goodDir = join(tmpDir, 'good');
  mkdirSync(goodDir);
  const nowTs = now();
  const goodRows = [
    { ts: nowTs, event: 'conversation.request', session_id: 's1' },
    { ts: nowTs, event: 'conversation.response', session_id: 's1', response_text: 'Nice, one step done.' },
    { ts: nowTs, event: 'conversation.request', session_id: 's1' },
    { ts: nowTs + 1, event: 'conversation.response', session_id: 's1', response_text: 'How about the next one.' },
    { ts: nowTs, event: 'conversation.request', session_id: 's2' },
    { ts: nowTs, event: 'conversation.response', session_id: 's2', response_text: 'How about we open the file.' },
    { ts: nowTs, event: 'latency.budgets', barge_in_yield_ms: 100 },
    // A corrupt line, exactly like the real garbled files in dev-logs/ — must not crash the CLI.
    'garbage-not-json',
  ];
  // >=BARGE_IN_MIN_SAMPLE fast barge-in dispatches, so G5 has enough sample to render a real PASS
  // here — the insufficient-sample FAIL path is already proven in isolation above; this fixture's
  // job is to prove a genuinely clean, fully-populated corpus passes end to end.
  for (let i = 0; i < BARGE_IN_MIN_SAMPLE; i++) {
    goodRows.push({ ts: nowTs, event: 'frame.processed', in: { type: 'barge_in' }, latency_ms: 1 + i * 0.1 });
  }
  writeFileSync(join(goodDir, 'relay-py.ndjson'), goodRows.map((r) => (typeof r === 'string' ? r : JSON.stringify(r))).join('\n') + '\n');

  test('a clean, fully-populated corpus (incl. enough barge-in samples) exits 0 and prints JSON', () => {
    const result = runCLI(['--dir', goodDir, '--json']);
    assert.equal(result.status, 0, `expected exit 0, got ${result.status}. stdout:\n${result.stdout}`);
    const jsonLine = result.stdout.trim().split('\n').filter((l) => l.startsWith('{')).pop();
    const parsed = JSON.parse(jsonLine);
    assert.equal(parsed.all_gated_passed, true);
    for (const c of parsed.gated) assert.equal(c.passed, true, `${c.id} unexpectedly failed: ${c.detail}`);
  });

  test('removing the barge-in frames from an otherwise-clean corpus fails ONLY G5 (insufficient sample), proving the aggregate exit code tracks a single rule\'s honesty, not just a vibe', () => {
    const thinDir = join(tmpDir, 'thin-barge-in');
    mkdirSync(thinDir);
    writeFileSync(join(thinDir, 'relay-py.ndjson'), goodRows
      .filter((r) => typeof r === 'string' || r.event !== 'frame.processed')
      .map((r) => (typeof r === 'string' ? r : JSON.stringify(r))).join('\n') + '\n');
    const result = runCLI(['--dir', thinDir, '--json']);
    assert.equal(result.status, 6);
    const jsonLine = result.stdout.trim().split('\n').filter((l) => l.startsWith('{')).pop();
    const parsed = JSON.parse(jsonLine);
    const failing = parsed.gated.filter((c) => !c.passed).map((c) => c.id);
    assert.deepEqual(failing, ['G5']);
  });

  const badDir = join(tmpDir, 'bad');
  mkdirSync(badDir);
  const badRows = [
    { ts: nowTs, event: 'conversation.request', session_id: 's1' },
    { ts: nowTs, event: 'conversation.wait_failed', session_id: 's1', error: 'wait silence budget exceeded: 7000ms > 5000ms' },
  ];
  writeFileSync(join(badDir, 'relay-py.ndjson'), badRows.map((r) => JSON.stringify(r)).join('\n') + '\n');

  test('a corpus with a real dead-air violation exits 6', () => {
    const result = runCLI(['--dir', badDir]);
    assert.equal(result.status, 6);
  });

  rmSync(tmpDir, { recursive: true, force: true });
}

// -------------------------------------------------------------------------------------------
// Informational: run against the REAL dev-logs/ corpus already in this repo. NOT asserted —
// this is live, growing evidence this test does not own (matches orb-shape-gate.test.mjs's own
// precedent of an unasserted informational run at the bottom of its suite). See the task report
// for the actual pasted output this produced.
// -------------------------------------------------------------------------------------------

console.log('\nvoice-ux-gate: real dev-logs/ in this repo (informational only, not asserted)');
const realLogDir = join(REPO_ROOT, 'dev-logs');
if (existsSync(realLogDir)) {
  const result = runCLI(['--dir', realLogDir, '--json']);
  console.log(`  exit=${result.status}`);
  console.log(result.stdout.split('\n').slice(0, 6).map((l) => `  ${l}`).join('\n'));
  console.log('  ... (see the task report / run the CLI directly for the full output)');
} else {
  console.log(`  (skip) ${realLogDir} not present`);
}

console.log(`\n${passCount} passed, ${failCount} failed`);
process.exit(failCount > 0 ? 1 : 0);
