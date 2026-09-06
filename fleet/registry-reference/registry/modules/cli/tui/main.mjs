// main.mjs — `fleet` with no verb: the conversational surface over fleet's own machinery.
//
// The shape is copied deliberately from a pty capture of `claude`'s TUI (100x34): a welcome box,
// a scrollback transcript, a pinned input with a rule above and below, a completion popup that
// opens on `/`, and a mode footer cycled with shift+tab. What is behind it is fleet's, not
// Claude Code's: fleet's skills, fleet's slash verbs, fleet's two-harness fan-out, and fleet's
// refusal to report a backend as working before it has actually answered once.
//
// Modes (shift+tab cycles):
//   solo    one lead backend answers, streamed token by token
//   fanout  both harnesses on the same task, role-split: one builds, one independently verifies
//   fleet   hands off to `fleet run`, the gated SDLC path with intake, SOW, and receipts
//
// Nothing here writes to ledger/RECEIPTS.jsonl. Conversational turns are not receipts, and a
// measurement run that appends to the production chain is how 776 rows once broke it permanently.

import { spawn } from 'node:child_process';
import { readdir, readFile, writeFile, mkdir } from 'node:fs/promises';
import { join, dirname, basename, resolve as pathResolve } from 'node:path';
import { homedir } from 'node:os';
import { discoverAll } from './discover.mjs';
import { probe, BACKENDS, LANES } from './backends.mjs';
import { buildPrompt, loadMatches, houseRules, gitContext, turn, fanout, backendGate } from './session.mjs';
import { Screen, box, popup, wrap, truncate, pad, width, tokenize, c, SPINNER } from './ui.mjs';

const FLEET_ROOT = process.env.FLEET_ROOT || pathResolve(import.meta.dirname, '..', '..', '..', '..');
const CWD = process.env.FLEET_TUI_CWD || process.cwd();
const HISTORY_FILE = join(homedir(), '.fleet', 'tui-history');
const VERSION = '0.1.0';
const MODES = ['solo', 'fanout', 'fleet'];

const screen = new Screen(process.stdout);
const state = {
  buf: '', cur: 0, mode: 'solo', busy: null, history: [], hIdx: -1, draft: '',
  popup: null, sel: 0, discovery: null, capability: {}, git: null, rules: [],
  turns: [], tokens: { input: 0, output: 0 }, lastCtrlC: 0, spin: 0, lanes: null, startedAt: 0,
  probing: null,   // the in-flight probeAll() promise, so a turn can WAIT rather than refuse
};

// ---------------------------------------------------------------- commands --
const BUILTINS = [
  { name: '/help', description: 'commands, modes, and keys' },
  { name: '/mode', description: `switch mode: ${MODES.join(' | ')} (shift+tab cycles)` },
  { name: '/backends', description: 'harness capability — probed, not assumed' },
  { name: '/skills', description: 'skills discovered for this workspace' },
  { name: '/mcp', description: 'MCP servers declared here, and whether they resolve' },
  { name: '/verbs', description: 'the fleet <verb> table, runnable as /verb' },
  { name: '/cost', description: 'tokens spent in this session' },
  { name: '/cwd', description: 'the workspace this session is scoped to' },
  { name: '/clear', description: 'clear the transcript and conversation memory' },
  { name: '/exit', description: 'leave (ctrl+d, or ctrl+c twice)' },
];

function commandList() {
  const d = state.discovery;
  const out = [...BUILTINS];
  for (const cm of d?.commands ?? []) out.push({ name: `/${cm.name}`, description: cm.description || `command · ${cm.origin}`, kind: 'command', ref: cm });
  for (const v of d?.verbs ?? []) {
    if (out.some(o => o.name === `/${v.name}`)) continue;
    out.push({ name: `/${v.name}`, description: `fleet ${v.name} — ${v.description}`, kind: 'verb', ref: v });
  }
  return out;
}

// ------------------------------------------------------------------ render --
function statusRight() {
  const parts = [];
  const ready = Object.entries(state.capability).filter(([, v]) => v.status === 'ready').map(([k]) => k);
  const probing = Object.entries(state.capability).filter(([, v]) => v.status === 'probing').map(([k]) => k);
  if (probing.length) parts.push(c.yellow(`probing ${probing.join(',')}`));
  parts.push(ready.length ? c.green(`● ${ready.join(' + ')}`) : c.red('● no backend'));
  if (state.tokens.output) parts.push(c.grey(`${(state.tokens.input / 1000).toFixed(1)}k in · ${state.tokens.output} out`));
  parts.push(c.grey(`${state.mode} · /mode`));
  return parts.join(c.grey(' · '));
}

