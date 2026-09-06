/**
 * "Should we change the LLM or the API?" — answered from telemetry that already exists.
 *
 * Asked for directly: *"you should keep record, do observability and other important things to make
 * sure the llms or apis we are using needs to be changed, cost meter, token meter, llm as a judge
 * etc."*
 *
 * The raw data was already being written — `cost.settled`, `gateway_client.completion`,
 * `atomizer.llm_attempt`, `conversation.response` — and nothing read it back. Six thousand cost rows
 * with no rollup is not observability; it is a log. This is the rollup.
 *
 *   node scripts/provider-report.mjs                  # all of dev-logs/
 *   node scripts/provider-report.mjs --hours 2        # recent window only
 *   node scripts/provider-report.mjs --json           # machine-readable
 *
 * Exits 0 always EXCEPT when it read nothing — a report over an empty set is a failure, not a clean
 * bill of health.
 */

import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const LOG_DIR = process.env.ORB_DEV_LOG_DIR ?? 'dev-logs';
const args = process.argv.slice(2);
const asJson = args.includes('--json');
const hoursIndex = args.indexOf('--hours');
const windowHours = hoursIndex >= 0 ? Number(args[hoursIndex + 1]) : null;

// Money is integer paise everywhere in this repo (floats for currency are banned), so it stays
// integer here too and is only divided for display.
const PAISE_PER_RUPEE = 100;
const RUPEES_PER_USD = 95; // repo-wide convention

function readRows() {
  if (!existsSync(LOG_DIR)) return [];
  const rows = [];
  for (const file of readdirSync(LOG_DIR).filter((f) => f.endsWith('.ndjson'))) {
    for (const line of readFileSync(join(LOG_DIR, file), 'utf8').split('\n')) {
      if (!line.trim()) continue;
      try { rows.push(JSON.parse(line)); } catch { /* a partially-written last line is normal */ }
    }
  }
  return rows;
}

const percentile = (sorted, p) =>
  sorted.length === 0 ? 0 : sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))];

