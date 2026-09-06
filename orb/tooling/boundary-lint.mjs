#!/usr/bin/env node
// Enforces the two import-boundary rules this repo cannot survive without:
//
// 1. C9 (one model door): nothing outside backend/gateway-sidecar/src may import a provider SDK
//    (@anthropic-ai/*, @google/generative-ai, etc). All model calls go through @pe/llm-gateway,
//    which the sidecar wraps for the Python/Rust backend (docs/adr/0004-backend-language-split.md).
// 2. Presence-plane isolation (blueprint doc 01 §3): presence may import nothing from cognitive or
//    voice. Invariant #1 has zero dependency on the crew or the network.
//
// Both are lint-time, not a style nit: a provider SDK import outside the relay proxy silently
// reintroduces an uncapped/unmetered spend path (breaks C12); a deep cross-plane import silently
// breaks the "presence never blocks on cognitive" guarantee this product is built to hold.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { extname, join, relative, dirname } from 'node:path';

const ROOT = process.cwd();
const PROVIDER_SDK_PATTERN = /@anthropic-ai\/|@google\/generative-ai|anthropic|openai|google_genai|deepgram|sarvam|cartesia|fish[_-]audio/i;
const ALLOWED_PROVIDER_DIR = 'backend/gateway-sidecar/src';
const PRESENCE_DIR = 'apps/mobile/src/presence';
const FORBIDDEN_FROM_PRESENCE = ['apps/mobile/src/cognitive', 'apps/mobile/src/voice'];
const CHECKED_DIRS = [
  'apps/mobile/src',
  'backend/gateway-sidecar/src',
  'backend/relay-py/src',
  'backend/relay-rs/src',
];

let violations = [];

function walk(dir) {
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry.startsWith('.')) continue;
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) walk(full);
    else if (/\.(ts|tsx|py|rs)$/.test(entry)) checkFile(full);
  }
}

function checkFile(file) {
  const rel = relative(ROOT, file);
  const src = readFileSync(file, 'utf8');
  const specs = importSpecs(src, extname(file));

  for (const spec of specs) {
    if (PROVIDER_SDK_PATTERN.test(spec) && !rel.startsWith(ALLOWED_PROVIDER_DIR)) {
      violations.push(`${rel}: imports provider SDK "${spec}" outside ${ALLOWED_PROVIDER_DIR} (C9 — route model calls through @pe/llm-gateway)`);
    }
    if (rel.startsWith(PRESENCE_DIR) && spec.startsWith('.')) {
      const resolved = join(dirname(file), spec);
      const resolvedRel = relative(ROOT, resolved);
      if (FORBIDDEN_FROM_PRESENCE.some((p) => resolvedRel.startsWith(p))) {
        violations.push(`${rel}: presence plane imports "${spec}" — presence must have zero dependency on cognitive/voice (invariant #1)`);
      }
    }
  }
}

function importSpecs(src, ext) {
  if (ext === '.ts' || ext === '.tsx') {
    return [...src.matchAll(/^\s*import[^;]*from\s+['"]([^'"]+)['"]/gm)].map((m) => m[1]);
  }
  if (ext === '.py') {
    return [...src.matchAll(/^\s*(?:from\s+([A-Za-z0-9_.]+)\s+import|import\s+([A-Za-z0-9_.]+))/gm)].map((m) => m[1] ?? m[2]);
  }
  if (ext === '.rs') {
    return [
      ...src.matchAll(/^\s*use\s+([A-Za-z0-9_:]+)(?:[;:{\s])/gm),
      ...src.matchAll(/^\s*extern\s+crate\s+([A-Za-z0-9_]+)\s*;/gm),
    ].map((m) => m[1]);
  }
  return [];
}

for (const dir of CHECKED_DIRS) {
  try {
    walk(join(ROOT, dir));
  } catch {
    // dir not created yet — fine during early scaffolding
  }
}

if (violations.length) {
  console.error('boundary-lint: violations found\n');
  for (const v of violations) console.error(' - ' + v);
  process.exit(1);
}
console.log('boundary-lint: clean');
