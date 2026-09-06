// discover.mjs — what this workspace makes available: skills, slash commands, MCP servers, verbs.
//
// Discovery is filesystem-only and never executes anything it finds. A SKILL.md is read for its
// frontmatter; a command .md for its `description:`; an MCP entry for its command. Nothing here
// spawns a process, so opening the UI in an untrusted directory cannot run that directory's code.
//
// Precedence is nearest-first: the workspace overrides the fleet repo, which overrides $HOME. A
// name collision resolves to the nearest definition and the shadowed one is RECORDED, not dropped,
// so `/skills` can show that a user-level skill was overridden rather than silently vanishing.

import { readFile, readdir, stat } from 'node:fs/promises';
import { join, basename, dirname } from 'node:path';
import { homedir } from 'node:os';

/** Parse `---\nkey: value\n---` frontmatter. Returns {} when absent — never throws. */
export function frontmatter(text) {
  if (!text.startsWith('---')) return {};
  const end = text.indexOf('\n---', 3);
  if (end === -1) return {};
  const out = {};
  for (const line of text.slice(3, end).split('\n')) {
    const i = line.indexOf(':');
    if (i <= 0) continue;
    out[line.slice(0, i).trim()] = line.slice(i + 1).trim();
  }
  return out;
}

/** Body after the frontmatter block, or the whole text when there is none. */
export function body(text) {
  if (!text.startsWith('---')) return text;
  const end = text.indexOf('\n---', 3);
  return end === -1 ? text : text.slice(end + 4).replace(/^\n+/, '');
}

async function listDirs(dir) {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries.filter(e => e.isDirectory() || e.isSymbolicLink()).map(e => join(dir, e.name));
  } catch { return []; }
}

async function listFiles(dir, ext) {
  try {
    const entries = await readdir(dir, { withFileTypes: true });
    return entries.filter(e => e.isFile() && e.name.endsWith(ext)).map(e => join(dir, e.name));
  } catch { return []; }
}

/** Search roots, nearest first. Duplicates (workspace === fleet repo) are collapsed. */
export function searchRoots(cwd, fleetRoot) {
  const roots = [
    { label: 'workspace', dir: cwd },
    { label: 'fleet', dir: fleetRoot },
    { label: 'user', dir: homedir() },
  ];
  const seen = new Set();
  return roots.filter(r => { if (seen.has(r.dir)) return false; seen.add(r.dir); return true; });
}

export async function discoverSkills(cwd, fleetRoot) {
  const found = new Map(); const shadowed = [];
  for (const root of searchRoots(cwd, fleetRoot)) {
    for (const dir of await listDirs(join(root.dir, '.claude', 'skills'))) {
      const path = join(dir, 'SKILL.md');
      let text; try { text = await readFile(path, 'utf8'); } catch { continue; }
      const fm = frontmatter(text);
      // A SKILL.md with no frontmatter name still has an identity: its directory. fleet's own
      // .claude/skills/fleet/SKILL.md is exactly that shape, and dropping it would understate
      // what is available.
      const name = fm.name || basename(dir);
      const skill = {
        name, path, origin: root.label, bytes: text.length,
        description: fm.description || body(text).split('\n').find(l => l.trim()) || '',
      };
      if (found.has(name)) { shadowed.push({ ...skill, shadowedBy: found.get(name).origin }); continue; }
      found.set(name, skill);
    }
  }
  return { skills: [...found.values()].sort((a, b) => a.name.localeCompare(b.name)), shadowed };
}

export async function discoverCommands(cwd, fleetRoot) {
  const found = new Map(); const shadowed = [];
  for (const root of searchRoots(cwd, fleetRoot)) {
    for (const path of await listFiles(join(root.dir, '.claude', 'commands'), '.md')) {
      let text; try { text = await readFile(path, 'utf8'); } catch { continue; }
      const fm = frontmatter(text);
      const name = basename(path, '.md');
      const cmd = { name, path, origin: root.label, description: fm.description || '', prompt: body(text) };
      if (found.has(name)) { shadowed.push({ ...cmd, shadowedBy: found.get(name).origin }); continue; }
      found.set(name, cmd);
    }
  }
  return { commands: [...found.values()].sort((a, b) => a.name.localeCompare(b.name)), shadowed };
}

/**
 * MCP servers declared for this workspace, each labelled `available` or `absent` with the reason.
 *
 * `configured` and `absent` are different claims and the console already keeps them apart
 * (console/server/capability.mjs). A server whose command is not on PATH is reported ABSENT with
 * its install hint — fleet's own mcp.bundle.json states this rule explicitly: "never silently
 * dropped, and never faked as present".
 */
