#!/usr/bin/env node
// orb-shape-gate.test.mjs
//
// Unit tests for the pure math in orb-shape-gate.mjs (CRC32, PNG codec round-trip, Otsu
// threshold, connected components, ray casting, stats), PLUS the calibration run that the gate
// is not allowed to ship without: a KNOWN-GOOD true circle and TWO known-bad regular polygons
// (12-gon, 24-gon) are rendered to real PNG files and fed through the ACTUAL CLI
// (`node scripts/orb-shape-gate.mjs <file>`, as a subprocess — not just calling internal
// functions) so this proves the shipped script, not a friendlier internal API.
//
// This is a plain Node script (`node scripts/orb-shape-gate.test.mjs`), not a vitest suite:
// vitest.config.ts's `test.include` is scoped to apps/mobile and the two backend sidecars, so a
// scripts/**/*.test.ts(x) file would silently never run under `npm run test`. A standalone
// script that fails loudly (non-zero exit, printed failures) matches how every other gate in
// scripts/ is run (see ios-launchable-check.mjs, native-preflight.mjs) and does not require
// touching vitest.config.ts, which is out of this change's scope.
//
// Usage: node scripts/orb-shape-gate.test.mjs

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { deflateSync } from 'node:zlib';
import { mkdtempSync, writeFileSync, rmSync, existsSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  crc32,
  decodePNG,
  toLuminance,
  otsuThreshold,
  largestComponent,
  castRays,
  radiusStats,
  asciiSparkline,
  DEFAULT_THRESHOLD_PCT,
} from './orb-shape-gate.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const GATE_PATH = join(__dirname, 'orb-shape-gate.mjs');

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

// -------------------------------------------------------------------------------------------
// Minimal PNG encoder + shape rasterizers — TEST-ONLY infrastructure for building known-shape
// fixtures. The gate itself never needs to write PNGs, only read them; this stays out of
// orb-shape-gate.mjs on purpose (it is not part of the shipped tool's runtime job).
// -------------------------------------------------------------------------------------------

function pngChunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, 'ascii');
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crcBuf]);
}

/** Encodes an RGBA8 buffer (no filtering — filter type None per row) as a valid PNG. Used only
 * to build test fixtures; deliberately simple (no compression-ratio concerns). */
function encodePNG(width, height, rgba) {
  const rowBytes = width * 4;
  const raw = Buffer.alloc((rowBytes + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (rowBytes + 1)] = 0; // filter type: None
    Buffer.from(rgba.buffer, rgba.byteOffset + y * rowBytes, rowBytes).copy(raw, y * (rowBytes + 1) + 1);
  }
  const compressed = deflateSync(raw, { level: 9 });
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type: truecolor + alpha
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0; // compression/filter/interlace methods
  const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  return Buffer.concat([sig, pngChunk('IHDR', ihdr), pngChunk('IDAT', compressed), pngChunk('IEND', Buffer.alloc(0))]);
}

function clamp01(v) {
  return Math.max(0, Math.min(1, v));
}

function fillCanvas(width, height, bg) {
  const rgba = new Uint8Array(width * height * 4);
  for (let i = 0; i < width * height; i++) {
    rgba[i * 4] = bg[0];
    rgba[i * 4 + 1] = bg[1];
    rgba[i * 4 + 2] = bg[2];
    rgba[i * 4 + 3] = 255;
  }
  return rgba;
}

function blend(rgba, width, x, y, fg, t) {
  const idx = (y * width + x) * 4;
  rgba[idx] = Math.round(rgba[idx] * (1 - t) + fg[0] * t);
  rgba[idx + 1] = Math.round(rgba[idx + 1] * (1 - t) + fg[1] * t);
  rgba[idx + 2] = Math.round(rgba[idx + 2] * (1 - t) + fg[2] * t);
}

/** Exact polar radius of a regular n-gon with the given apothem (inradius), at absolute angle
 * theta. phi is the angle measured from the nearest edge-midpoint direction, in [-pi/n, pi/n];
 * r(theta) = apothem / cos(phi) is the standard closed form for a regular polygon's boundary. */
function polygonRadiusAt(theta, apothem, n) {
  const step = (2 * Math.PI) / n;
  const k = Math.round(theta / step);
  const phi = theta - k * step;
  return apothem / Math.cos(phi);
}

