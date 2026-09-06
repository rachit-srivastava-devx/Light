#!/usr/bin/env node
/**
 * voice-ux-gate.mjs — the mechanisable half of docs/VOICE-UX-GUIDELINES.md, made to fail.
 *
 * Why this exists
 * ----------------
 * `docs/VOICE-UX-GUIDELINES.md` researched published voice-agent UX guidance (barge-in,
 * endpointing, latency, dead air, ASR recovery, confirmation, WCAG) and split it into what this
 * repo's own artefacts can prove versus what still needs a human ear. A prompt rule or a doc is a
 * `mitigates`; a check that fails is a `kills (mechanical)`. This is the mechanical half — see
 * that document's §8 for exactly which rule maps to which check below and why each threshold is
 * what it is.
 *
 * This gate does NOT re-measure what two existing gates in this repo already own:
 *   - `ux-gate/gate.py`      — single-drive UX checks (turn answered, one-breath, no-repeat,
 *                               USER-CLOCK first-audio latency) over one `trace.jsonl`.
 *   - `audio-gate/`          — rendered-audio continuity/latency from a real mic/speaker capture.
 * This gate's job is what neither of those can do: CENSUS-scale checks across the full
 * `dev-logs/` corpus — tens of thousands of turns across weeks, not one run's handful of turns.
 *
 * Reuse, not reinvention: the NDJSON reader below (glob `*.ndjson`, parse per line inside a
 * try/catch, `--hours`/`--json`, "0 rows read is a failure") copies `scripts/provider-report.mjs`
 * verbatim in shape. Money, where it appears at all here, stays integer paise (repo convention) —
 * this gate does not sum money, only counts events, so the convention shows up only in passthrough
 * fields taken directly from log rows.
 *
 * Design rules carried over from `ux-gate/gate.py` (same scars, same fixes):
 *   - A rule with nothing to measure (denominator 0, or a sample too small to trust) is a FAIL,
 *     never a silent pass. See G5 for a check built specifically to demonstrate this: it measures
 *     a real signal that (as of this writing) has a denominator of ~1 in the whole corpus, and it
 *     fails loudly for exactly that reason rather than reporting a meaningless 100%.
 *   - Every check publishes its denominator. "PASS" with no count next to it is not evidence.
 *   - A GATED check has a threshold traceable to either a cited rule ("this must never happen") or
 *     this product's own self-declared config value — never an invented percentage. Measurements
 *     with no defensible bar (degraded-reply rate, reservation-exceeded rate) are printed in full,
 *     with full denominators, but marked OBSERVE and excluded from the exit code — see
 *     docs/VOICE-UX-GUIDELINES.md §8 for why manufacturing a threshold there would be worse than
 *     not having one.
 *   - `dev-logs/` is a shared firehose: manual dev sessions, the full pytest suite, adversarial
 *     fuzzing (`adversarial/`), and e2e runs all write to the same files with no provenance tag
 *     (see docs/VOICE-UX-GUIDELINES.md §9, finding #2). Every OBSERVE rate below inherits that
 *     limitation and says so in its own output — a spike can mean "a redteam suite ran," not "real
 *     users are failing."
 *
 * Usage
 *   node scripts/voice-ux-gate.mjs                  # all of dev-logs/
 *   node scripts/voice-ux-gate.mjs --hours 6         # recent window only
 *   node scripts/voice-ux-gate.mjs --json            # machine-readable (printed after the report)
 *   node scripts/voice-ux-gate.mjs --dir <path>      # override the dev-logs directory
 *   node scripts/voice-ux-gate.mjs --e2e-dir <path>  # override the e2e runs directory
 *
 * Exit codes
 *   0 — every GATED rule passed, on a non-empty overall sample.
 *   6 — dev-logs/ produced 0 rows, OR any GATED rule failed (including "measured nothing" /
 *       insufficient-sample failures).
 */

import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

// --- reading dev-logs/ — same shape as scripts/provider-report.mjs on purpose --------------------

const DEFAULT_LOG_DIR = process.env.ORB_DEV_LOG_DIR ?? 'dev-logs';
const DEFAULT_E2E_DIR = join('e2e-human-simulator', 'runs');

