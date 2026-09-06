// backends.mjs — the two real harnesses behind the UI, probed honestly and streamed live.
//
// This mirrors registry/services/dispatch/dispatch.sh's contract (same two harnesses, same
// FLEET_CODEX_BIN / FLEET_CLAUDE_BIN fixture-injection idiom) but streams incrementally instead of
// buffering a whole run, because a chat surface that shows nothing for 90 seconds is not a chat
// surface. dispatch.sh remains the gated, receipt-writing path for real work; this is the
// conversational path in front of it.
//
// ON PROBING: a backend is NEVER reported ready because its binary exists. Two orchestration tools
// in this repo's failure corpus were adopted and defended for a full day without either ever
// having run. `probe()` performs a real one-word round trip and reports what actually came back.
// Until it returns, the status is `probing`, not `ready`.

import { spawn } from 'node:child_process';

export const BACKENDS = {
  codex: {
    id: 'codex', label: 'Codex', bin: () => process.env.FLEET_CODEX_BIN || 'codex',
    models: ['codex-default', 'gpt-5.6-luna'], defaultModel: 'codex-default',
  },
  claude: {
    id: 'claude', label: 'Claude', bin: () => process.env.FLEET_CLAUDE_BIN || 'claude',
    models: ['sonnet', 'opus', 'haiku'], defaultModel: 'sonnet',
  },
};

/** Fan-out lanes: the two workers a `/fanout` turn puts on the same task, deliberately different. */
export const LANES = [
  { id: 'sonnet', backend: 'claude', model: 'sonnet', label: 'sonnet' },
  { id: 'codex', backend: 'codex', model: 'codex-default', label: 'codex' },
];

function argv(backend, { model, cwd, sandbox, mcpConfig }) {
  if (backend === 'codex') {
    const a = ['exec', '--skip-git-repo-check', '--json', '-s', sandbox || 'read-only'];
    if (cwd) a.push('-C', cwd);
    if (model && model !== 'codex-default') a.push('-m', model);
    a.push('-');
    return a;
  }
  // claude: stream-json is the only mode carrying usage metadata; plain -p prints the answer alone.
  const a = ['-p', '--output-format', 'stream-json', '--include-partial-messages', '--verbose'];
  if (model && model !== 'claude-default') a.push('--model', model);
  if (cwd) a.push('--add-dir', cwd);
  if (sandbox !== 'read-only') a.push('--permission-mode', 'acceptEdits');
  if (mcpConfig) a.push('--mcp-config', mcpConfig);
  return a;
}

/**
 * Normalise one JSONL line from either backend into UI events.
 * Unknown shapes yield [] rather than throwing: a backend that adds an event type must not crash
 * the UI, and must not be silently reported as having produced nothing either — `raw` carries it.
 */
export function normalise(backend, obj) {
  const out = [];
  if (backend === 'codex') {
    if (obj.type === 'item.completed' || obj.type === 'item.started') {
      const it = obj.item || {};
      const done = obj.type === 'item.completed';
      // codex reports the SAME item twice — once started, once completed. Both carry the item id,
      // so the id is what de-duplicates them; emitting on `completed` alone would lose the live
      // "running now" signal, and emitting on both printed every command twice (observed).
      const id = it.id ?? null;
      if (it.type === 'agent_message' && done && it.text) out.push({ kind: 'text', text: it.text });
      else if (it.type === 'reasoning' && it.text) out.push({ kind: 'reason', text: it.text, id });
      else if (it.type === 'command_execution') out.push({ kind: 'tool', label: `bash ${(it.command || '').slice(0, 80)}`, done, id });
      else if (it.type === 'file_change' || it.type === 'patch_apply') out.push({ kind: 'tool', label: `edit ${(it.path || it.status || '')}`.slice(0, 90), done, id });
      else if (it.type === 'error' && it.message) out.push({ kind: 'warn', text: it.message, id });
      else if (done && it.type) out.push({ kind: 'tool', label: it.type, done, id });
    } else if (obj.type === 'turn.completed') {
      const u = obj.usage || {};
      out.push({ kind: 'usage', usage: { input: u.input_tokens ?? 0, cached: u.cached_input_tokens ?? 0, output: u.output_tokens ?? 0, costUsd: null } });
    } else if (obj.type === 'turn.failed' || obj.type === 'error') {
      out.push({ kind: 'error', text: obj.error?.message || obj.message || 'turn failed' });
    }
    return out;
  }
  // claude
  if (obj.type === 'stream_event' && obj.event?.type === 'content_block_delta' && obj.event.delta?.type === 'text_delta') {
    out.push({ kind: 'delta', text: obj.event.delta.text });
  } else if (obj.type === 'assistant' && Array.isArray(obj.message?.content)) {
    for (const block of obj.message.content) {
      if (block.type === 'text' && block.text) out.push({ kind: 'text', text: block.text });
      else if (block.type === 'tool_use') out.push({ kind: 'tool', label: `${block.name} ${JSON.stringify(block.input || {}).slice(0, 70)}`, done: false });
    }
  } else if (obj.type === 'result') {
    const u = obj.usage || {};
    out.push({ kind: 'usage', usage: { input: u.input_tokens ?? 0, cached: u.cache_read_input_tokens ?? 0, output: u.output_tokens ?? 0, costUsd: obj.total_cost_usd ?? null } });
    // `is_error` is the backend's own verdict and outranks its exit code. The observed 403 on this
    // machine exits 0 while carrying is_error:true and the real reason in `result`.
    if (obj.is_error) out.push({ kind: 'error', text: obj.result || obj.api_error_status || 'backend reported an error' });
  }
  return out;
}