function laneLines(cols) {
  if (!state.lanes) return [];
  const el = ((Date.now() - state.startedAt) / 1000).toFixed(0);
  return [...state.lanes.entries()].map(([id, l]) => {
    const glyph = l.state === 'running' ? c.cyan(SPINNER[state.spin % SPINNER.length])
      : l.state === 'ok' ? c.green('✔') : l.state === 'skipped' ? c.yellow('⊘') : c.red('✖');
    const role = l.role ? c.grey(`[${l.role}]`) : '';
    return truncate(`  ${glyph} ${pad(id, 8)} ${role} ${c.grey(l.note || '')}${l.state === 'running' ? c.grey(` ${el}s`) : ''}`, cols);
  });
}

function busyLine(cols) {
  if (!state.busy) return [];
  const el = ((Date.now() - state.startedAt) / 1000).toFixed(0);
  return [truncate(`${c.cyan(SPINNER[state.spin % SPINNER.length])} ${state.busy} ${c.grey(`${el}s · esc to interrupt`)}`, cols)];
}

function render() {
  const cols = screen.cols;
  const lines = [];
  if (state.popup?.items.length) lines.push(...popup(state.popup.items, state.sel, cols));
  lines.push(...laneLines(cols));
  lines.push(...busyLine(cols));
  const right = statusRight();
  lines.push(' '.repeat(Math.max(0, cols - width(right))) + right);
  lines.push(c.grey('─'.repeat(cols)));

  // The prompt line scrolls horizontally rather than wrapping: a wrapped sticky line breaks the
  // redraw accounting and eats a row of transcript on every repaint.
  const promptGlyph = state.busy ? c.grey('❯') : c.cyan('❯');
  const avail = cols - 2;
  let shown = state.buf, off = 0;
  if (width(shown) > avail) { off = Math.max(0, state.cur - avail + 1); shown = shown.slice(off, off + avail); }
  const placeholder = state.buf ? '' : c.grey(state.busy ? 'working…' : 'ask, or / for commands');
  lines.push(`${promptGlyph} ${shown}${placeholder}`);
  const caret = { line: lines.length - 1, col: 2 + Math.max(0, state.cur - off) };

  lines.push(c.grey('─'.repeat(cols)));
  const modeHint = state.mode === 'solo' ? 'one lead backend'
    : state.mode === 'fanout' ? 'both harnesses, role-split (build ‖ verify)'
    : 'gated SDLC via fleet run';
  lines.push(c.grey(`  ⏵⏵ ${state.mode} mode — ${modeHint} (shift+tab to cycle)`));
  screen.setSticky(lines, caret);
}

let spinTimer = null;
function startSpin() { if (spinTimer) return; state.startedAt = Date.now(); spinTimer = setInterval(() => { state.spin++; render(); }, 120); }
function stopSpin() { if (spinTimer) clearInterval(spinTimer); spinTimer = null; }

// -------------------------------------------------------------- transcript --
const say = (glyph, text, colour = c.white) => {
  const cols = screen.cols;
  const lines = wrap(String(text), cols - 3);
  screen.emit(lines.map((l, i) => `${i === 0 ? colour(glyph) : ' '} ${l}`).join('\n'));
};
const info = t => say('◇', t, c.blue);
const warn = t => say('▲', t, c.yellow);
const fail = t => say('✖', t, c.red);
const note = t => screen.emit(c.grey(`  ${truncate(t, screen.cols - 2)}`));