export async function discoverMcp(cwd, fleetRoot, pathEnv = process.env.PATH || '') {
  const servers = new Map();
  const sources = [];
  const add = (name, spec, source) => { if (!servers.has(name)) servers.set(name, { name, spec, source }); };

  for (const root of searchRoots(cwd, fleetRoot)) {
    for (const rel of ['.mcp.json', join('.claude', 'mcp.json')]) {
      const path = join(root.dir, rel);
      try {
        const cfg = JSON.parse(await readFile(path, 'utf8'));
        sources.push(path);
        for (const [name, spec] of Object.entries(cfg.mcpServers || cfg.servers || {})) add(name, spec, path);
      } catch { /* absent or unparseable: not an error, just nothing declared here */ }
    }
  }
  // ~/.claude.json holds the user's global servers under `mcpServers`.
  try {
    const path = join(homedir(), '.claude.json');
    const cfg = JSON.parse(await readFile(path, 'utf8'));
    if (cfg.mcpServers && Object.keys(cfg.mcpServers).length) {
      sources.push(path);
      for (const [name, spec] of Object.entries(cfg.mcpServers)) add(name, spec, path);
    }
  } catch { /* same */ }

  const dirs = pathEnv.split(':').filter(Boolean);
  const resolved = [];
  for (const s of servers.values()) {
    const cmd = s.spec?.command;
    const transport = s.spec?.type || (s.spec?.url ? 'http' : 'stdio');
    if (!cmd) {
      // A URL transport has nothing local to probe; saying "available" would be a claim this
      // function cannot support, so it is reported as declared-only.
      resolved.push({ ...s, transport, status: s.spec?.url ? 'declared' : 'invalid',
        detail: s.spec?.url || 'no command and no url' });
      continue;
    }
    let where = null;
    if (cmd.includes('/')) { try { await stat(cmd); where = cmd; } catch { /* miss */ } }
    else for (const d of dirs) { try { await stat(join(d, cmd)); where = join(d, cmd); break; } catch { /* miss */ } }
    resolved.push({ ...s, transport, status: where ? 'available' : 'absent', detail: where || `${cmd} not on PATH` });
  }
  return { servers: resolved.sort((a, b) => a.name.localeCompare(b.name)), sources };
}

/** The `fleet <verb>` table, parsed out of the entrypoint's own help so it can never drift. */
export async function discoverVerbs(fleetRoot) {
  let text; try { text = await readFile(join(fleetRoot, 'fleet'), 'utf8'); } catch { return []; }
  const out = [];
  // Lines of the form:  '  verb                     description' \
  // `\s+` and not `\s{2,}`: one verb's help line is padded with a SINGLE space, and requiring two
  // silently dropped it from the command popup — a verb the operator could run but never see.
  for (const m of text.matchAll(/^\s*'\s{2}([a-z][a-z0-9-]*)\s+(.+?)'\s*\\?$/gm)) {
    out.push({ name: m[1], description: m[2].trim() });
  }
  return out.sort((a, b) => a.name.localeCompare(b.name));
}

/**
 * Score a skill against a message. Deliberately crude and deliberately transparent: the UI shows
 * which skills matched and why, so a wrong match is visible rather than silently steering a run.
 * Matching is over the description's quoted trigger phrases first, then bare word overlap.
 */
export function matchSkills(skills, message, limit = 3) {
  const words = new Set(message.toLowerCase().match(/[a-z][a-z0-9+-]{2,}/g) || []);
  if (!words.size) return [];
  const scored = [];
  for (const s of skills) {
    const hay = `${s.name} ${s.description}`.toLowerCase();
    let score = 0; const hits = [];
    // A quoted trigger phrase appearing verbatim in the message is the strongest signal.
    for (const [, phrase] of hay.matchAll(/"([^"]{3,60})"/g)) {
      if (message.toLowerCase().includes(phrase)) { score += 5; hits.push(phrase); }
    }
    for (const w of words) {
      if (w.length < 4) continue;
      if (hay.includes(w)) { score += 1; hits.push(w); }
    }
    if (s.name.split(/[-_]/).some(part => part.length > 3 && words.has(part))) score += 3;
    if (score > 0) scored.push({ skill: s, score, hits: [...new Set(hits)].slice(0, 4) });
  }
  return scored.sort((a, b) => b.score - a.score).slice(0, limit).filter(x => x.score >= 2);
}

export async function discoverAll(cwd, fleetRoot) {
  const [sk, cm, mcp, verbs] = await Promise.all([
    discoverSkills(cwd, fleetRoot), discoverCommands(cwd, fleetRoot),
    discoverMcp(cwd, fleetRoot), discoverVerbs(fleetRoot),
  ]);
  return { ...sk, ...cm, mcp: mcp.servers, mcpSources: mcp.sources, verbs,
           shadowed: [...sk.shadowed, ...cm.shadowed] };
}