/** Rasterizes a disc (n=0) or a regular n-gon (apothem = inradius) onto a near-black canvas,
 * with ~1px of edge antialiasing so these fixtures aren't unrealistically harder-edged than a
 * real anti-aliased render (the gate's Otsu threshold + bilinear ray sampling has to cope with
 * an AA band either way). */
function renderShape({ width, height, cx, cy, radius, n = 0, bg = [8, 9, 13], fg = [198, 188, 172] }) {
  const rgba = fillCanvas(width, height, bg);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      const d = Math.hypot(dx, dy);
      const rAtAngle = n >= 3 ? polygonRadiusAt(Math.atan2(dy, dx), radius, n) : radius;
      const t = clamp01(rAtAngle + 0.5 - d);
      if (t > 0) blend(rgba, width, x, y, fg, t);
    }
  }
  return rgba;
}

function runCLI(pngPath, extraArgs = []) {
  try {
    const stdout = execFileSync(process.execPath, [GATE_PATH, pngPath, '--json', ...extraArgs], { encoding: 'utf8' });
    return { status: 0, stdout, stderr: '' };
  } catch (err) {
    return { status: err.status, stdout: err.stdout || '', stderr: err.stderr || '' };
  }
}

function lastJsonLine(stdout) {
  const lines = stdout.trim().split('\n');
  return JSON.parse(lines[lines.length - 1]);
}

// -------------------------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------------------------

console.log('orb-shape-gate: unit tests');

test('crc32 matches the standard check value for "123456789"', () => {
  assert.equal(crc32(Buffer.from('123456789')), 0xcbf43926);
});

test('PNG encode -> decode round-trips arbitrary RGBA pixels exactly', () => {
  const width = 37;
  const height = 29; // deliberately odd/non-power-of-2 to catch stride bugs
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const idx = (y * width + x) * 4;
      rgba[idx] = (x * 7 + y * 3) % 256;
      rgba[idx + 1] = (x * 13 + y) % 256;
      rgba[idx + 2] = (x + y * 17) % 256;
      rgba[idx + 3] = 255;
    }
  }
  const png = encodePNG(width, height, rgba);
  const decoded = decodePNG(png);
  assert.equal(decoded.width, width);
  assert.equal(decoded.height, height);
  assert.deepEqual(Array.from(decoded.rgba), Array.from(rgba));
});

test('decodePNG rejects a non-PNG buffer with a clear error, not a crash', () => {
  assert.throws(() => decodePNG(Buffer.from('not a png')), /bad 8-byte signature/);
});

test('otsuThreshold separates a clean two-cluster histogram correctly', () => {
  const n = 10000;
  const lum = new Uint8Array(n);
  for (let i = 0; i < n; i++) lum[i] = i < n * 0.7 ? 10 : 200; // 70% background, 30% bright
  const t = otsuThreshold(lum);
  // The real invariant: classifying with this threshold puts every background pixel on one side
  // and every foreground pixel on the other. The exact numeric threshold within the empty gap
  // [10,200) is an implementation tie-break detail, not the thing being tested.
  assert.ok(10 <= t && t < 200, `threshold ${t} should fall in the empty gap between the two clusters`);
});

test('largestComponent finds a centered blob and ignores a small far corner speck', () => {
  const width = 21;
  const height = 21;
  const lum = new Uint8Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (Math.hypot(x - 10, y - 10) <= 6) lum[y * width + x] = 200;
    }
  }
  // A 2x2 speck far from the disc.
  lum[0] = 200;
  lum[1] = 200;
  lum[width] = 200;
  lum[width + 1] = 200;

  const comp = largestComponent(lum, width, height, 100);
  assert.ok(comp.count > 50, `expected the disc (~113px) to win, got largest=${comp.count}px`);
  assert.ok(Math.abs(comp.cx - 10) < 0.6, `centroid.x=${comp.cx} should be ~10`);
  assert.ok(Math.abs(comp.cy - 10) < 0.6, `centroid.y=${comp.cy} should be ~10`);
});