function banner() {
  const cols = Math.min(screen.cols, 110);
  const left = [
    '', c.bold('  fleet'), '',
    '   ▐▛███▜▌', '  ▝▜█████▛▘', '    ▘▘ ▝▝', '',
    c.grey(`  ${state.mode} · ${basename(CWD)}`),
  ];
  const d = state.discovery;
  const right = [
    c.bold('Ready when you are'),
    `${d.skills.length} skills · ${d.commands.length} commands · ${d.verbs.length} verbs`,
    `${d.mcp.filter(s => s.status === 'available').length}/${d.mcp.length} mcp resolved`,
    c.grey('─'.repeat(Math.max(0, Math.floor(cols * 0.55) - 4))),
    c.bold('Getting started'),
    c.grey('/ for commands · shift+tab for mode · ? in /help'),
    c.grey('fan-out puts both harnesses on one task'),
    '',
  ];
  const lw = Math.floor(cols * 0.4), rw = cols - lw - 5;
  const rows = [];
  for (let i = 0; i < Math.max(left.length, right.length); i++) {
    rows.push(`${pad(truncate(left[i] ?? '', lw), lw)} ${c.grey('│')} ${truncate(right[i] ?? '', rw)}`);
  }
  screen.emit('');
  screen.emit(box(`fleet v${VERSION}`, rows, cols).join('\n'));
  screen.emit('');
  note(CWD);
  if (state.git) note(`git ${state.git.branch} · ${state.git.changed} uncommitted · ${state.git.head}`);
}

// ---------------------------------------------------------------- backends --
async function probeAll() {
  for (const id of Object.keys(BACKENDS)) state.capability[id] = { status: 'probing' };
  render();
  await Promise.all(Object.keys(BACKENDS).map(async id => {
    const r = await probe(id, { cwd: CWD });
    state.capability[id] = r;
    if (r.status === 'ready') info(`${id} ready (${(r.ms / 1000).toFixed(1)}s)`);
    else {
      warn(`${id} ${r.status}: ${r.reason}`);
      if (id === 'claude' && /organization has disabled|api key/i.test(r.reason || ''))
        note('fix: export ANTHROPIC_API_KEY=…  (or ask the org admin to re-enable Claude Code subscription access)');
      if (r.status === 'missing') note(`fix: install the ${id} CLI, or set FLEET_${id.toUpperCase()}_BIN`);
    }
    render();
  }));
  const ready = Object.values(state.capability).filter(v => v.status === 'ready').length;
  if (!ready) fail('no harness can reach a model. Chat turns will refuse rather than pretend.');
  else if (ready < 2 && state.mode === 'fanout') warn('fan-out has one usable lane; it will run solo and say so.');
  render();
}

// ------------------------------------------------------------------- turns --
function mkLanes(ids) { state.lanes = new Map(ids.map(id => [id, { state: 'running', note: '', role: null }])); }

/** Stream a solo turn's text into scrollback line by line. */
function makeStreamer() {
  let pending = '';
  return {
    push(text) {
      pending += text;
      let nl;
      while ((nl = pending.indexOf('\n')) !== -1) {
        const line = pending.slice(0, nl); pending = pending.slice(nl + 1);
        for (const l of wrap(line, screen.cols - 3)) screen.emit(`  ${l}`);
      }
    },
    flush() { if (pending.trim()) for (const l of wrap(pending, screen.cols - 3)) screen.emit(`  ${l}`); pending = ''; },
  };
}

function accountUsage(u) { if (!u) return; state.tokens.input += u.input || 0; state.tokens.output += u.output || 0; }

async function runSolo(message, ctl) {
  // runTurn has already held this message until the probe settled; this is the defensive floor.
  const lead = ['claude', 'codex'].find(id => state.capability[id]?.status === 'ready');
  if (!lead) { fail('no ready backend — your message was NOT sent. /backends shows why.'); return; }
  const matches = await loadMatches(state.discovery.skills, message, 3);
  if (matches.length) info(`skills: ${matches.map(m => `${m.skill.name}(${m.score})`).join(', ')}`);
  const prompt = buildPrompt({
    message, cwd: CWD, role: 'solo', rules: state.rules, skills: matches,
    mcp: state.discovery.mcp, git: state.git, history: state.turns.slice(-6),
  });
  state.busy = `${lead} · ${BACKENDS[lead].defaultModel}`;
  startSpin(); render();
  const stream = makeStreamer();
  screen.emit('');
  const r = await turn({
    backend: lead, model: BACKENDS[lead].defaultModel, prompt, cwd: CWD,
    sandbox: 'workspace-write', signal: ctl.signal,
    onEvent: ev => {
      if (ev.kind === 'delta' || ev.kind === 'text') stream.push(ev.kind === 'text' ? ev.text + '\n' : ev.text);
      else if (ev.kind === 'tool') { stream.flush(); note(`⚒ ${ev.label}`); }
      else if (ev.kind === 'warn') { stream.flush(); note(`▲ ${ev.text}`); }
      else if (ev.kind === 'usage') accountUsage(ev.usage);
    },
  });
  stream.flush(); stopSpin(); state.busy = null;
  if (!r.ok) fail(`${lead}: ${r.reason}`);
  else { state.turns.push({ role: 'user', text: message }, { role: 'assistant', text: r.text }); }
  screen.emit('');
  render();
}

