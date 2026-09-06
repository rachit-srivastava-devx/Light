import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import {
  decodePNG, toLuminance, otsuThreshold, largestComponent, castRays, radiusStats, asciiSparkline,
} from '/Users/rachitsrivastava/youtube/Principal Engineering/company/products/adhd-focus-orb/scripts/orb-shape-gate.mjs';

const DEFAULT_RAYS = 360;
const THRESHOLD_PCT = 0.25;

const KNOWN_STATE_COLORS = {
  booting: '#6576e8',
  listening: '#448bd6',
  thinking: '#5c50e6',
  working: '#278f89',
  success: '#58b89f',
  paused: '#53627f',
  error: '#c87552',
};

function hexToRgb(hex) {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function nearestState(rgb) {
  let best = null;
  let bestDist = Infinity;
  for (const [state, hex] of Object.entries(KNOWN_STATE_COLORS)) {
    const c = hexToRgb(hex);
    const d = Math.hypot(rgb[0] - c[0], rgb[1] - c[1], rgb[2] - c[2]);
    if (d < bestDist) {
      bestDist = d;
      best = state;
    }
  }
  return { state: best, dist: bestDist };
}

function findDips(radii, minDepthPct = 1.0) {
  const n = radii.length;
  const present = radii.filter((v) => v !== null);
  if (present.length === 0) return [];
  const mean = present.reduce((a, b) => a + b, 0) / present.length;
  const dips = [];
  for (let i = 0; i < n; i++) {
    const v = radii[i];
    if (v === null) continue;
    const prev = radii[(i - 1 + n) % n];
    const next = radii[(i + 1) % n];
    if (prev === null || next === null) continue;
    if (v <= prev && v <= next && ((mean - v) / mean) * 100 >= minDepthPct) {
      dips.push({ angleDeg: (i / n) * 360, radius: v, depthPct: ((mean - v) / mean) * 100 });
    }
  }
  dips.sort((a, b) => a.angleDeg - b.angleDeg);
  const merged = [];
  for (const d of dips) {
    const last = merged[merged.length - 1];
    if (last) {
      const gap = Math.min(Math.abs(d.angleDeg - last.angleDeg), 360 - Math.abs(d.angleDeg - last.angleDeg));
      if (gap < 8) {
        if (d.depthPct > last.depthPct) merged[merged.length - 1] = d;
        continue;
      }
    }
    merged.push(d);
  }
  // also check wraparound merge (first vs last near 0/360)
  if (merged.length > 1) {
    const first = merged[0];
    const last = merged[merged.length - 1];
    const gap = Math.min(Math.abs(first.angleDeg - last.angleDeg), 360 - Math.abs(first.angleDeg - last.angleDeg));
    if (gap < 8) {
      if (last.depthPct > first.depthPct) merged.shift();
      else merged.pop();
    }
  }
  return merged;
}

const dir = process.argv[2];
if (!dir) {
  console.error('usage: node analyze_burst.mjs <dir-of-pngs>');
  process.exit(1);
}
const files = readdirSync(dir).filter((f) => f.endsWith('.png')).sort();

const rows = [];
for (const f of files) {
  const path = join(dir, f);
  const buf = readFileSync(path);
  let decoded;
  try {
    decoded = decodePNG(buf);
  } catch (err) {
    rows.push({ file: f, error: `decode failed: ${err.message}` });
    continue;
  }
  const { width, height, rgba } = decoded;
  const luminance = toLuminance(rgba, width, height);
  const otsu = otsuThreshold(luminance);
  const comp = largestComponent(luminance, width, height, otsu);
  if (!comp) {
    rows.push({ file: f, error: 'no component found' });
    continue;
  }
  const corners = [
    [comp.minX, comp.minY],
    [comp.minX, comp.maxY],
    [comp.maxX, comp.minY],
    [comp.maxX, comp.maxY],
  ];
  let maxCornerDist = 0;
  for (const [px, py] of corners) {
    const d = Math.hypot(px - comp.cx, py - comp.cy);
    if (d > maxCornerDist) maxCornerDist = d;
  }
  const maxRadius = Math.min(Math.min(width, height) / 2, maxCornerDist * 1.5);
  const radii = castRays({ luminance, width, height, threshold: otsu, cx: comp.cx, cy: comp.cy, rayCount: DEFAULT_RAYS, maxRadius });
  const stats = radiusStats(radii);
  const misses = radii.length - (stats ? stats.validCount : 0);

  let rSum = 0, gSum = 0, bSum = 0, count = 0;
  const sampleR = stats ? stats.mean * 0.4 : 20;
  for (let k = 0; k < 8; k++) {
    const ang = (k / 8) * 2 * Math.PI;
    const sx = Math.round(comp.cx + Math.cos(ang) * sampleR);
    const sy = Math.round(comp.cy + Math.sin(ang) * sampleR);
    if (sx >= 0 && sx < width && sy >= 0 && sy < height) {
      const idx = (sy * width + sx) * 4;
      rSum += rgba[idx];
      gSum += rgba[idx + 1];
      bSum += rgba[idx + 2];
      count++;
    }
  }
  const avgRgb = count ? [rSum / count, gSum / count, bSum / count] : [0, 0, 0];
  const nearest = nearestState(avgRgb);
  const dipList = stats && misses === 0 ? findDips(radii) : [];

  rows.push({
    file: f,
    exit: stats && misses === 0 && stats.maxDeviationPct <= THRESHOLD_PCT ? 0 : 6,
    deviationPct: stats ? stats.maxDeviationPct : null,
    misses,
    hex: '#' + avgRgb.map((v) => Math.round(v).toString(16).padStart(2, '0')).join(''),
    nearestState: nearest.state,
    nearestDist: Math.round(nearest.dist),
    dipCount: dipList.length,
    dipAngles: dipList.map((d) => d.angleDeg.toFixed(0)).join(','),
    sparkline: stats ? asciiSparkline(radii) : '(no data)',
  });
}

console.log('file                exit  deviation%   rgb       nearest-state(dist)  dips  dip-angles');
for (const r of rows) {
  if (r.error) {
    console.log(`${r.file.padEnd(18)}  ERROR ${r.error}`);
    continue;
  }
  console.log(
    `${r.file.padEnd(18)}  ${String(r.exit).padEnd(4)}  ${r.deviationPct?.toFixed(2).padStart(8)}%   ${r.hex}  ${r.nearestState.padEnd(10)}(${r.nearestDist})       ${r.dipCount}     [${r.dipAngles}]`,
  );
}

console.log('\n--- sparklines (radius vs angle, 0deg=east, CCW) ---');
for (const r of rows) {
  if (r.error) continue;
  console.log(`${r.file}  dev=${r.deviationPct?.toFixed(2)}%  state~${r.nearestState}`);
  console.log('  ' + r.sparkline);
}