test('castRays + radiusStats measure near-zero deviation for a true circle mask', () => {
  const width = 101;
  const height = 101;
  const lum = new Uint8Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (Math.hypot(x - 50, y - 50) <= 40) lum[y * width + x] = 200;
    }
  }
  const radii = castRays({ luminance: lum, width, height, threshold: 100, cx: 50, cy: 50, rayCount: 360, maxRadius: 55 });
  const stats = radiusStats(radii);
  assert.equal(stats.validCount, 360, 'every ray should find a crossing on a solid disc');
  assert.ok(stats.maxDeviationPct < 4, `hard-edged 40px-radius disc on an integer grid: expected <4% quantization noise, got ${stats.maxDeviationPct}%`);
});

test('castRays measures a clearly larger deviation for a hexagon than for a circle at the same scale', () => {
  const width = 101;
  const height = 101;
  const lum = new Uint8Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const dx = x - 50;
      const dy = y - 50;
      const rAtAngle = polygonRadiusAt(Math.atan2(dy, dx), 35, 6);
      if (Math.hypot(dx, dy) <= rAtAngle) lum[y * width + x] = 200;
    }
  }
  const radii = castRays({ luminance: lum, width, height, threshold: 100, cx: 50, cy: 50, rayCount: 360, maxRadius: 55 });
  const stats = radiusStats(radii);
  assert.ok(stats.maxDeviationPct > 8, `a hexagon (apothem 35) should show a large deviation, got ${stats.maxDeviationPct}%`);
});

test('asciiSparkline never renders the deepest facet dip as a blank space', () => {
  // Regression guard: found via real evidence (AFTER_auroramist_fixed_3.png) where the sharpest,
  // most diagnostically important dip in the profile printed as invisible blank space because
  // the lowest bucket mapped to a ' ' character instead of a visible bar.
  const radii = [10, 10, 10, 1, 10, 10, 10]; // one sharp low outlier among flat highs
  const line = asciiSparkline(radii, 7);
  assert.ok(!line.includes(' '), `sparkline should have no blank/space characters, got "${line}"`);
});

test('polygonRadiusAt matches the closed-form apothem/circumradius for n=12 and n=24', () => {
  for (const n of [12, 24]) {
    const apothem = 100;
    let min = Infinity;
    let max = -Infinity;
    for (let i = 0; i < 100000; i++) {
      const theta = (i / 100000) * 2 * Math.PI;
      const r = polygonRadiusAt(theta, apothem, n);
      if (r < min) min = r;
      if (r > max) max = r;
    }
    const expectedMax = apothem / Math.cos(Math.PI / n);
    assert.ok(Math.abs(min - apothem) < 0.01, `n=${n}: min radius ${min} should equal apothem ${apothem}`);
    assert.ok(Math.abs(max - expectedMax) < 0.01, `n=${n}: max radius ${max} should equal circumradius ${expectedMax}`);
  }
});

// -------------------------------------------------------------------------------------------
// Calibration: true circle (known-good) vs 12-gon and 24-gon (known-bad), through the REAL CLI.
// -------------------------------------------------------------------------------------------

console.log('\norb-shape-gate: calibration (real CLI, generated PNG fixtures)');

const tmpDir = mkdtempSync(join(tmpdir(), 'orb-shape-gate-calibration-'));
const SIZE = 640;
const CENTER = SIZE / 2;
const RADIUS = 240; // also used as the apothem (inradius) for both polygons, so all three
// shapes share the same "how far in the flat parts of the boundary sits" reference point.

const shapes = [
  { name: 'circle', n: 0 },
  { name: '12gon', n: 12 },
  { name: '24gon', n: 24 },
];

const calibrationResults = {};
for (const { name, n } of shapes) {
  const rgba = renderShape({ width: SIZE, height: SIZE, cx: CENTER, cy: CENTER, radius: RADIUS, n });
  const png = encodePNG(SIZE, SIZE, rgba);
  const path = join(tmpDir, `${name}.png`);
  writeFileSync(path, png);
  const result = runCLI(path);
  calibrationResults[name] = result;
  const parsed = result.stdout.includes('{') ? lastJsonLine(result.stdout) : null;
  const devPct = parsed?.stats?.maxDeviationPct;
  console.log(
    `  ${name}: exit=${result.status}  maxDeviationPct=${devPct !== undefined ? devPct.toFixed(3) + '%' : 'N/A'}` +
      (parsed ? `  mean_r=${parsed.stats.mean.toFixed(2)}` : ''),
  );
}