async function runFanout(message, ctl) {
  const matches = await loadMatches(state.discovery.skills, message, 3);
  if (matches.length) info(`skills: ${matches.map(m => `${m.skill.name}(${m.score})`).join(', ')}`);
  const common = { message, cwd: CWD, rules: state.rules, skills: matches, mcp: state.discovery.mcp, git: state.git, history: state.turns.slice(-4) };
  const prompts = {
    build: buildPrompt({ ...common, role: 'build' }),
    verify: buildPrompt({ ...common, role: 'verify' }),
    solo: buildPrompt({ ...common, role: 'solo' }),
  };
  mkLanes(LANES.map(l => l.id));
  state.busy = 'fan-out'; startSpin(); render();

  const results = await fanout({
    prompts, capability: state.capability, cwd: CWD, sandbox: 'workspace-write', signal: ctl.signal,
    onEvent: ev => {
      const l = state.lanes.get(ev.lane); if (!l) return;
      if (ev.kind === 'start') l.role = ev.role;
      else if (ev.kind === 'tool') l.note = ev.label;
      else if (ev.kind === 'usage') { accountUsage(ev.usage); l.note = `${ev.usage.output} out`; }
      else if (ev.kind === 'skipped') { l.state = 'skipped'; l.note = ev.text; }
      else if (ev.kind === 'done') { l.state = ev.ok ? 'ok' : 'failed'; l.note = ev.ok ? 'complete' : (ev.reason || 'failed'); }
      render();
    },
  });
  stopSpin(); state.busy = null;

  // Each lane's full answer goes to scrollback only once it has settled: two concurrent token
  // streams interleaved in one column is unreadable, and a reader cannot tell which model said what.
  for (const r of results) {
    screen.emit('');
    const tag = r.status === 'ok' ? c.green(`── ${r.lane.label} [${r.lane.role}] ──`)
      : r.status === 'skipped' ? c.yellow(`── ${r.lane.label} skipped ──`)
      : c.red(`── ${r.lane.label} failed ──`);
    screen.emit(tag);
    if (r.status === 'ok') { for (const l of wrap(r.result.text, screen.cols - 3)) screen.emit(`  ${l}`); state.turns.push({ role: 'assistant', text: `[${r.lane.label}] ${r.result.text}` }); }
    else note(r.reason || r.result?.reason || 'no reason reported');
  }
  state.turns.unshift({ role: 'user', text: message });
  const ok = results.filter(r => r.status === 'ok').length;
  screen.emit('');
  info(`fan-out: ${ok}/${results.length} lanes produced output` + (ok < results.length ? ' — the rest are named above, not hidden' : ''));
  state.lanes = null; screen.emit(''); render();
}

async function runFleet(message) {
  info(`handing off to the gated path: fleet run "${truncate(message, 60)}"`);
  note('this is intake → clarifying questions → SOW → role fan-out. It refuses on ambiguity by design.');
  await runShell(join(FLEET_ROOT, 'fleet'), ['run', message], FLEET_ROOT);
}

/** Run a fleet verb or a shell command, streaming its output into the transcript. */
function runShell(cmd, args, cwd) {
  return new Promise(res => {
    state.busy = `${basename(cmd)} ${args[0] ?? ''}`; startSpin(); render();
    const child = spawn(cmd, args, { cwd: cwd || CWD, stdio: ['ignore', 'pipe', 'pipe'] });
    let buf = '';
    const pump = chunk => {
      buf += chunk; let nl;
      while ((nl = buf.indexOf('\n')) !== -1) { const line = buf.slice(0, nl); buf = buf.slice(nl + 1); screen.emit(c.grey('  ' + truncate(line, screen.cols - 2))); }
    };
    child.stdout.setEncoding('utf8'); child.stdout.on('data', pump);
    child.stderr.setEncoding('utf8'); child.stderr.on('data', pump);
    child.on('error', e => { stopSpin(); state.busy = null; fail(`${cmd}: ${e.message}`); render(); res(1); });
    child.on('close', code => {
      if (buf.trim()) screen.emit(c.grey('  ' + truncate(buf, screen.cols - 2)));
      stopSpin(); state.busy = null;
      (code === 0 ? info : warn)(`exit ${code}`);
      render(); res(code);
    });
  });
}

