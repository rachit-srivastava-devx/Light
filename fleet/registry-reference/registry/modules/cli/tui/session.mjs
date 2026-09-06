// session.mjs — what a turn actually sends, and how a fan-out turn is split.
//
// FAN-OUT IS ROLE-SPLIT, NOT DUPLICATED. Two agents given write access to the same tree for the
// same task collide, and this repo has already paid for that once: a correctly-scoped dispatch was
// failed with exit 6 naming three files a DIFFERENT concurrent dispatch had written in the same
// wall-clock window (docs/reports/CYCLE-PROOF.md, "the stage 5c incident"). So the lanes take the
// builder/verifier split the role registry already declares: one writes, one independently checks.
// Both models are on the task; only one has the pen.
//
// If the build lane's backend is unavailable, the roles swap rather than the turn silently
// becoming half a fan-out — and the UI says so.

import { readFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { join } from 'node:path';
import { run, LANES, BACKENDS } from './backends.mjs';
import { matchSkills, body as skillBody } from './discover.mjs';

const exec = (cmd, args, opts) => new Promise(res =>
  execFile(cmd, args, { timeout: 4000, ...opts }, (e, so) => res(e ? '' : String(so).trim())));

export async function gitContext(cwd) {
  const [branch, dirty, head] = await Promise.all([
    exec('git', ['rev-parse', '--abbrev-ref', 'HEAD'], { cwd }),
    exec('git', ['status', '--porcelain'], { cwd }),
    exec('git', ['log', '-1', '--format=%h %s'], { cwd }),
  ]);
  if (!branch) return null;
  const changed = dirty ? dirty.split('\n').filter(Boolean).length : 0;
  return { branch, changed, head };
}

/** fleet's own standing instructions, when the UI is pointed at a repo that has them. */
export async function houseRules(cwd, fleetRoot) {
  const parts = [];
  for (const [label, path] of [
    ['PRINCIPLES.md (fleet — 19 corrections paid for in production)', join(fleetRoot, 'PRINCIPLES.md')],
    ['CLAUDE.md (workspace)', join(cwd, 'CLAUDE.md')],
    ['AGENTS.md (workspace)', join(cwd, 'AGENTS.md')],
  ]) {
    try {
      const text = await readFile(path, 'utf8');
      if (text.trim()) parts.push({ label, path, text: text.slice(0, 12000) });
    } catch { /* absent is normal */ }
  }
  return parts;
}

const ROLE_BRIEF = {
  build: [
    'You are the BUILD lane. You hold the pen: make the change in the workspace.',
    'Work one slice at a time. Write the failing check first where one is possible.',
    'Do not push, merge, or open a PR — that gate belongs to the human.',
  ].join('\n'),
  verify: [
    'You are the VERIFY lane, running independently and in parallel with a build lane.',
    'You are READ-ONLY. Do not edit files. Reproduce claims from scratch and be adversarial.',
    'Report: what you could confirm with a real command and exit code, what you could NOT, and',
    'the single most likely way this change is wrong. An honest gap is information.',
  ].join('\n'),
  solo: [
    'Answer or act on the request directly. Prefer reading the code over guessing about it.',
    'State what you did NOT do. Never report a proxy (an HTTP 200, an exit 0 after a pipe) as the',
    'property itself.',
  ].join('\n'),
};

/**
 * Assemble the prompt. Ordered cheapest-to-most-specific so a backend that truncates loses the
 * least important context first, and so the actual request is the last thing it reads.
 */
export function buildPrompt({ message, cwd, role = 'solo', rules = [], skills = [], mcp = [], git, history = [] }) {
  const p = [];
  p.push('# fleet session');
  p.push(`workspace: ${cwd}`);
  if (git) p.push(`git: ${git.branch}${git.changed ? ` (${git.changed} uncommitted)` : ' (clean)'}${git.head ? ` · ${git.head}` : ''}`);
  const usable = mcp.filter(s => s.status === 'available');
  if (usable.length) p.push(`mcp available: ${usable.map(s => s.name).join(', ')}`);
  p.push('');
  p.push('## Your lane');
  p.push(ROLE_BRIEF[role] || ROLE_BRIEF.solo);
  p.push('');

  if (skills.length) {
    p.push('## Skills selected for this request');
    p.push('These matched the request and are the house method for it. Apply them.');
    p.push('');
    for (const { skill, hits } of skills) {
      p.push(`### skill: ${skill.name}  (matched: ${hits.join(', ')})`);
      // A skill's value is in its body, not its blurb; cap it so three matches cannot crowd out
      // the request itself.
      p.push(skillBody(skill.raw ?? '').slice(0, 6000) || skill.description);
      p.push('');
    }
  }
  if (rules.length) {
    p.push('## Standing rules for this tree');
    for (const r of rules) { p.push(`### ${r.label}`); p.push(r.text); p.push(''); }
  }
  if (history.length) {
    p.push('## Conversation so far');
    for (const h of history) p.push(`${h.role === 'user' ? 'User' : 'Assistant'}: ${h.text.slice(0, 2000)}`);
    p.push('');
  }
  p.push('## Request');
  p.push(message);
  return p.join('\n');
}

/** Read the bodies of matched skills so buildPrompt can inline them. */
export async function loadMatches(skills, message, limit) {
  const matches = matchSkills(skills, message, limit);
  await Promise.all(matches.map(async m => { try { m.skill.raw = await readFile(m.skill.path, 'utf8'); } catch { m.skill.raw = ''; } }));
  return matches;
}

/**
 * Partition the harnesses three ways. `probing` is NOT `unavailable`: collapsing the two throws
 * away a message typed during startup, which is exactly what happened — a turn was refused with
 * "no ready backend" 8.4 seconds before codex reported ready. A caller that cannot tell "not yet
 * known" from "known unusable" cannot decide whether to wait or to refuse.
 */
export function backendGate(capability) {
  const ready = [], probing = [], blocked = [];
  for (const [id, cap] of Object.entries(capability || {})) {
    const status = cap?.status;
    if (status === 'ready') ready.push(id);
    else if (status === 'probing' || status === undefined) probing.push(id);
    else blocked.push({ id, status, reason: cap?.reason || status });
  }
  return { ready, probing, blocked, decision: ready.length ? 'go' : probing.length ? 'wait' : 'refuse' };
}

/** One backend, one role, streamed. */
export function turn({ backend, model, prompt, cwd, sandbox, timeoutMs, mcpConfig, onEvent, signal }) {
  return run(backend, { prompt, cwd, model, sandbox, timeoutMs, mcpConfig, onEvent, signal });
}

/**
 * Both lanes at once. Returns [{lane, role, result}] once every lane has settled.
 * A lane whose backend is not ready is NOT run and NOT reported as a pass — it settles as
 * `skipped` carrying the probe's reason, so a half fan-out can never read as a whole one.
 */
export async function fanout({ prompts, capability, cwd, sandbox, timeoutMs, mcpConfig, onEvent, signal }) {
  const ready = LANES.filter(l => capability[l.backend]?.status === 'ready');
  // Whoever is ready builds; a second ready lane verifies. With one lane ready the split collapses
  // to solo, stated plainly rather than pretended away.
  const assigned = LANES.map((l, i) => {
    const isReady = capability[l.backend]?.status === 'ready';
    let role;
    if (!isReady) role = null;
    else if (ready.length === 1) role = 'solo';
    else role = ready[0].id === l.id ? 'build' : 'verify';
    return { ...l, role, ready: isReady, reason: capability[l.backend]?.reason };
  });

  const results = await Promise.all(assigned.map(async lane => {
    if (!lane.ready) {
      onEvent({ lane: lane.id, kind: 'skipped', text: lane.reason || 'backend not ready' });
      return { lane, status: 'skipped', reason: lane.reason || 'backend not ready' };
    }
    onEvent({ lane: lane.id, kind: 'start', role: lane.role });
    const r = await turn({
      backend: lane.backend, model: lane.model, prompt: prompts[lane.role] ?? prompts.solo,
      cwd, sandbox: lane.role === 'verify' ? 'read-only' : sandbox, timeoutMs, mcpConfig, signal,
      onEvent: ev => onEvent({ lane: lane.id, ...ev }),
    });
    onEvent({ lane: lane.id, kind: 'done', ok: r.ok, reason: r.reason });
    return { lane, status: r.ok ? 'ok' : 'failed', result: r };
  }));
  return results;
}

export { BACKENDS, LANES };