test('known-good true circle PASSES (exit 0)', () => {
  assert.equal(calibrationResults.circle.status, 0, `circle should pass:\n${calibrationResults.circle.stdout}\n${calibrationResults.circle.stderr}`);
});

test('known-bad 12-gon FAILS (exit 6)', () => {
  assert.equal(calibrationResults['12gon'].status, 6, `12-gon should fail:\n${calibrationResults['12gon'].stdout}\n${calibrationResults['12gon'].stderr}`);
});

test('known-bad 24-gon FAILS (exit 6)', () => {
  assert.equal(calibrationResults['24gon'].status, 6, `24-gon should fail:\n${calibrationResults['24gon'].stdout}\n${calibrationResults['24gon'].stderr}`);
});

test('circle measures comfortably below the default threshold, with real margin', () => {
  const dev = lastJsonLine(calibrationResults.circle.stdout).stats.maxDeviationPct;
  assert.ok(
    dev < DEFAULT_THRESHOLD_PCT / 2,
    `circle deviation ${dev}% should be well under half the ${DEFAULT_THRESHOLD_PCT}% threshold, or the margin isn't real`,
  );
});

test('bad-file inputs FAIL loudly (exit 6), never a silent pass', () => {
  const missing = runCLI(join(tmpDir, 'does-not-exist.png'));
  assert.equal(missing.status, 6);
  const garbage = join(tmpDir, 'garbage.png');
  writeFileSync(garbage, Buffer.from('this is not a png'));
  const bad = runCLI(garbage);
  assert.equal(bad.status, 6);
});

test('an all-background image (no orb) FAILS loudly rather than passing on nothing', () => {
  const rgba = fillCanvas(200, 200, [8, 9, 13]);
  const path = join(tmpDir, 'empty.png');
  writeFileSync(path, encodePNG(200, 200, rgba));
  const result = runCLI(path);
  assert.equal(result.status, 6);
});

rmSync(tmpDir, { recursive: true, force: true });

// -------------------------------------------------------------------------------------------
// Informational: run against whatever real orb evidence already exists in the repo. NOT
// asserted in this suite — these are session-specific evidence captures (a timestamped manual
// UI-drive dir, an Android e2e artifact, and whatever a parallel agent may be writing to
// apps/mobile/evidence/ right now) that this test does not own and cannot guarantee will keep
// existing at these paths. A hard assertion here would make this suite fail for reasons that
// have nothing to do with orb-shape-gate.mjs's own correctness. Read the printed numbers.
// -------------------------------------------------------------------------------------------
console.log('\norb-shape-gate: real evidence found in the repo (informational only, not asserted)');

const repoRoot = join(__dirname, '..');
const candidateRealEvidence = [
  join(repoRoot, 'evidence/ui-drive-070313/01-launch.png'),
  join(repoRoot, 'docs/evidence/android-e2e/focus-orb-android-orb-only.png'),
];
const mobileEvidenceDir = join(repoRoot, 'apps/mobile/evidence');
if (existsSync(mobileEvidenceDir)) {
  for (const f of readdirSync(mobileEvidenceDir)) {
    if (f.toLowerCase().endsWith('.png')) candidateRealEvidence.push(join(mobileEvidenceDir, f));
  }
}

for (const path of candidateRealEvidence) {
  if (!existsSync(path)) {
    console.log(`  (skip) ${path} — not present`);
    continue;
  }
  const result = runCLI(path);
  const parsed = result.stdout.includes('{') ? lastJsonLine(result.stdout) : null;
  const devPct = parsed?.stats?.maxDeviationPct;
  const verdict = result.status === 0 ? 'ROUND' : 'NOT ROUND / unmeasurable';
  console.log(
    `  ${path}\n    exit=${result.status} (${verdict})  maxDeviationPct=${devPct !== undefined ? devPct.toFixed(2) + '%' : 'N/A'}`,
  );
}

console.log(`\n${passCount} passed, ${failCount} failed`);
process.exit(failCount > 0 ? 1 : 0);