// ---------------------------------------------------------------- dispatch --
async function handle(input) {
  const text = input.trim();
  if (!text) return;
  state.history.unshift(text); state.hIdx = -1;
  saveHistory().catch(() => { /* deliberate: ~/.fleet history is a convenience file; failing to persist it must not abort the turn the operator asked for */ });
  say('❯', text, c.cyan);

  if (text.startsWith('/')) {
    const [word, ...rest] = text.slice(1).split(/\s+/);
    const arg = rest.join(' ');
    const builtin = BUILTINS.find(b => b.name === `/${word}`);
    if (builtin) return runBuiltin(word, arg);
    const cmd = state.discovery.commands.find(cm => cm.name === word);
    if (cmd) {
      info(`command /${word} (${cmd.origin}) → ${state.mode} turn`);
      return runTurn(`${cmd.prompt}\n\n${arg}`.trim());
    }
    const verb = state.discovery.verbs.find(v => v.name === word);
    if (verb) return runShell(join(FLEET_ROOT, 'fleet'), [word, ...rest], FLEET_ROOT);
    warn(`unknown command /${word} — press / to see what exists`);
    return;
  }
  return runTurn(text);
}

/**
 * Hold a message until the harness probe has actually settled.
 * Returns false only when nothing can reach a model, or the operator interrupted the wait.
 */
async function awaitBackends(ctl) {
  let gate = backendGate(state.capability);
  if (gate.decision === 'go') return true;
  if (gate.decision === 'wait' && state.probing) {
    state.busy = `waiting for the ${gate.probing.join(' + ')} probe`;
    startSpin(); render();
    await Promise.race([
      state.probing,
      new Promise(res => ctl?.signal?.addEventListener('abort', res, { once: true })),
    ]);
    stopSpin(); state.busy = null; render();
    if (ctl?.signal?.aborted) return false;
    gate = backendGate(state.capability);
  }
  if (gate.decision === 'go') return true;
  fail('no harness can reach a model — your message was NOT sent.');
  for (const b of gate.blocked) note(`${b.id}: ${b.reason}`);
  note('/backends re-reads the probe; ↑ recalls the message.');
  return false;
}

let inflight = null;
async function runTurn(message) {
  if (inflight) { warn('a turn is already running — esc interrupts it'); return; }
  inflight = new AbortController();
  try {
    // `fleet` mode shells out to the gated pipeline and needs no harness of its own.
    if (state.mode !== 'fleet' && !(await awaitBackends(inflight))) {
      if (inflight.signal.aborted) warn('cancelled while waiting for the probe — ↑ recalls the message');
      return;
    }
    if (state.mode === 'fleet') await runFleet(message);
    else if (state.mode === 'fanout') await runFanout(message, inflight);
    else await runSolo(message, inflight);
  } catch (e) {
    stopSpin(); state.busy = null; state.lanes = null;
    if (e?.name === 'AbortError') warn('interrupted'); else fail(`turn failed: ${e?.message || e}`);
    render();
  } finally { inflight = null; }
}