/** Reads every *.ndjson file in `dir`, parsing each line independently. A line that fails to parse
 * (a partial last write, or one of this repo's known adversarial-fuzz junk files — see
 * docs/VOICE-UX-GUIDELINES.md §9 finding #1) is skipped, never invented or coerced. */
export function readRows(dir) {
  if (!existsSync(dir)) return { rows: [], files: [] };
  const files = readdirSync(dir).filter((f) => f.endsWith('.ndjson'));
  const rows = [];
  for (const file of files) {
    const text = readFileSync(join(dir, file), 'utf8');
    for (const line of text.split('\n')) {
      if (!line.trim()) continue;
      try {
        rows.push(JSON.parse(line));
      } catch {
        /* corrupt/partial line — skipped, not fabricated */
      }
    }
  }
  return { rows, files };
}

export function windowRows(rows, hours) {
  if (!hours) return rows;
  const cutoff = Date.now() / 1000 - hours * 3600;
  return rows.filter((r) => (r.ts ?? 0) >= cutoff);
}

const byEvent = (rows, name) => rows.filter((r) => r.event === name);

/** Prosody/emphasis markup ("[warm]", "[short pause]", "[breathing]") is never spoken aloud —
 * confirmed in docs/UX-HUMAN-REVIEW.md: hearing the bracket text spoken IS the defect. The
 * one-breath test is about spoken length, so these are stripped before counting, matching what a
 * listener actually hears. `conversation.response.response_text` is pre-envelope relay output and
 * still carries these tags; `ux-gate/gate.py`'s U4 does not need this step because it reads
 * trace.jsonl's already-separated `speech.spoken_text`. */
export function stripProsodyTags(text) {
  return String(text ?? '').replace(/\[[^\]]*\]/g, ' ').replace(/\s+/g, ' ').trim();
}

// --- a Check is either GATED (contributes to the exit code) or OBSERVE (printed, denominator and
// all, but not graded — see the file header for why) --------------------------------------------

function check({ id, title, gated, source, denominator, denominatorLabel, measured, measuredLabel, threshold, passed, detail }) {
  return { id, title, gated, source, denominator, denominatorLabel, measured, measuredLabel, threshold, passed, detail };
}

const INSUFFICIENT = (id, title, gated, source, denominatorLabel, n, minNeeded, detail) =>
  check({
    id, title, gated, source,
    denominator: n, denominatorLabel,
    measured: null, measuredLabel: null, threshold: null,
    passed: false,
    detail: `measured nothing — ${denominatorLabel} = ${n}, need >= ${minNeeded} to trust a verdict. ${detail}`,
  });

// --- G1: dead air / silence-budget violations must be zero ---------------------------------------
// Source: adhd-conversation-design/SKILL.md Rule 17 — "treat exceeding a silence budget as a
// FAILING state, not a degraded one." conversation.wait_failed fires exactly when the app's own
// configured wait budget was already exceeded, so the threshold is not invented — it is "never
// trip your own alarm."

function checkDeadAir(rows) {
  const requests = byEvent(rows, 'conversation.request').length;
  const waitFailed = byEvent(rows, 'conversation.wait_failed');
  if (requests === 0) {
    return INSUFFICIENT('G1', 'Zero dead-air / silence-budget violations', true,
      'adhd-conversation-design Rule 17', 'conversation.request in window', 0, 1,
      'no attempted turns in this window.');
  }
  return check({
    id: 'G1', title: 'Zero dead-air / silence-budget violations', gated: true,
    source: 'adhd-conversation-design Rule 17 (§4 of VOICE-UX-GUIDELINES.md)',
    denominator: requests, denominatorLabel: 'attempted turns (conversation.request)',
    measured: waitFailed.length, measuredLabel: 'conversation.wait_failed events',
    threshold: 0,
    passed: waitFailed.length === 0,
    detail: waitFailed.length
      ? `first breach: ${waitFailed[0].error ?? '(no detail on row)'}`
      : '',
  });
}

