// Corpus row B9. The original defect: the console header read "121 shown of 120 nodes" -- a count
// that exceeded its own total, because a synthetic failure-engine node was rendered but never added
// to graph.nodes. Two numbers from different sets, divided.
//
// The previous detector was `const shown = state.nodes.length; const total = state.nodes.length;`
// then asserted shown <= total. That is a tautology: it compares a value to itself and can never
// fail. It also ran via `node --input-type=module` on stdin, unsupported here, so it errored on every
// invocation and the row read MISSED for a reason unrelated to what it claims to check.
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
const root = process.env.FLEET_ROOT ?? process.cwd();
const { collectAll } = await import(pathToFileURL(resolve(root, 'console/server/collect.mjs')).href);
const { buildGraph } = await import(pathToFileURL(resolve(root, 'console/server/graph.mjs')).href);

const state = buildGraph(await collectAll(process.cwd()));
const problems = [];

// 1. Every edge endpoint must exist as a node. An edge to a missing node is the shape that produced
//    the phantom count -- something referenced but not enumerated.
const ids = new Set(state.nodes.map(n => n.id));
const dangling = state.edges.filter(e => !ids.has(e.source) || !ids.has(e.target));
if (dangling.length) {
  problems.push(`${dangling.length} edge(s) reference a node not in graph.nodes, e.g. ${dangling[0].id}`);
}

// 2. Node ids must be unique: a duplicate inflates any length-based count.
const dupes = state.nodes.length - ids.size;
if (dupes > 0) problems.push(`${dupes} duplicate node id(s)`);

// 3. Every ticket must correspond to a real node, so the board and the map cannot disagree.
const orphanTickets = state.tickets.filter(t => !ids.has(`agent:${t.id}`) && !ids.has(`brief:${t.id}`));
if (orphanTickets.length) {
  problems.push(`${orphanTickets.length} ticket(s) with no node, e.g. ${orphanTickets[0].id}`);
}

// 4. A count the UI shows must never exceed the set it counts.
for (const [k, v] of Object.entries(state.counts ?? {})) {
  if (Number.isInteger(v) && v > state.tickets.length) {
    problems.push(`counts.${k}=${v} exceeds tickets=${state.tickets.length}`);
  }
}

if (problems.length) {
  for (const p of problems) console.log(p);
  process.exit(1);
}
console.log(`ok: ${state.nodes.length} nodes, ${state.edges.length} edges, ${state.tickets.length} tickets, no dangling refs`);