async function runBuiltin(word, arg) {
  const d = state.discovery;
  switch (word) {
    case 'help': {
      screen.emit('');
      info('modes — shift+tab cycles');
      note('solo    one lead backend answers, streamed');
      note('fanout  both harnesses on one task, role-split: one builds, one verifies read-only');
      note('fleet   hands off to `fleet run` — intake, SOW, role SDLC, receipts');
      info('keys');
      note('/ commands · @ files · ↑↓ history · esc interrupt/clear · ctrl+c twice or ctrl+d to exit');
      info(`commands (${commandList().length})`);
      for (const b of BUILTINS) note(`${pad(b.name, 12)} ${b.description}`);
      note(`… plus ${d.commands.length} workspace commands and ${d.verbs.length} fleet verbs — press /`);
      screen.emit(''); return;
    }
    case 'mode': {
      if (arg && MODES.includes(arg)) state.mode = arg;
      else if (arg) { warn(`unknown mode '${arg}' — one of ${MODES.join(', ')}`); return; }
      else state.mode = MODES[(MODES.indexOf(state.mode) + 1) % MODES.length];
      info(`mode: ${state.mode}`); return;
    }
    case 'backends': {
      screen.emit('');
      for (const [id, cap] of Object.entries(state.capability)) {
        const glyph = cap.status === 'ready' ? c.green('●') : cap.status === 'probing' ? c.yellow('◐') : c.red('○');
        say(glyph, `${pad(id, 8)} ${cap.status}${cap.ms ? c.grey(` (${(cap.ms / 1000).toFixed(1)}s)`) : ''}`, x => x);
        if (cap.reason) note(cap.reason);
      }
      note('status is a real one-word round trip, not a `command -v` check.');
      screen.emit(''); return;
    }
    case 'skills': {
      screen.emit(''); info(`${d.skills.length} skills`);
      for (const s of d.skills.slice(0, 40)) note(`${pad(s.name, 26)} ${c.grey(s.origin)}  ${truncate(s.description, screen.cols - 40)}`);
      if (d.skills.length > 40) note(`… and ${d.skills.length - 40} more`);
      if (d.shadowed.length) warn(`${d.shadowed.length} shadowed by a nearer definition: ${d.shadowed.map(s => s.name).join(', ')}`);
      screen.emit(''); return;
    }
    case 'mcp': {
      screen.emit('');
      if (!d.mcp.length) { warn('no MCP servers declared for this workspace'); note('looked in: .mcp.json, .claude/mcp.json, ~/.claude.json'); screen.emit(''); return; }
      for (const s of d.mcp) {
        const glyph = s.status === 'available' ? c.green('●') : s.status === 'declared' ? c.yellow('◐') : c.red('○');
        say(glyph, `${pad(s.name, 24)} ${pad(s.status, 10)} ${c.grey(s.transport)}`, x => x); note(s.detail);
      }
      note(`sources: ${d.mcpSources.join(', ')}`);
      screen.emit(''); return;
    }
    case 'verbs': {
      screen.emit(''); info(`${d.verbs.length} fleet verbs — run any as /<verb>`);
      for (const v of d.verbs) note(`${pad('/' + v.name, 26)} ${truncate(v.description, screen.cols - 30)}`);
      screen.emit(''); return;
    }
    case 'cost': {
      info(`this session: ${state.tokens.input.toLocaleString()} in · ${state.tokens.output.toLocaleString()} out · ${state.turns.length} turns`);
      note('per-call USD is not reported: the codex backend emits no cost field, and inventing one would be a fabricated number.');
      return;
    }
    case 'cwd': { info(CWD); if (state.git) note(`git ${state.git.branch} · ${state.git.changed} uncommitted`); return; }
    case 'clear': { state.turns = []; state.tokens = { input: 0, output: 0 }; process.stdout.write('\x1b[2J\x1b[H'); screen.reset(); banner(); info('transcript and conversation memory cleared'); return; }
    case 'exit': return shutdown(0);
  }
}

// --------------------------------------------------------------- completion --
async function updatePopup() {
  const upto = state.buf.slice(0, state.cur);
  const slash = /(?:^|\s)\/([a-z0-9:-]*)$/i.exec(upto);
  if (slash && state.buf.trimStart().startsWith('/')) {
    const q = slash[1].toLowerCase();
    const items = commandList().filter(i => i.name.slice(1).toLowerCase().startsWith(q));
    state.popup = items.length ? { kind: 'slash', items, token: slash[1] } : null; state.sel = 0; return;
  }
  const at = /(?:^|\s)@([^\s]*)$/.exec(upto);
  if (at) {
    const frag = at[1];
    const dir = frag.includes('/') ? join(CWD, dirname(frag)) : CWD;
    const want = basename(frag).toLowerCase();
    try {
      const entries = await readdir(dir, { withFileTypes: true });
      const items = entries
        .filter(e => !e.name.startsWith('.') && e.name.toLowerCase().startsWith(want))
        .slice(0, 40)
        .map(e => ({ name: (frag.includes('/') ? dirname(frag) + '/' : '') + e.name + (e.isDirectory() ? '/' : ''), description: e.isDirectory() ? 'directory' : 'file' }));
      state.popup = items.length ? { kind: 'file', items, token: frag } : null; state.sel = 0; return;
    } catch { state.popup = null; return; }
  }
  state.popup = null;
}