// --- G2: no consecutive verbatim-identical spoken replies within a session -----------------------
// Source: Alexa/Nuance repair convention; this repo's own docs/UX-HUMAN-REVIEW.md H8 and
// ux-gate/gate.py's U3 (same rule, single-run scope there; this is the census version).
//
// DATA-QUALITY GUARD, found while building this and left in on purpose: dev-logs/ reuses short,
// generic session_id values (e.g. "s1") as a lazy pytest fixture default across HUNDREDS of
// unrelated, independently-run test cases — confirmed directly: session "s1" alone carries 126
// conversation.response rows, and the gap between "consecutive" same-session rows in this corpus
// ranges up to 866,169 seconds (~10 days). Grouping by session_id and comparing raw adjacency
// therefore does not measure "the product repeated itself to one user" — it measures "two
// unrelated test invocations, run days apart, happened to both hit the same canned string,"
// which is a corpus artifact (see docs/VOICE-UX-GUIDELINES.md §9 finding #2), not a UX defect.
// MAX_PLAUSIBLE_GAP_SECONDS bounds what counts as "consecutive in one real interaction": generous
// enough to allow a real check-in-later gap (this product's whole concept is the user working on
// a task between check-ins), tight enough to exclude cross-test-run collisions measured in hours
// or days. Both the raw and the gap-bounded numbers are reported — this filter is a data-quality
// bound, not a second invented UX threshold, and is not hidden.
//
// SECOND guard, found by checking the FIRST fix's own result rather than trusting it: even
// gap-bounded, the repeat rate barely moved (2933->2823 of ~4300), because a full pytest run
// exercises hundreds of unrelated test cases inside one 30-minute window, many sharing the same
// lazy session_id. Checked what was actually repeating: 93% of gap-bounded repeats carry
// `source: "model"` with text like "A short reply here." / "Memory adapter response." — these are
// mocked-gateway fixture strings (e.g. `ScriptGateway` in
// backend/relay-py/tests/test_load_shift_gets_one_repair.py), not a real provider repeating
// itself. `source` values that are explicitly deterministic/templated by design
// (`safety_fallback`, `wait_companion`, `format_fallback`, `deterministic_control`) are excluded
// outright — repeating a fixed canned line on purpose is not the H8/U3 defect, which is about the
// GENERATIVE path failing to vary its phrasing. `source: "model"`/`"model_repaired"` remain
// checked because that is where a real defect would show up, but this gate cannot fully rule out
// mocked-gateway contamination within that bucket without joining to
// `gateway_client.completion.is_fake_adapter` by session/time proximity — not implemented here,
// named as a limitation rather than papered over. See docs/VOICE-UX-GUIDELINES.md §9 finding #6.

const MAX_PLAUSIBLE_GAP_SECONDS = 30 * 60; // 30 minutes — see comment above
const GENERATIVE_SOURCES = new Set(['model', 'model_repaired']);