function main() {
  let rows = readRows();
  const cutoff = windowHours ? Date.now() / 1000 - windowHours * 3600 : null;
  if (cutoff !== null) rows = rows.filter((r) => (r.ts ?? 0) >= cutoff);

  if (rows.length === 0) {
    console.error(`FAIL: read 0 rows from ${LOG_DIR}/. A report over an empty set proves nothing.`);
    return 6;
  }

  const byEvent = new Map();
  for (const r of rows) byEvent.set(r.event, (byEvent.get(r.event) ?? 0) + 1);

  // --- money -------------------------------------------------------------------------------------
  const settled = rows.filter((r) => r.event === 'cost.settled');
  const totalPaise = settled.reduce((sum, r) => sum + (r.cost_paise ?? 0), 0);
  const sessions = new Set(settled.map((r) => r.session_id));
  const exceeded = byEvent.get('cost.reservation_exceeded') ?? 0;

  // --- latency + tokens, per model ---------------------------------------------------------------
  const completions = rows.filter((r) => r.event === 'gateway_client.completion');
  const perModel = new Map();
  for (const r of completions) {
    // The model name is not always on the row; fall back to a single bucket rather than inventing
    // per-model numbers that would look authoritative and be wrong.
    const key = r.model ?? r.provider ?? '(model not recorded)';
    const bucket = perModel.get(key) ?? {
      n: 0, latencies: [], tokensIn: 0, tokensOut: 0, cacheRead: 0,
      // A T0 `memory` reply answers in ~60ms and costs nothing. Averaged in with a real provider it
      // does not just add noise — it drags p50 below anything a user could ever experience, which
      // is a true measurement of the wrong quantity. So fake calls are counted and shown, never
      // mixed into the real-provider verdict.
      adapter: r.adapter ?? null, fake: r.is_fake_adapter === true,
    };
    bucket.n += 1;
    if (typeof r.latency_ms === 'number') bucket.latencies.push(r.latency_ms);
    // Field names verified against a real row, not assumed: the gateway logs
    // {input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens}. The first version of
    // this script guessed `llm_tokens_in`/`prompt_tokens` and reported 0 tokens per call, which
    // looked like a broken token meter when the meter was fine and the reader was wrong.
    const usage = r.usage ?? {};
    bucket.tokensIn += usage.input_tokens ?? usage.llm_tokens_in ?? usage.prompt_tokens ?? 0;
    bucket.tokensOut += usage.output_tokens ?? usage.llm_tokens_out ?? usage.completion_tokens ?? 0;
    bucket.cacheRead += usage.cache_read_tokens ?? 0;
    perModel.set(key, bucket);
  }

  // --- quality signals ---------------------------------------------------------------------------
  const responses = rows.filter((r) => r.event === 'conversation.response');
  const degraded = responses.filter((r) => r.degraded);
  const degradeReasons = new Map();
  for (const r of degraded) degradeReasons.set(r.degrade_reason, (degradeReasons.get(r.degrade_reason) ?? 0) + 1);
  const vetoes = rows.filter((r) => r.event === 'conversation.safety_veto');
  const vetoControls = new Map();
  for (const r of vetoes) vetoControls.set(r.control, (vetoControls.get(r.control) ?? 0) + 1);
  const atomizerAttempts = byEvent.get('atomizer.llm_attempt') ?? 0;
  const atomizerRejected = byEvent.get('atomizer.rejected') ?? 0;

  const report = {
    window: windowHours ? `last ${windowHours}h` : 'all recorded',
    rows: rows.length,
    money: {
      total_paise: totalPaise,
      total_rupees: totalPaise / PAISE_PER_RUPEE,
      total_usd: totalPaise / PAISE_PER_RUPEE / RUPEES_PER_USD,
      sessions: sessions.size,
      paise_per_session: sessions.size ? Math.round(totalPaise / sessions.size) : 0,
      reservation_exceeded: exceeded,
    },
    models: [...perModel.entries()].map(([model, b]) => {
      const sorted = [...b.latencies].sort((x, y) => x - y);
      return {
        model,
        adapter: b.adapter,
        fake: b.fake,
        calls: b.n,
        latency_p50_ms: percentile(sorted, 50),
        latency_p95_ms: percentile(sorted, 95),
        tokens_in: b.tokensIn,
        tokens_out: b.tokensOut,
        tokens_per_call: b.n ? Math.round((b.tokensIn + b.tokensOut) / b.n) : 0,
        // Prompt-cache hit rate is the biggest single cost lever on a long system prompt, and it is
        // invisible unless someone divides these two numbers.
        cache_read_tokens: b.cacheRead,
        cache_hit_pct: b.tokensIn ? Math.round((1000 * b.cacheRead) / b.tokensIn) / 10 : 0,
      };
    }),
    quality: {
      responses: responses.length,
      degraded: degraded.length,
      degraded_pct: responses.length ? Math.round((1000 * degraded.length) / responses.length) / 10 : 0,
      degrade_reasons: Object.fromEntries(degradeReasons),
      safety_vetoes: vetoes.length,
      veto_controls: Object.fromEntries(vetoControls),
      atomizer_attempts: atomizerAttempts,
      atomizer_rejected: atomizerRejected,
      atomizer_reject_pct: atomizerAttempts
        ? Math.round((1000 * atomizerRejected) / atomizerAttempts) / 10
        : 0,
    },
  };

  if (asJson) { console.log(JSON.stringify(report, null, 2)); return 0; }

  const m = report.money;
  console.log(`ORB PROVIDER REPORT — ${report.window}, ${report.rows} log rows\n`);
  console.log('MONEY');
  console.log(`  total            ₹${m.total_rupees.toFixed(2)}  ($${m.total_usd.toFixed(2)})  ${m.total_paise} paise`);
  console.log(`  sessions         ${m.sessions}`);
  console.log(`  per session      ₹${(m.paise_per_session / PAISE_PER_RUPEE).toFixed(2)}`);
  console.log(`  budget exceeded  ${m.reservation_exceeded}${m.reservation_exceeded > 0 ? '   <-- sessions are hitting the reservation ceiling' : ''}`);
  console.log('\nMODEL CALLS  (real providers first; T0 fakes are listed but never averaged in)');
  if (report.models.length === 0) console.log('  (no gateway completions recorded in this window)');
  const real = report.models.filter((r) => !r.fake && r.model !== '(model not recorded)');
  const fake = report.models.filter((r) => r.fake);
  const unattributed = report.models.filter((r) => !r.fake && r.model === '(model not recorded)');
  for (const row of real) {
    console.log(`  ${row.model}${row.adapter ? `  [adapter=${row.adapter}]` : ''}`);
    console.log(`    calls ${row.calls} · p50 ${row.latency_p50_ms}ms · p95 ${row.latency_p95_ms}ms`);
    console.log(`    tokens in ${row.tokens_in} / out ${row.tokens_out} · ${row.tokens_per_call}/call`);
    console.log(`    prompt cache ${row.cache_hit_pct}% of input tokens served from cache`);
  }
  if (real.length === 0) console.log('  (no REAL provider calls in this window — the numbers below are T0 fakes)');
  if (real.length > 1) console.log(`  ^ ${real.length} real models side by side — this is the provider comparison.`);
  for (const row of fake) {
    console.log(`  ${row.model} [T0 FAKE — excluded from the verdict] calls ${row.calls} · p50 ${row.latency_p50_ms}ms`);
  }
  for (const row of unattributed) {
    console.log(`  ${row.model}: ${row.calls} calls logged before provider identity was recorded`);
    console.log('    (historical rows only — new rows carry model/adapter/is_fake_adapter)');
  }
  const q = report.quality;
  console.log('\nQUALITY');
  console.log(`  replies          ${q.responses}`);
  console.log(`  degraded         ${q.degraded} (${q.degraded_pct}%)`);
  for (const [reason, n] of Object.entries(q.degrade_reasons)) console.log(`      ${reason}: ${n}`);
  console.log(`  safety vetoes    ${q.safety_vetoes}`);
  for (const [control, n] of Object.entries(q.veto_controls)) console.log(`      ${control}: ${n}`);
  console.log(`  atomizer         ${q.atomizer_rejected}/${q.atomizer_attempts} rejected (${q.atomizer_reject_pct}%)`);

  console.log('\nWHAT WOULD MEAN "CHANGE THE PROVIDER"');
  console.log('  · p95 latency above the turn budget on the voice path');
  console.log('  · degraded % rising without a matching code change');
  console.log('  · atomizer reject % high — the model is not producing usable structure');
  console.log('  · cost per session trending toward the ₹4 reservation');
  console.log('  · prompt-cache hit rate falling — a prompt edit invalidated the cached prefix');
  console.log('\nNOT COVERED (say it rather than imply it):');
  console.log('  · rows written BEFORE provider identity was added stay in one unattributed bucket.');
  console.log('    They are shown separately above and are not part of any per-model comparison —');
  console.log('    re-run the chain to accumulate attributable rows.');
  console.log('  · no LLM-as-judge score runs in production; quality here is degradation and veto');
  console.log('    counts, which are proxies for quality, not quality.');
  return 0;
}

process.exit(main());