function acceptCompletion() {
  const p = state.popup; if (!p) return false;
  const pick = p.items[state.sel]; if (!pick) return false;
  const value = p.kind === 'slash' ? pick.name.slice(1) : pick.name;
  const before = state.buf.slice(0, state.cur), after = state.buf.slice(state.cur);
  const cut = before.length - p.token.length;
  const trail = p.kind === 'file' && value.endsWith('/') ? '' : ' ';
  state.buf = before.slice(0, cut) + value + trail + after;
  state.cur = cut + value.length + trail.length;
  state.popup = null; return true;
}

// ---------------------------------------------------------------- history --
async function loadHistory() {
  try { state.history = (await readFile(HISTORY_FILE, 'utf8')).split('\n').filter(Boolean).reverse().slice(0, 500); } catch { state.history = []; }
}
async function saveHistory() {
  await mkdir(dirname(HISTORY_FILE), { recursive: true });
  await writeFile(HISTORY_FILE, state.history.slice(0, 500).reverse().join('\n') + '\n', 'utf8');
}

// ------------------------------------------------------------------- input --
//
// A pty read is NOT a keystroke. Typing a line and pressing return can arrive as ONE chunk
// ("hello\r"), and treating a chunk as an atomic key means return never submits — measured, not
// theorised. So: real pastes are delimited by bracketed-paste markers and inserted verbatim;
// everything else is split into individual keys and dispatched one at a time.
const PASTE_ON = '\x1b[200~', PASTE_OFF = '\x1b[201~';
let pasteBuf = null;


function onData(chunk) {
  if (pasteBuf !== null) {
    const end = chunk.indexOf(PASTE_OFF);
    if (end === -1) { pasteBuf += chunk; return; }
    insertText(pasteBuf + chunk.slice(0, end));
    pasteBuf = null;
    return onData(chunk.slice(end + PASTE_OFF.length));
  }
  const start = chunk.indexOf(PASTE_ON);
  if (start !== -1) {
    onData(chunk.slice(0, start));
    pasteBuf = '';
    return onData(chunk.slice(start + PASTE_ON.length));
  }
  for (const k of tokenize(chunk)) handleKey(k);
}

/** Literal text into the buffer at the caret — used for pastes and printable keys alike. */
function insertText(text) {
  const clean = text.replace(/\r\n?/g, '\n').replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, '');
  if (!clean) return;
  state.buf = state.buf.slice(0, state.cur) + clean + state.buf.slice(state.cur);
  state.cur += clean.length;
  updatePopup().then(render);
}