function checkNoVerbatimRepeat(rows) {
  const responses = byEvent(rows, 'conversation.response')
    .filter((r) => typeof r.response_text === 'string')
    .filter((r) => r.source === undefined || GENERATIVE_SOURCES.has(r.source));
  const bySession = new Map();
  for (const r of responses) {
    const key = r.session_id ?? '(no session_id)';
    if (!bySession.has(key)) bySession.set(key, []);
    bySession.get(key).push(r);
  }
  let rawPairs = 0;
  let pairs = 0; // gap-bounded ("plausible") pairs — this is what gates
  let excludedForGap = 0;
  let repeats = 0;
  let firstRepeatSession = null;
  for (const [session, list] of bySession) {
    list.sort((a, b) => (a.ts ?? 0) - (b.ts ?? 0));
    for (let i = 1; i < list.length; i++) {
      rawPairs++;
      const gap = (list[i].ts ?? 0) - (list[i - 1].ts ?? 0);
      if (gap > MAX_PLAUSIBLE_GAP_SECONDS) {
        excludedForGap++;
        continue;
      }
      pairs++;
      const a = list[i - 1].response_text.trim();
      const b = list[i].response_text.trim();
      if (a && a === b) {
        repeats++;
        if (!firstRepeatSession) firstRepeatSession = session;
      }
    }
  }
  const gapNote = `${rawPairs} raw same-session generative-source pairs found; ${excludedForGap} excluded as `
    + `>${MAX_PLAUSIBLE_GAP_SECONDS / 60}min apart (near-certain cross-test-run session_id reuse, `
    + `see docs/VOICE-UX-GUIDELINES.md §9 #2), leaving ${pairs} pairs actually compared. `
    + `Deterministic/templated sources (safety_fallback, wait_companion, format_fallback, `
    + `deterministic_control) are excluded entirely — repeating a fixed canned line by design is `
    + `not this rule's concern. CAVEAT NOT FIXED: source:"model" in dev-logs/ also covers pytest's `
    + `mocked gateway clients returning literal fixture strings (e.g. "A short reply here."); this `
    + `check cannot yet tell that apart from a real provider repeating itself — see finding #6.`;
  if (pairs === 0) {
    return INSUFFICIENT('G2', 'No consecutive verbatim-identical GENERATIVE spoken replies (per session, gap-bounded)', true,
      'Alexa/Nuance repair convention; docs/UX-HUMAN-REVIEW.md H8; ux-gate/gate.py U3',
      'gap-bounded (<=30min), generative-source-only, same-session reply pairs', 0, 1,
      `${responses.length} generative-source replies across ${bySession.size} session(s). ${gapNote}`);
  }
  return check({
    id: 'G2', title: 'No consecutive verbatim-identical GENERATIVE spoken replies (per session, gap-bounded)', gated: true,
    source: 'Alexa/Nuance repair convention; docs/UX-HUMAN-REVIEW.md H8; ux-gate/gate.py U3',
    denominator: pairs, denominatorLabel: 'gap-bounded, generative-source-only, same-session reply pairs',
    measured: repeats, measuredLabel: 'identical consecutive pairs',
    threshold: 0,
    passed: repeats === 0,
    detail: (repeats ? `first repeat in session ${firstRepeatSession}. ` : '') + gapNote,
  });
}

// --- G3: spoken replies pass the one-breath test (<=240 chars, prosody tags stripped) ------------
// Source: Amazon Alexa's official one-breath test (developer.amazon.com blog, see
// VOICE-UX-GUIDELINES.md §3). Reuses ux-gate/gate.py's ONE_BREATH_CHARS=240 verbatim for
// cross-gate consistency rather than picking a different number.

const ONE_BREATH_CHARS = 240;

function checkOneBreath(rows) {
  const responses = byEvent(rows, 'conversation.response').filter((r) => typeof r.response_text === 'string');
  const texts = responses.map((r) => stripProsodyTags(r.response_text)).filter(Boolean);
  if (texts.length === 0) {
    return INSUFFICIENT('G3', `Spoken replies pass the one-breath test (<=${ONE_BREATH_CHARS} chars)`, true,
      'Amazon Alexa one-breath test', 'spoken replies checked', 0, 1, 'no reply text in window.');
  }
  const tooLong = texts.filter((t) => t.length > ONE_BREATH_CHARS);
  return check({
    id: 'G3', title: `Spoken replies pass the one-breath test (<=${ONE_BREATH_CHARS} chars, prosody tags stripped)`,
    gated: true,
    source: 'Amazon Alexa one-breath test (developer.amazon.com) — threshold shared with ux-gate/gate.py',
    denominator: texts.length, denominatorLabel: 'spoken replies checked',
    measured: tooLong.length, measuredLabel: 'replies over the limit',
    threshold: ONE_BREATH_CHARS,
    passed: tooLong.length === 0,
    detail: tooLong.length ? `longest: ${Math.max(...tooLong.map((t) => t.length))} chars` : '',
  });
}

// --- G4: every accepted request resolves to a terminal outcome (no silent black hole) ------------
// Source: docs/UX-HUMAN-REVIEW.md H11 "failure honesty"; W3C COGA "graceful recovery"; census
// version of ux-gate/gate.py's U1. Deliberately an AGGREGATE count comparison, not a per-session
// join (documented limitation, not hidden) — see docs/VOICE-UX-GUIDELINES.md §8.

