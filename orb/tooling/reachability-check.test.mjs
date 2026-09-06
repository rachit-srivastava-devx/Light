import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const CHECKER = join(HERE, 'reachability-check.mjs');
const PYTHON_CHECKER = join(HERE, 'reachability-python.py');

function write(root, path, content) {
  const target = join(root, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function run(root, config) {
  const configPath = join(root, 'reachability.config.json');
  writeFileSync(configPath, JSON.stringify(config, null, 2));
  const result = spawnSync(process.execPath, [CHECKER, '--root', root, '--config', configPath, '--json', '--skip-clippy'], {
    encoding: 'utf8',
    maxBuffer: 20 * 1024 * 1024,
  });
  assert.notEqual(result.status, 2, result.stderr);
  return { result, report: JSON.parse(result.stdout) };
}

function baseConfig(root) {
  return {
    version: 1,
    typescript: { tsconfigs: ['tsconfig.json'], returnValueDeclarationPatterns: [] },
    python: {
      binary: 'python3',
      script: PYTHON_CHECKER,
      sourceRoots: ['py_src'],
      testRoots: ['py_tests'],
    },
    rust: { cargoManifest: 'rust/Cargo.toml', runClippy: false },
    suppressionRoots: ['src', 'rust'],
    composition: {
      roots: ['src/main.ts'],
      thresholdParameterPatterns: ['^(?:pause|duration)[A-Za-z]*Ms$'],
    },
    wireContracts: [{
      name: 'fixture-wire',
      clientFile: 'src/main.ts',
      inputInterface: 'ConversationInput',
      resultInterface: 'ConversationResult',
      factoryFunction: 'createPort',
      responseVariable: 'body',
      pythonFile: 'py_src/app.py',
      requestClass: 'ConversationRequest',
      responseClass: 'ConversationResponse',
    }],
    allowlist: [],
  };
}

test('catches all five historical built-but-unreachable signatures', () => {
  const root = mkdtempSync(join(tmpdir(), 'orb-reachability-five-'));
  try {
    write(root, 'tsconfig.json', JSON.stringify({
      compilerOptions: { target: 'ES2022', module: 'ESNext', moduleResolution: 'bundler', strict: true, noEmit: true },
      include: ['src/**/*.ts'],
    }));
    write(root, 'src/guard.ts', 'export function guarded(): string { return "safe"; }\n');
    write(root, 'src/guard.test.ts', 'import { guarded } from "./guard"; guarded();\n');
    write(root, 'src/main.ts', `
interface ConversationInput {
  readonly session_id: string;
  readonly text: string;
  readonly mode?: 'focus' | 'converse' | 'teach';
}
interface ConversationResult { readonly text: string; }
function createPort(fetchImpl: typeof fetch) {
  return async function respond(input: ConversationInput): Promise<ConversationResult> {
    const response = await fetchImpl('/v1/respond', {
      method: 'POST',
      body: JSON.stringify({ session_id: input.session_id, text: input.text }),
    });
    const body = await response.json() as Record<string, unknown>;
    return { text: String(body.text) };
  };
}
interface VoiceOutput { readonly frames: readonly string[]; }
interface VoicePort { handle(): Promise<VoiceOutput>; }
function compose(voice: VoicePort) {
  const turn = async () => voice.handle();
  return async () => { await turn(); await turn(); };
}
function decideEndpoint(pauseMs: number): boolean { return pauseMs > 100; }
export function main(): boolean { return decideEndpoint(0); }
void createPort;
void compose;
`);
    write(root, 'py_src/app.py', `
class ConversationRequest:
    session_id: str
    text: str
    mode: str | None = None

class ConversationResponse:
    text: str
`);
    write(root, 'py_src/conversation_guard.py', 'async def complete_guarded_conversation() -> str:\n    return "safe"\n');
    write(root, 'py_tests/test_conversation_guard.py', 'from conversation_guard import complete_guarded_conversation\n\ndef test_guard():\n    assert complete_guarded_conversation\n');
    write(root, 'rust/src/lib.rs', '#[allow(dead_code)]\npub const THINK_TIME_COVER_BUDGET_MS: u64 = 250;\n');

    const { result, report } = run(root, baseConfig(root));
    assert.equal(result.status, 1);
    const ids = new Set(report.findings.map((finding) => finding.id));
    assert(ids.has('orphan:src/guard.ts:guarded'), 'TS test-only export was not caught');
    assert(ids.has('orphan:py_src/conversation_guard.py:complete_guarded_conversation'), 'Python test-only export was not caught');
    assert(ids.has('wire:fixture-wire:request:client_not_serialized:mode'), 'produced-but-untransmitted mode was not caught');
    assert(ids.has('return:src/main.ts:turn'), 'discarded Promise result was not caught');
    assert(ids.has('literal:src/main.ts:main:decideEndpoint:pauseMs:0'), 'composition-root disabling literal was not caught');
    assert([...ids].some((id) => id.includes('rust/src/lib.rs') && id.includes('allow-dead-code')), 'unreasoned Rust dead-code suppression was not caught');
    assert(report.denominators.symbolsScanned > 0);
    assert(report.denominators.importersResolved > 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('fails closed when callable-symbol denominator is zero', () => {
  const root = mkdtempSync(join(tmpdir(), 'orb-reachability-zero-'));
  try {
    write(root, 'tsconfig.json', JSON.stringify({
      compilerOptions: { target: 'ES2022', module: 'ESNext', noEmit: true },
      include: ['src/**/*.ts'],
    }));
    write(root, 'src/main.ts', 'export interface ContractOnly { readonly value: string; }\n');
    mkdirSync(join(root, 'py_src'));
    mkdirSync(join(root, 'py_tests'));
    const config = baseConfig(root);
    config.composition.roots = [];
    config.wireContracts = [];
    config.suppressionRoots = ['src'];
    const { result, report } = run(root, config);
    assert.equal(result.status, 1);
    assert(report.findings.some((finding) => finding.code === 'zero-denominator'));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('accepts an inline suppression justification and publishes the allowlist denominator', () => {
  const root = mkdtempSync(join(tmpdir(), 'orb-reachability-reason-'));
  try {
    write(root, 'tsconfig.json', JSON.stringify({
      compilerOptions: { target: 'ES2022', module: 'ESNext', noEmit: true },
      include: ['src/**/*.ts'],
    }));
    write(root, 'src/main.ts', 'export function live(): number { return 1; }\nconsole.log(live());\n');
    write(root, 'rust/src/lib.rs', '#[allow(dead_code)] // reachability: exported for an external crate consumer\npub const PUBLIC_BUDGET_MS: u64 = 250;\n');
    mkdirSync(join(root, 'py_src'));
    mkdirSync(join(root, 'py_tests'));
    const config = baseConfig(root);
    config.composition.roots = [];
    config.wireContracts = [];
    config.allowlist = [];
    const { report } = run(root, config);
    assert(!report.findings.some((finding) => finding.code === 'suppression-missing-reason'));
    assert.equal(report.denominators.allowlistSize, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