function handleKey(k) {
  // ---- control
  if (k === '\x03') { // ctrl+c
    if (inflight) { inflight.abort(); stopSpin(); state.busy = null; state.lanes = null; warn('interrupted'); render(); return; }
    const now = Date.now();
    if (now - state.lastCtrlC < 1500) return shutdown(0);
    state.lastCtrlC = now; note('press ctrl+c again to exit'); render(); return;
  }
  if (k === '\x04') { if (!state.buf) return shutdown(0); return; }        // ctrl+d
  if (k === '\x1b' || k === '\x1b\x1b') {                                   // esc
    if (state.popup) { state.popup = null; render(); return; }
    if (inflight) { inflight.abort(); return; }
    if (state.buf) { state.buf = ''; state.cur = 0; render(); }
    return;
  }
  if (k === '\x1b[Z') { state.mode = MODES[(MODES.indexOf(state.mode) + 1) % MODES.length]; render(); return; } // shift+tab
  if (k === '\t') { if (acceptCompletion()) { render(); return; } }
  if (k === '\r' || k === '\n') {
    if (state.popup) { acceptCompletion(); render(); return; }
    const line = state.buf; state.buf = ''; state.cur = 0; state.popup = null; render();
    handle(line).catch(e => { fail(String(e?.message || e)); render(); });
    return;
  }
  // ---- navigation
  if (k === '\x1b[A' || k === '\x1b[B') {                                   // up / down
    if (state.popup) { state.sel = Math.max(0, Math.min(state.popup.items.length - 1, state.sel + (k === '\x1b[A' ? -1 : 1))); render(); return; }
    if (k === '\x1b[A') {
      if (state.hIdx === -1) state.draft = state.buf;
      if (state.hIdx + 1 < state.history.length) { state.hIdx++; state.buf = state.history[state.hIdx]; state.cur = state.buf.length; }
    } else if (state.hIdx >= 0) {
      state.hIdx--; state.buf = state.hIdx === -1 ? state.draft : state.history[state.hIdx]; state.cur = state.buf.length;
    }
    render(); return;
  }
  if (k === '\x1b[D') { state.cur = Math.max(0, state.cur - 1); render(); return; }
  if (k === '\x1b[C') { state.cur = Math.min(state.buf.length, state.cur + 1); render(); return; }
  if (k === '\x01' || k === '\x1b[H') { state.cur = 0; render(); return; }
  if (k === '\x05' || k === '\x1b[F') { state.cur = state.buf.length; render(); return; }
  // ---- editing
  if (k === '\x7f' || k === '\b') {
    if (state.cur > 0) { state.buf = state.buf.slice(0, state.cur - 1) + state.buf.slice(state.cur); state.cur--; }
    return void updatePopup().then(render);
  }
  if (k === '\x1b[3~') { state.buf = state.buf.slice(0, state.cur) + state.buf.slice(state.cur + 1); return void updatePopup().then(render); }
  if (k === '\x17') {                                                        // ctrl+w
    const left = state.buf.slice(0, state.cur).replace(/\s*\S+$/, '');
    state.buf = left + state.buf.slice(state.cur); state.cur = left.length;
    return void updatePopup().then(render);
  }
  if (k === '\x15') { state.buf = state.buf.slice(state.cur); state.cur = 0; return void updatePopup().then(render); } // ctrl+u
  if (k === '\x0c') { process.stdout.write('\x1b[2J\x1b[H'); screen.reset(); render(); return; }                   // ctrl+l
  // ---- printable
  if (k.length === 1 && k.charCodeAt(0) < 32) return;   // unhandled control byte: ignore, never echo
  if (k.startsWith('\x1b')) return;                     // unhandled escape sequence: same
  insertText(k);
}

// ----------------------------------------------------------------- lifecycle --
let shuttingDown = false;
function shutdown(code) {
  if (shuttingDown) return; shuttingDown = true;
  stopSpin();
  try { inflight?.abort(); } catch { /* deliberate: already-aborted controllers throw; we are exiting either way */ }
  screen.close();
  process.stdout.write('\x1b[?2004l');
  if (process.stdin.isTTY) process.stdin.setRawMode(false);
  process.stdout.write(c.grey('  fleet out.\n'));
  process.exit(code);
}

async function boot() {
  if (!process.stdin.isTTY) {
    process.stderr.write('fleet: the UI needs a terminal. Use `fleet <verb>` for non-interactive work,\n' +
      '       or `fleet run "task"` for the gated pipeline.\n');
    process.exit(2);
  }
  process.stdout.write('\x1b[2J\x1b[H');
  const [discovery, git, rules] = await Promise.all([
    discoverAll(CWD, FLEET_ROOT), gitContext(CWD), houseRules(CWD, FLEET_ROOT),
  ]);
  state.discovery = discovery; state.git = git; state.rules = rules;
  await loadHistory();
  banner();
  if (rules.length) note(`house rules loaded: ${rules.map(r => r.label.split(' ')[0]).join(', ')}`);
  render();

  process.stdin.setRawMode(true); process.stdin.resume(); process.stdin.setEncoding('utf8');
  process.stdout.write('\x1b[?2004h');   // bracketed paste: a pasted block stays one block
  process.stdin.on('data', onData);
  process.stdout.on('resize', () => render());
  process.on('SIGINT', () => {});                       // ctrl+c is handled in raw mode, not here
  process.on('SIGTERM', () => shutdown(0));

  state.probing = probeAll().catch(e => { fail(`probe failed: ${e.message}`); render(); });
}

boot().catch(e => { screen.close(); process.stderr.write(`fleet: ${e?.stack || e}\n`); process.exit(1); });