const TERMINAL_OUTCOME_EVENTS = [
  'conversation.response',
  'conversation.reservation_exceeded',
  'conversation.gateway_error',
  'conversation.wait_failed',
];
const ACCOUNTED_FOR_THRESHOLD = 0.99;

function checkNoBlackHole(rows) {
  const requests = byEvent(rows, 'conversation.request').length;
  const outcomes = TERMINAL_OUTCOME_EVENTS.reduce((sum, ev) => sum + byEvent(rows, ev).length, 0);
  if (requests === 0) {
    return INSUFFICIENT('G4', 'Every accepted request resolves to a terminal outcome', true,
      'docs/UX-HUMAN-REVIEW.md H11; COGA graceful recovery',
      'conversation.request in window', 0, 1, 'no attempted turns in this window.');
  }
  const rate = Math.min(1, outcomes / requests);
  return check({
    id: 'G4', title: 'Every accepted request resolves to a terminal outcome (no silent black hole)',
    gated: true,
    source: 'docs/UX-HUMAN-REVIEW.md H11; W3C COGA graceful recovery; census version of ux-gate/gate.py U1',
    denominator: requests, denominatorLabel: 'conversation.request',
    measured: `${outcomes}/${requests} (${(rate * 100).toFixed(1)}%)`,
    measuredLabel: `accounted for by {${TERMINAL_OUTCOME_EVENTS.join(', ')}}`,
    threshold: `>=${(ACCOUNTED_FOR_THRESHOLD * 100).toFixed(0)}%`,
    passed: rate >= ACCOUNTED_FOR_THRESHOLD,
    detail: 'AGGREGATE count comparison across the whole window, not a per-session/per-turn join — '
      + 'a real per-turn black hole could hide inside a coincidentally-balanced total. Documented '
      + 'limitation, not a claim of exact pairing.',
  });
}

// --- G5: the relay yields the floor within its own budget (RELAY-SIDE ONLY) ----------------------
// Source: this product's own latency.budgets event (barge_in_yield_ms). Deliberately narrow: this
// checks how fast the RELAY yields, NEVER "the user's audio actually stopped" — that stronger
// claim belongs to audio-gate/, which documents in its own README that it is not wired yet. Built
// specifically to demonstrate "measured nothing must FAIL, not silently pass" — see
// docs/VOICE-UX-GUIDELINES.md §8.
//
// 2026-08-28: THE GRADED QUANTITY CHANGED, because the old one could not fail.
// This check used to grade `frame.processed.latency_ms` for frames with `in.type == "barge_in"` —
// the time the read loop spent handling the frame ONCE IT HAD READ IT. That number was ~0.1ms even
// while the relay was structurally incapable of barge-in: the pre-fix `serve_connection` could not
// read a `barge_in` frame at all until the whole utterance had been synthesised and flushed, so
// the frame was only ever handled when there was nothing left to interrupt. Run against a corpus
// containing that relay's own rows, this check reported p95 = 0.103ms PASS against a 100ms budget
// for a relay independently measured at p50 2176ms / p95 4046ms. It was measuring the wrong
// quantity, truly.
//
// It now grades `latency.barge_in_yield.yield_latency_ms`: frame read -> floor actually yielded
// (generation retired, its audio suppressed, acknowledgement queued). There is deliberately NO
// fallback to the old field when that event is absent — falling back to a metric that cannot fail
// is how a gate reports a green it did not earn. No rows, no verdict: INSUFFICIENT.

const BARGE_IN_MIN_SAMPLE = 5;
const DEFAULT_BARGE_IN_YIELD_MS = 100; // AGENTS.md's own stated bar, used only if no
                                        // latency.budgets row is present in the corpus at all.

const G5_TITLE = 'Relay yields the floor on barge-in within its own budget '
  + '(RELAY-SIDE YIELD ONLY — not rendered audio)';