/**
 * Run one turn. Resolves when the process exits; `onEvent` fires as output arrives.
 * Never rejects on a backend failure — the failure IS the result, and swallowing it into a throw
 * is how a broken path reports success somewhere upstream.
 */
export function run(backend, { prompt, cwd, model, sandbox = 'read-only', timeoutMs = 900000, mcpConfig, onEvent = () => {}, signal }) {
  const cfg = BACKENDS[backend];
  if (!cfg) return Promise.resolve({ ok: false, exit: 2, reason: `unknown backend '${backend}'`, text: '', usage: null });
  const args = argv(backend, { model, cwd, sandbox, mcpConfig });

  return new Promise(resolve => {
    let child;
    try {
      child = spawn(cfg.bin(), args, { cwd: cwd || process.cwd(), stdio: ['pipe', 'pipe', 'pipe'], signal });
    } catch (e) {
      return resolve({ ok: false, exit: 3, reason: `cannot spawn ${cfg.bin()}: ${e.message}`, text: '', usage: null });
    }

    let stdoutBuf = '', stderrTail = '', text = '', usage = null, errText = null, sawDelta = false, unparsed = 0, settled = false;
    const shown = new Set();   // item ids already surfaced, so a started/completed pair prints once
    const finish = r => { if (settled) return; settled = true; clearTimeout(timer); resolve(r); };
    const timer = setTimeout(() => { try { child.kill('SIGKILL'); } catch { /* deliberate: the backend already exited (ESRCH); the timeout verdict below is still the truth */ } finish({ ok: false, exit: 1, reason: `timeout after ${Math.round(timeoutMs / 1000)}s`, text, usage, timedOut: true }); }, timeoutMs);

    const consume = line => {
      if (!line.trim()) return;
      let obj;
      try { obj = JSON.parse(line); } catch { unparsed++; onEvent({ kind: 'raw', text: line }); return; }
      for (const ev of normalise(backend, obj)) {
        if (ev.kind === 'delta') { sawDelta = true; text += ev.text; onEvent(ev); }
        // A backend that streamed deltas already delivered this text; taking the final block too
        // would print the whole answer twice.
        else if (ev.kind === 'text') { if (!sawDelta) { text += (text ? '\n' : '') + ev.text; onEvent(ev); } }
        else {
          if (ev.id != null) { const key = `${ev.kind}:${ev.id}`; if (shown.has(key)) continue; shown.add(key); }
          if (ev.kind === 'usage') usage = ev.usage;
          if (ev.kind === 'error') errText = ev.text;
          onEvent(ev);
        }
      }
    };

    child.stdout.setEncoding('utf8');
    child.stdout.on('data', chunk => {
      stdoutBuf += chunk;
      let nl;
      while ((nl = stdoutBuf.indexOf('\n')) !== -1) { consume(stdoutBuf.slice(0, nl)); stdoutBuf = stdoutBuf.slice(nl + 1); }
    });
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', c => { stderrTail = (stderrTail + c).slice(-4000); });

    child.on('error', e => finish({ ok: false, exit: 3, reason: `${cfg.bin()}: ${e.message}`, text, usage }));
    child.on('close', code => {
      if (stdoutBuf.trim()) consume(stdoutBuf);
      if (errText) return finish({ ok: false, exit: 1, reason: errText, text, usage, stderr: stderrTail });
      if (code !== 0) return finish({ ok: false, exit: code === null ? 1 : 1, reason: `${cfg.bin()} exited ${code}${stderrTail ? `: ${stderrTail.trim().split('\n').slice(-2).join(' ')}` : ''}`, text, usage, stderr: stderrTail });
      // Exit 0 with nothing parseable is NOT a success. A silently-empty turn reported as ok is
      // the "silently wrong output" the L8 bar forbids; dispatch.sh calls this exit 4.
      if (!text.trim()) return finish({ ok: false, exit: 4, reason: `exit 0 but no assistant text (${unparsed} unparseable line(s))${stderrTail ? `: ${stderrTail.trim().split('\n').slice(-1)[0]}` : ''}`, text, usage, stderr: stderrTail });
      finish({ ok: true, exit: 0, text, usage, stderr: stderrTail });
    });

    child.stdin.on('error', () => {}); // the backend may exit before reading; not our failure to raise
    child.stdin.end(prompt);
  });
}

/** A real one-word round trip. The only evidence that a backend is usable at all. */
export async function probe(backend, { timeoutMs = 75000, cwd } = {}) {
  const started = Date.now();
  const r = await run(backend, {
    prompt: 'Reply with the single word: OK', cwd, sandbox: 'read-only', timeoutMs,
    model: BACKENDS[backend]?.defaultModel,
  });
  const ms = Date.now() - started;
  if (r.ok) return { status: 'ready', ms, detail: r.text.trim().slice(0, 40), usage: r.usage };
  const missing = /ENOENT|cannot spawn|not found/i.test(r.reason || '');
  return { status: missing ? 'missing' : 'unavailable', ms, reason: (r.reason || 'unknown failure').trim() };
}