function checkBargeInDispatch(rows) {
  const budgetRows = byEvent(rows, 'latency.budgets');
  const budget = budgetRows.length
    ? budgetRows[budgetRows.length - 1].barge_in_yield_ms ?? DEFAULT_BARGE_IN_YIELD_MS
    : DEFAULT_BARGE_IN_YIELD_MS;
  const yields = byEvent(rows, 'latency.barge_in_yield')
    .filter((r) => typeof r.yield_latency_ms === 'number');
  if (yields.length < BARGE_IN_MIN_SAMPLE) {
    const bargeFrames = byEvent(rows, 'frame.processed').filter((r) => r?.in?.type === 'barge_in');
    return INSUFFICIENT('G5', G5_TITLE,
      true, 'this product\'s own latency.budgets.barge_in_yield_ms',
      'latency.barge_in_yield events', yields.length, BARGE_IN_MIN_SAMPLE,
      `Not enough real yield measurements in this corpus (${bargeFrames.length} barge_in frames were `
      + 'processed, but a processed frame is not a measured yield — see the note above this function '
      + 'for why the old frame.processed.latency_ms proxy was retired rather than used as a fallback). '
      + 'Produce them with `npm run e2e:bargein` against a running relay. Note this is still '
      + 'RELAY-SIDE: audio-gate/README.md remains the owner of rendered-at-the-speaker yield, and '
      + 'says outright it is not wired yet.');
  }
  const latencies = yields.map((r) => r.yield_latency_ms).sort((a, b) => a - b);
  const p95 = latencies[Math.min(latencies.length - 1, Math.floor(0.95 * latencies.length))];
  const outsideBudget = yields.filter((r) => r.within_budget === false).length;
  return check({
    id: 'G5',
    title: G5_TITLE,
    gated: true,
    source: `this product's own latency.budgets.barge_in_yield_ms (=${budget}ms)`,
    denominator: yields.length, denominatorLabel: 'latency.barge_in_yield events',
    measured: p95, measuredLabel: 'p95 relay yield_latency_ms (frame read -> floor yielded)',
    threshold: budget,
    passed: p95 <= budget,
    detail: 'Claim: the relay retired the speaking generation, suppressed its remaining audio, and '
      + `acknowledged the yield this fast (${outsideBudget}/${yields.length} individual yields were `
      + 'over budget). Does NOT prove audio stopped at the speaker — the client playback buffer and '
      + 'the speaker itself are audio-gate/\'s job and are not yet wired there.',
  });
}

// --- O1: degraded-response rate (OBSERVE — no external "acceptable %" source exists) -------------

function checkDegradedRate(rows) {
  const responses = byEvent(rows, 'conversation.response');
  const degraded = responses.filter((r) => r.degraded === true);
  const reasons = new Map();
  for (const r of degraded) reasons.set(r.source ?? '(no source field)', (reasons.get(r.source ?? '(no source field)') ?? 0) + 1);
  return check({
    id: 'O1', title: 'Degraded-response rate', gated: false,
    source: 'AGENTS.md quality framing — no external "acceptable %" source found; reported, not graded',
    denominator: responses.length, denominatorLabel: 'conversation.response',
    measured: responses.length ? `${degraded.length}/${responses.length} (${((1000 * degraded.length) / responses.length / 10).toFixed(1)}%)` : '0/0',
    measuredLabel: 'degraded:true',
    threshold: null, passed: null,
    detail: degraded.length ? `by source: ${[...reasons.entries()].map(([k, v]) => `${k}=${v}`).join(', ')}` : '',
  });
}

// --- O2: reservation-exceeded-with-no-reply-text rate (OBSERVE) ----------------------------------
// conversation.reservation_exceeded / cost.reservation_exceeded / atomize.reservation_exceeded all
// map to HTTPException(status_code=402, detail=str(err)) with NO response envelope — confirmed by
// reading backend/relay-py/src/orb_relay/app.py:423,442,613,679 (read-only; not edited). Reported,
// not graded: docs/VOICE-UX-GUIDELINES.md §9 finding #2 — dev-logs/ mixes real usage with pytest
// and adversarial-fuzz traffic with no provenance tag, so a high rate here can mean "a redteam
// suite ran," not "real users are hitting a paywall." Finding #4 in the same doc: the mobile
// client DOES have a fallback path for this (ConversationPort.ts), but dev-logs/mobile.ndjson
// shows zero occurrences of it firing in this corpus — this gate cannot see past the relay
// boundary to confirm whether that fallback works, only that the relay's own reply is empty.

function checkReservationExceeded(rows) {
  const responses = byEvent(rows, 'conversation.response').length;
  const convExceeded = byEvent(rows, 'conversation.reservation_exceeded').length;
  const costExceeded = byEvent(rows, 'cost.reservation_exceeded').length;
  const atomizeExceeded = byEvent(rows, 'atomize.reservation_exceeded').length;
  const den = responses + convExceeded;
  return check({
    id: 'O2', title: 'Reservation-exceeded turns that get NO spoken reply text (HTTP 402, empty envelope)',
    gated: false,
    source: 'docs/UX-HUMAN-REVIEW.md H11 / W3C COGA graceful-recovery framing; HTTP mapping confirmed at '
      + 'backend/relay-py/src/orb_relay/app.py:423,442,613,679 — reported, not graded (see detail)',
    denominator: den, denominatorLabel: 'conversation.response + conversation.reservation_exceeded',
    measured: den ? `${convExceeded}/${den} (${((1000 * convExceeded) / den / 10).toFixed(1)}%)` : '0/0',
    measuredLabel: 'turn-scoped reservation_exceeded (no reply text)',
    threshold: null, passed: null,
    detail: `broader, NOT turn-aligned, cost-line-level context: cost.reservation_exceeded=${costExceeded} `
      + `(can fire more than once per turn, includes non-conversation cost lines), `
      + `atomize.reservation_exceeded=${atomizeExceeded}. dev-logs/ mixes manual/pytest/adversarial `
      + `traffic with no provenance tag (VOICE-UX-GUIDELINES.md §9 #2) — treat a spike as "something `
      + `fired this code path," not "N real users got silence."`,
  });
}

// --- e2e context (informational only; the authoritative single-run judge is ux-gate/gate.py) -----

function findLatestE2ERun(e2eDir) {
  if (!existsSync(e2eDir)) return null;
  const candidates = [];
  const walk = (dir, depth) => {
    if (depth > 3) return;
    let entries;
    try { entries = readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const entry of entries) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) { walk(full, depth + 1); continue; }
      if (entry.name === 'run-result.json') candidates.push(full);
    }
  };
  walk(e2eDir, 0);
  if (candidates.length === 0) return null;
  candidates.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  const latestPath = candidates[0];
  try {
    const data = JSON.parse(readFileSync(latestPath, 'utf8'));
    return { path: latestPath, mtime: statSync(latestPath).mtime.toISOString(), data };
  } catch {
    return { path: latestPath, mtime: statSync(latestPath).mtime.toISOString(), data: null };
  }
}

// --- report assembly + printing -------------------------------------------------------------------

export function buildReport(rows, { hours, e2eDir } = {}) {
  const windowed = windowRows(rows, hours);
  const gated = [
    checkDeadAir(windowed),
    checkNoVerbatimRepeat(windowed),
    checkOneBreath(windowed),
    checkNoBlackHole(windowed),
    checkBargeInDispatch(windowed),
  ];
  const observe = [
    checkDegradedRate(windowed),
    checkReservationExceeded(windowed),
  ];
  const e2e = e2eDir ? findLatestE2ERun(e2eDir) : null;
  return {
    window: hours ? `last ${hours}h` : 'all recorded',
    rows_total: rows.length,
    rows_in_window: windowed.length,
    gated,
    observe,
    e2e_context: e2e && e2e.data ? {
      run: e2e.path,
      mtime: e2e.mtime,
      p50_inject_to_first_audio_ms: e2e.data.p50_inject_to_first_audio_ms ?? null,
      turns_expected: e2e.data.turns_expected ?? null,
      turns_complete: e2e.data.turns_complete ?? null,
      note: 'informational only — user-clock, single run. ux-gate/gate.py is the authoritative '
        + 'pass/fail judge for this exact number; not duplicated here.',
    } : null,
    all_gated_passed: gated.every((c) => c.passed === true),
  };
}

function printHuman(report) {
  console.log(`VOICE-UX GATE — ${report.window}, ${report.rows_in_window} log rows in window (${report.rows_total} total read)\n`);
  console.log('GATED (pass/fail, drives exit code)');
  console.log('-'.repeat(78));
  for (const c of report.gated) {
    const status = c.passed === true ? 'PASS ' : 'FAIL ';
    console.log(`${status} ${c.id}  ${c.title}`);
    console.log(`        source     : ${c.source}`);
    console.log(`        denominator: ${c.denominator} (${c.denominatorLabel})`);
    if (c.measured !== null) console.log(`        measured   : ${c.measured}${c.measuredLabel ? ` [${c.measuredLabel}]` : ''}`);
    if (c.threshold !== null) console.log(`        threshold  : ${c.threshold}`);
    if (c.detail) console.log(`        detail     : ${c.detail}`);
  }
  console.log('\nOBSERVE (measured + denominator, not graded — see docs/VOICE-UX-GUIDELINES.md §8 for why)');
  console.log('-'.repeat(78));
  for (const c of report.observe) {
    console.log(`OBSERVE ${c.id}  ${c.title}`);
    console.log(`        source     : ${c.source}`);
    console.log(`        denominator: ${c.denominator} (${c.denominatorLabel})`);
    console.log(`        measured   : ${c.measured}${c.measuredLabel ? ` [${c.measuredLabel}]` : ''}`);
    if (c.detail) console.log(`        detail     : ${c.detail}`);
  }
  if (report.e2e_context) {
    console.log('\nE2E CONTEXT (informational, most recent run under e2e-human-simulator/runs/)');
    console.log('-'.repeat(78));
    console.log(`        run        : ${report.e2e_context.run}`);
    console.log(`        mtime      : ${report.e2e_context.mtime}`);
    console.log(`        p50 inject-to-first-audio: ${report.e2e_context.p50_inject_to_first_audio_ms} ms`
      + ` (${report.e2e_context.turns_complete}/${report.e2e_context.turns_expected} turns)`);
    console.log(`        note       : ${report.e2e_context.note}`);
  } else {
    console.log('\nE2E CONTEXT: none found under the configured e2e directory.');
  }
  const passedCount = report.gated.filter((c) => c.passed).length;
  console.log('\n' + '='.repeat(78));
  console.log(`GATED: ${passedCount}/${report.gated.length} passed`);
  console.log(report.all_gated_passed && report.rows_total > 0 ? 'OVERALL PASS' : 'OVERALL FAIL');
}

function parseArgs(argv) {
  const args = { hours: null, json: false, dir: DEFAULT_LOG_DIR, e2eDir: DEFAULT_E2E_DIR };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--hours') args.hours = Number(argv[++i]);
    else if (argv[i] === '--json') args.json = true;
    else if (argv[i] === '--dir') args.dir = argv[++i];
    else if (argv[i] === '--e2e-dir') args.e2eDir = argv[++i];
  }
  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const { rows, files } = readRows(args.dir);
  if (rows.length === 0) {
    console.error(`FAIL: read 0 rows from ${args.dir}/. A gate over an empty set proves nothing.`);
    return 6;
  }
  const report = buildReport(rows, { hours: args.hours, e2eDir: args.e2eDir });
  report.dev_log_dir = args.dir;
  report.dev_log_files = files;
  printHuman(report);
  if (args.json) console.log('\n' + JSON.stringify(report));
  return report.all_gated_passed && report.rows_total > 0 ? 0 : 6;
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (isMain) {
  process.exit(main());
}

export {
  checkDeadAir, checkNoVerbatimRepeat, checkOneBreath, checkNoBlackHole, checkBargeInDispatch,
  checkDegradedRate, checkReservationExceeded, findLatestE2ERun,
  ONE_BREATH_CHARS, BARGE_IN_MIN_SAMPLE, ACCOUNTED_FOR_THRESHOLD, TERMINAL_OUTCOME_EVENTS,
};
