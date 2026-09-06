#!/usr/bin/env node
// orb-shape-gate.mjs
//
// FAILS when the ADHD Focus Orb is not round, measured from ACTUAL RENDERED PIXELS of a
// screenshot — not from component code, props, or unit tests. This gate exists because the
// orb can render as a faceted polygon while the entire unit-test suite stays green: nothing in
// a JS/TS test tree observes what a GPU actually rasterized. A screenshot is the only artifact
// that captures that, so this script only ever looks at pixels.
//
// USAGE
//   node scripts/orb-shape-gate.mjs <screenshot.png> [options]
//
// OPTIONS
//   --rays <N>          Number of angular samples cast from the centroid. Default 360.
//                        Hard floor of 180 is enforced regardless of what is passed (a lower
//                        count can miss facets on a 12-sided polygon, which the whole point of
//                        this gate is to catch) — a value below 180 is clamped up, not honored.
//   --threshold <pct>   Max allowed deviation (max |radius-mean| / mean, as a percentage)
//                        before the gate fails. Default 0.25 — see README section "orb shape
//                        gate" for the calibration run that justifies this number.
//   --min-px <N>        Minimum pixel count for the largest bright blob to be trusted as "the
//                        orb" rather than noise/a stray UI element. Default 0.05% of the image.
//   --json               Also emit a machine-readable JSON report on stdout (single line, after
//                        the human-readable report).
//   --debug              Print stack traces on failure.
//
// EXIT CODES
//   0 = orb found and round (max deviation <= threshold)
//   6 = not round, OR the measurement could not be trusted (no orb found, too few boundary
//       crossings, unsupported/corrupt PNG, missing file). This gate never exits 0 on an empty
//       or partial measurement — see `fail()`.
//
// HOW TO PRODUCE A SCREENSHOT TO FEED THIS GATE
//   This is a plain Node CLI script; it cannot call MCP tools (e.g. the iOS Simulator control
//   tool), so it cannot take its own screenshot. Produce one out-of-band, then pass the path:
//     - iOS Simulator, from an agent session with simulator MCP access: the `control` tool's
//       `screenshot` action returns a PNG directly.
//     - iOS Simulator, from a plain shell: `xcrun simctl io booted screenshot out.png`
//     - Android emulator/device: `adb exec-out screencap -p > out.png`
//     - Any other source: any PNG with the orb on a near-black background works — this gate
//       does not care how the PNG was produced, only what its pixels show.
//   Non-interlaced PNG only (see LIMITATIONS below); every tool listed above produces that.
//
// METHOD
//   1. Decode the PNG (hand-rolled decoder below — see LIMITATIONS for what it supports).
//   2. Convert to luminance and binarize with Otsu's method (automatic threshold that
//      maximizes between-class variance — not a hand-picked brightness cutoff).
//   3. Flood-fill connected components on the binary mask; the LARGEST bright blob is treated
//      as "the orb" (this is what makes the gate ignore status-bar icons and text — those are
//      small, separate blobs on the same near-black background).
//   4. Compute that blob's centroid (mean x, mean y of its member pixels — a true pixel-mass
//      centroid, not a bounding-box center, so an asymmetric/lobed shape is not measured from a
//      misleading "middle").
//   5. Cast `--rays` rays from the centroid at evenly spaced angles. Walk outward in 0.2px
//      steps, bilinearly sampling luminance, and record the last radius that was still above
//      the Otsu threshold before a run of consecutive below-threshold samples confirms the ray
//      has exited the shape (a debounce, so one dark interior pixel/gradient dip doesn't get
//      mistaken for the boundary).
//   6. Report min/max/mean/stddev radius and the max deviation as % of mean. A true circle
//      should read close to 0%; a faceted polygon reads a clearly larger number BY
//      CONSTRUCTION (its radius is genuinely not constant vs angle) — see the calibration
//      table in the README for the measured numbers this gate's default threshold is based on.
//
// LIMITATIONS (what this gate CANNOT detect / does not attempt)
//   - Interlaced (Adam7) PNGs are rejected with a clear error, not silently mis-decoded.
//   - A shape whose centroid falls outside its own silhouette (e.g. a crescent, or a "cloud" of
//     lobes concave enough to exclude its own center of mass) will show ray misses; this gate
//     treats that as a measurement failure (exit 6), not as "concave, but who cares" — see #3
//     in the module docstring above.
//   - Colour is not checked — this measures ONLY shape, never whether the orb is the right hue.
//   - A soft/blurred glow halo around a crisp core is scored on whatever Otsu's automatic
//     threshold decides is "the orb": a dim-enough halo falls below threshold and is ignored
//     (in practice, correctly measuring the crisp body); a bright/large enough halo could get
//     folded into the measured blob and bias the boundary outward. This was not something we
//     had a real bright-halo screenshot to calibrate against, so treat that combination as an
//     unverified case.
//   - Compression/scaling artifacts from a lossy re-encode (this gate assumes a PNG straight off
//     a simulator/device, not a JPEG re-save or a resized copy) are not modeled.
//   - This is a 2D silhouette check. It cannot tell a round disc from a round-but-oblate
//     ellipse-that-happens-to-be-radially-symmetric-in-this-one-screenshot if the capture only
//     ever shows one orientation — it reports what the pixels show, nothing about 3D geometry.

import { readFileSync } from 'node:fs';
import { inflateSync } from 'node:zlib';
import { pathToFileURL } from 'node:url';

// ---------------------------------------------------------------------------------------------
// CRC-32 (needed to verify PNG chunk integrity; also reused by the test file's PNG encoder so
// both directions of the codec agree on one implementation).
// ---------------------------------------------------------------------------------------------

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

export function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

// ---------------------------------------------------------------------------------------------
// Minimal PNG decoder. Supports non-interlaced PNGs, bit depth 8/16 for color types 0/2/4/6, and
// bit depth 1/2/4/8 for color types 0 (greyscale) and 3 (indexed) — i.e. every combination the
// PNG spec actually allows, except Adam7 interlacing (rejected explicitly, see LIMITATIONS).
// Every real screenshot source this gate documents (simctl, adb) emits 8-bit non-interlaced
// PNG, so the generalization beyond that is defensive breadth, not the tested-hard path.
// ---------------------------------------------------------------------------------------------

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const CHANNELS_BY_COLOR_TYPE = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 };

function paethPredictor(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  if (pb <= pc) return b;
  return c;
}

function readSample(recon, rowStart, bitPos, bitDepth) {
  if (bitDepth === 8) return recon[rowStart + (bitPos >> 3)];
  if (bitDepth === 16) {
    const off = rowStart + (bitPos >> 3);
    return (recon[off] << 8) | recon[off + 1];
  }
  const byteOff = rowStart + (bitPos >> 3);
  const bitsIntoByte = bitPos & 7;
  const shift = 8 - bitDepth - bitsIntoByte;
  const mask = (1 << bitDepth) - 1;
  return (recon[byteOff] >> shift) & mask;
}

/** Decodes a PNG buffer to {width, height, rgba: Uint8Array(width*height*4)}. Throws with a
 * specific, actionable message on anything it does not support — never returns a silently wrong
 * decode. */
export function decodePNG(buf) {
  if (buf.length < 8 || !buf.subarray(0, 8).equals(PNG_SIGNATURE)) {
    throw new Error('not a PNG file (bad 8-byte signature)');
  }
  let offset = 8;
  let width;
  let height;
  let bitDepth;
  let colorType;
  let interlace;
  let palette = null;
  const idatChunks = [];

  while (offset + 8 <= buf.length) {
    const length = buf.readUInt32BE(offset);
    const type = buf.toString('ascii', offset + 4, offset + 8);
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    if (dataEnd + 4 > buf.length) throw new Error(`truncated PNG: ${type} chunk runs past end of file`);
    const data = buf.subarray(dataStart, dataEnd);
    const crcRead = buf.readUInt32BE(dataEnd);
    const crcCalc = crc32(Buffer.concat([Buffer.from(type, 'ascii'), data]));
    if (crcCalc !== crcRead) {
      process.stderr.write(`[orb-shape-gate] warning: CRC mismatch in ${type} chunk — file may be corrupt\n`);
    }
    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data.readUInt8(8);
      colorType = data.readUInt8(9);
      interlace = data.readUInt8(12);
    } else if (type === 'PLTE') {
      palette = [];
      for (let i = 0; i + 2 < data.length; i += 3) palette.push([data[i], data[i + 1], data[i + 2]]);
    } else if (type === 'IDAT') {
      idatChunks.push(Buffer.from(data));
    } else if (type === 'IEND') {
      break;
    }
    offset = dataEnd + 4;
  }

  if (width === undefined) throw new Error('missing IHDR chunk — not a valid PNG');
  if (interlace !== 0) {
    throw new Error(
      'interlaced (Adam7) PNG is not supported — re-export as non-interlaced. ' +
        'simctl/adb screenshots are non-interlaced by default; if this came from somewhere ' +
        'else, re-save it as a plain baseline PNG.',
    );
  }
  const channels = CHANNELS_BY_COLOR_TYPE[colorType];
  if (channels === undefined) throw new Error(`unsupported PNG color type ${colorType}`);
  if (colorType === 3 && !palette) throw new Error('indexed-color PNG is missing its PLTE chunk');
  if (idatChunks.length === 0) throw new Error('PNG has no IDAT chunks (no pixel data)');

  const raw = inflateSync(Buffer.concat(idatChunks));

  const bitsPerPixel = channels * bitDepth;
  const bytesPerPixel = Math.max(1, Math.ceil(bitsPerPixel / 8));
  const rowBytes = Math.ceil((width * bitsPerPixel) / 8);
  const expectedRawBytes = (rowBytes + 1) * height;
  if (raw.length < expectedRawBytes) {
    throw new Error(
      `decompressed PNG data is short: got ${raw.length} bytes, need ${expectedRawBytes} ` +
        `for ${width}x${height} at ${bitDepth}bpc/colorType=${colorType}`,
    );
  }

  const recon = new Uint8Array(rowBytes * height);
  let pos = 0;
  for (let y = 0; y < height; y++) {
    const filterType = raw[pos];
    pos += 1;
    const rowStart = y * rowBytes;
    for (let i = 0; i < rowBytes; i++) {
      const x = raw[pos + i];
      const a = i >= bytesPerPixel ? recon[rowStart + i - bytesPerPixel] : 0;
      const b = y > 0 ? recon[rowStart - rowBytes + i] : 0;
      const c = y > 0 && i >= bytesPerPixel ? recon[rowStart - rowBytes + i - bytesPerPixel] : 0;
      let val;
      switch (filterType) {
        case 0:
          val = x;
          break;
        case 1:
          val = x + a;
          break;
        case 2:
          val = x + b;
          break;
        case 3:
          val = x + Math.floor((a + b) / 2);
          break;
        case 4:
          val = x + paethPredictor(a, b, c);
          break;
        default:
          throw new Error(`unknown PNG scanline filter type ${filterType} at row ${y}`);
      }
      recon[rowStart + i] = val & 0xff;
    }
    pos += rowBytes;
  }

  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    const rowStart = y * rowBytes;
    let bitPos = 0;
    for (let xpix = 0; xpix < width; xpix++) {
      const samples = new Array(channels);
      for (let ch = 0; ch < channels; ch++) {
        samples[ch] = readSample(recon, rowStart, bitPos, bitDepth);
        bitPos += bitDepth;
      }
      const outIdx = (y * width + xpix) * 4;
      let r;
      let g;
      let b;
      let a = 255;
      if (colorType === 0) {
        let v = samples[0];
        if (bitDepth < 8) v = Math.round((v * 255) / ((1 << bitDepth) - 1));
        else if (bitDepth === 16) v = v >>> 8;
        r = g = b = v;
      } else if (colorType === 2) {
        [r, g, b] = samples;
        if (bitDepth === 16) {
          r >>>= 8;
          g >>>= 8;
          b >>>= 8;
        }
      } else if (colorType === 3) {
        const idx = samples[0];
        const p = palette[idx] || [0, 0, 0];
        [r, g, b] = p;
      } else if (colorType === 4) {
        let [v, al] = samples;
        if (bitDepth === 16) {
          v >>>= 8;
          al >>>= 8;
        }
        r = g = b = v;
        a = al;
      } else {
        // colorType === 6
        let [rr, gg, bb, al] = samples;
        if (bitDepth === 16) {
          rr >>>= 8;
          gg >>>= 8;
          bb >>>= 8;
          al >>>= 8;
        }
        r = rr;
        g = gg;
        b = bb;
        a = al;
      }
      rgba[outIdx] = r;
      rgba[outIdx + 1] = g;
      rgba[outIdx + 2] = b;
      rgba[outIdx + 3] = a;
    }
  }

  return { width, height, rgba };
}

// ---------------------------------------------------------------------------------------------
// Analysis: luminance -> Otsu threshold -> largest connected component -> centroid -> ray cast
// -> stats. Every function below is pure (no I/O) so the test file can exercise it directly.
// ---------------------------------------------------------------------------------------------

/** Rec. 709 perceptual luma from RGBA8. Alpha is ignored (screenshots are opaque in practice;
 * documented as a limitation, not silently "handled"). */
export function toLuminance(rgba, width, height) {
  const lum = new Uint8Array(width * height);
  for (let i = 0; i < lum.length; i++) {
    const o = i * 4;
    lum[i] = Math.round(0.2126 * rgba[o] + 0.7152 * rgba[o + 1] + 0.0722 * rgba[o + 2]);
  }
  return lum;
}

/** Otsu's method: the threshold that maximizes between-class variance of a bimodal histogram.
 * This is what makes the fg/bg split a MEASURED number rather than a hand-picked brightness
 * cutoff — it adapts to whatever background/foreground tones a given screenshot actually has. */
export function otsuThreshold(luminance) {
  const hist = new Uint32Array(256);
  for (let i = 0; i < luminance.length; i++) hist[luminance[i]]++;
  const total = luminance.length;
  let sum = 0;
  for (let t = 0; t < 256; t++) sum += t * hist[t];

  let sumB = 0;
  let wB = 0;
  let maxVar = -1;
  let threshold = 127;
  for (let t = 0; t < 256; t++) {
    wB += hist[t];
    if (wB === 0) continue;
    const wF = total - wB;
    if (wF === 0) break;
    sumB += t * hist[t];
    const mB = sumB / wB;
    const mF = (sum - sumB) / wF;
    const varBetween = wB * wF * (mB - mF) * (mB - mF);
    if (varBetween > maxVar) {
      maxVar = varBetween;
      threshold = t;
    }
  }
  return threshold;
}

/** Flood-fills the binary mask (luminance > threshold) and returns the largest component as
 * {count, cx, cy, minX, maxX, minY, maxY}, or null if there is no foreground pixel at all. */
export function largestComponent(luminance, width, height, threshold) {
  const n = width * height;
  const visited = new Uint8Array(n);
  const stack = new Int32Array(n);
  let best = null;

  for (let start = 0; start < n; start++) {
    if (visited[start] || luminance[start] <= threshold) continue;
    let sp = 0;
    stack[sp++] = start;
    visited[start] = 1;
    let count = 0;
    let sumX = 0;
    let sumY = 0;
    let minX = width;
    let maxX = -1;
    let minY = height;
    let maxY = -1;
    while (sp > 0) {
      const idx = stack[--sp];
      const x = idx % width;
      const y = (idx / width) | 0;
      count++;
      sumX += x;
      sumY += y;
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
      if (x > 0) {
        const i2 = idx - 1;
        if (!visited[i2] && luminance[i2] > threshold) {
          visited[i2] = 1;
          stack[sp++] = i2;
        }
      }
      if (x < width - 1) {
        const i2 = idx + 1;
        if (!visited[i2] && luminance[i2] > threshold) {
          visited[i2] = 1;
          stack[sp++] = i2;
        }
      }
      if (y > 0) {
        const i2 = idx - width;
        if (!visited[i2] && luminance[i2] > threshold) {
          visited[i2] = 1;
          stack[sp++] = i2;
        }
      }
      if (y < height - 1) {
        const i2 = idx + width;
        if (!visited[i2] && luminance[i2] > threshold) {
          visited[i2] = 1;
          stack[sp++] = i2;
        }
      }
    }
    if (!best || count > best.count) {
      best = { count, cx: sumX / count, cy: sumY / count, minX, maxX, minY, maxY };
    }
  }
  return best;
}

function bilinearLuminance(luminance, width, height, x, y) {
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const x1 = Math.min(x0 + 1, width - 1);
  const y1 = Math.min(y0 + 1, height - 1);
  const fx = x - x0;
  const fy = y - y0;
  const v00 = luminance[y0 * width + x0];
  const v10 = luminance[y0 * width + x1];
  const v01 = luminance[y1 * width + x0];
  const v11 = luminance[y1 * width + x1];
  const top = v00 + (v10 - v00) * fx;
  const bot = v01 + (v11 - v01) * fx;
  return top + (bot - top) * fy;
}

/** Casts `rayCount` rays from (cx, cy) across a full 2*PI, walking outward in `step`-pixel
 * increments up to `maxRadius`. For each ray, records the last radius sampled as "inside" (above
 * threshold) before `debouncePx` worth of consecutive "outside" samples confirm the ray has
 * truly exited the shape (guards against one dark interior pixel reading as the edge). A ray
 * that never sees an inside sample returns null for that angle — a miss, not a zero. */
export function castRays({ luminance, width, height, threshold, cx, cy, rayCount, maxRadius, step = 0.2, debouncePx = 2 }) {
  const debounceSteps = Math.max(4, Math.round(debouncePx / step));
  const radii = new Array(rayCount).fill(null);
  for (let k = 0; k < rayCount; k++) {
    const angle = (k / rayCount) * 2 * Math.PI;
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    let lastInsideR = null;
    let consecutiveOutside = 0;
    for (let r = 0; r <= maxRadius; r += step) {
      const x = cx + cos * r;
      const y = cy + sin * r;
      if (x < 0 || y < 0 || x >= width - 1 || y >= height - 1) break;
      const lum = bilinearLuminance(luminance, width, height, x, y);
      if (lum > threshold) {
        lastInsideR = r;
        consecutiveOutside = 0;
      } else if (lastInsideR !== null) {
        consecutiveOutside++;
        if (consecutiveOutside >= debounceSteps) break;
      }
    }
    radii[k] = lastInsideR;
  }
  return radii;
}

/** min/max/mean/std over the non-null radii, plus maxDeviationPct = max(|r-mean|)/mean*100. */
export function radiusStats(radii) {
  const valid = radii.filter((r) => r !== null);
  if (valid.length === 0) return null;
  const min = Math.min(...valid);
  const max = Math.max(...valid);
  const mean = valid.reduce((a, b) => a + b, 0) / valid.length;
  const variance = valid.reduce((a, b) => a + (b - mean) ** 2, 0) / valid.length;
  const std = Math.sqrt(variance);
  const maxDeviationPct = mean > 0 ? (Math.max(...valid.map((r) => Math.abs(r - mean))) / mean) * 100 : Infinity;
  return { min, max, mean, std, maxDeviationPct, validCount: valid.length, totalCount: radii.length };
}

/** A compact one-line unicode sparkline of radius vs angle, downsampled into `cols` buckets, so
 * a human can see facets (a polygon shows a repeating sawtooth; a circle shows a flat line). */
export function asciiSparkline(radii, cols = 72) {
  // No blank/space level: the bucket with the SMALLEST radius is the most diagnostically
  // important one (it's the facet dip), so it must render as a visible bar, not empty space.
  // '?' is reserved separately below for a bucket with literally no ray data in it.
  const blocks = '▁▂▃▄▅▆▇█';
  const valid = radii.filter((r) => r !== null);
  if (valid.length === 0) return '(no data)';
  const min = Math.min(...valid);
  const max = Math.max(...valid);
  const span = max - min || 1;
  const n = radii.length;
  let out = '';
  for (let c = 0; c < cols; c++) {
    const startIdx = Math.floor((c / cols) * n);
    const endIdx = Math.max(startIdx + 1, Math.floor(((c + 1) / cols) * n));
    let sum = 0;
    let count = 0;
    for (let i = startIdx; i < endIdx && i < n; i++) {
      if (radii[i] !== null) {
        sum += radii[i];
        count++;
      }
    }
    if (count === 0) {
      out += '?';
      continue;
    }
    const avg = sum / count;
    const level = Math.round(((avg - min) / span) * (blocks.length - 1));
    out += blocks[Math.max(0, Math.min(blocks.length - 1, level))];
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------------------------

const MIN_RAYS = 180;
const DEFAULT_RAYS = 360;
// Justified by the calibration run in scripts/orb-shape-gate.test.mjs and the README "orb shape
// gate" section — measured, not guessed:
//   real ROUND iOS screenshot (evidence/ui-drive-070313/01-launch.png):  0.09% deviation
//   synthetic true circle (640px canvas, r=240):                        0.06% deviation
//   synthetic 24-sided regular polygon (same r, apothem=240):           0.50% deviation
//   synthetic 12-sided regular polygon (same r, apothem=240):           2.35% deviation
//   real NON-round Android screenshot (a lobed "cloud" of circles):    25.60% deviation
// 0.25% is the geometric mean of the best observed round-orb reading (0.09%) and the mildest
// known-bad control (the 24-gon, 0.50%) — it sits with real margin above real-world round-orb
// noise (2.8x) and real margin below even the SUBTLEST facet defect tested (2.0x), while sitting
// two orders of magnitude below the actual observed regression (25.60%). See LIMITATIONS above:
// this noise floor scales with orb radius in pixels, so a much smaller/lower-res screenshot may
// need a looser --threshold — recalibrate against that resolution rather than assuming this
// number travels unchanged.
const DEFAULT_THRESHOLD_PCT = 0.25;
const DEFAULT_MIN_PX_FRACTION = 0.0005; // 0.05% of image area

function parseArgs(argv) {
  const args = { rays: DEFAULT_RAYS, threshold: DEFAULT_THRESHOLD_PCT, json: false, debug: false, minPx: null };
  const positional = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--rays') args.rays = Number(argv[++i]);
    else if (a === '--threshold') args.threshold = Number(argv[++i]);
    else if (a === '--min-px') args.minPx = Number(argv[++i]);
    else if (a === '--json') args.json = true;
    else if (a === '--debug') args.debug = true;
    else positional.push(a);
  }
  args.imagePath = positional[0];
  return args;
}

class GateFailure extends Error {}

function runGate(imagePath, opts = {}) {
  const rays = Math.max(MIN_RAYS, Number.isFinite(opts.rays) ? opts.rays : DEFAULT_RAYS);
  if (Number.isFinite(opts.rays) && opts.rays < MIN_RAYS) {
    process.stderr.write(`[orb-shape-gate] warning: --rays ${opts.rays} is below the ${MIN_RAYS} floor; using ${MIN_RAYS}\n`);
  }
  const threshold = Number.isFinite(opts.threshold) ? opts.threshold : DEFAULT_THRESHOLD_PCT;

  if (!imagePath) {
    throw new GateFailure('no screenshot path given — usage: node scripts/orb-shape-gate.mjs <screenshot.png>');
  }

  let fileBuf;
  try {
    fileBuf = readFileSync(imagePath);
  } catch (err) {
    throw new GateFailure(`could not read ${imagePath}: ${err.message}`);
  }

  let decoded;
  try {
    decoded = decodePNG(fileBuf);
  } catch (err) {
    throw new GateFailure(`could not decode ${imagePath} as PNG: ${err.message}`);
  }
  const { width, height, rgba } = decoded;
  const luminance = toLuminance(rgba, width, height);
  const otsu = otsuThreshold(luminance);

  const minPx = opts.minPx != null ? opts.minPx : Math.round(width * height * DEFAULT_MIN_PX_FRACTION);
  const comp = largestComponent(luminance, width, height, otsu);
  if (!comp) {
    throw new GateFailure(
      `found no pixel brighter than the Otsu threshold (${otsu}/255) in ${width}x${height} ${imagePath} — ` +
        'no orb-like blob to measure. Nothing was measured, so this is a FAIL, not a pass.',
    );
  }
  if (comp.count < minPx) {
    throw new GateFailure(
      `largest bright blob is only ${comp.count}px (< ${minPx}px floor, ${(DEFAULT_MIN_PX_FRACTION * 100).toFixed(2)}% ` +
        `of the ${width}x${height} image) — too small to trust as "the orb"; likely noise or a stray UI element.`,
    );
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

  const radii = castRays({ luminance, width, height, threshold: otsu, cx: comp.cx, cy: comp.cy, rayCount: rays, maxRadius });
  const stats = radiusStats(radii);
  const misses = radii.length - (stats ? stats.validCount : 0);

  const report = {
    imagePath,
    width,
    height,
    otsuThreshold: otsu,
    component: { pixels: comp.count, bbox: [comp.minX, comp.minY, comp.maxX, comp.maxY], centroid: [comp.cx, comp.cy] },
    rays: radii.length,
    misses,
    stats,
    thresholdPct: threshold,
  };

  const lines = [];
  lines.push(`[orb-shape-gate] image: ${imagePath} (${width}x${height})`);
  lines.push(`[orb-shape-gate] background/foreground split (Otsu threshold): ${otsu} of 255`);
  lines.push(
    `[orb-shape-gate] orb component: ${comp.count}px, bbox [${comp.minX},${comp.minY}]-[${comp.maxX},${comp.maxY}], ` +
      `centroid=(${comp.cx.toFixed(1)}, ${comp.cy.toFixed(1)})`,
  );

  if (!stats || misses > 0) {
    lines.push(`[orb-shape-gate] rays: ${radii.length} cast, ${radii.length - misses} crossings found, ${misses} MISSED`);
    console.log(lines.join('\n'));
    throw new GateFailure(
      `${misses} of ${radii.length} rays never found a boundary crossing (never left the bright blob before running ` +
        'out of search radius, or the centroid itself was not inside the blob). A shape this concave relative to its ' +
        'own centroid cannot be trusted as measured — this is a FAIL, not a pass on partial data.',
    );
  }

  lines.push(`[orb-shape-gate] rays: ${radii.length} cast, ${radii.length} crossings found (0 misses)`);
  lines.push(
    `[orb-shape-gate] radius: min=${stats.min.toFixed(2)}  max=${stats.max.toFixed(2)}  mean=${stats.mean.toFixed(2)}  std=${stats.std.toFixed(2)}`,
  );
  lines.push(
    `[orb-shape-gate] max deviation: ${stats.maxDeviationPct.toFixed(2)}% of mean radius  (threshold: ${threshold.toFixed(2)}%)`,
  );
  lines.push('[orb-shape-gate] radius-vs-angle profile (0deg=east, going counter-clockwise):');
  lines.push('  ' + asciiSparkline(radii));

  const pass = stats.maxDeviationPct <= threshold;
  lines.push(
    pass
      ? `[orb-shape-gate] PASS — orb is round (${stats.maxDeviationPct.toFixed(2)}% <= ${threshold.toFixed(2)}%)`
      : `[orb-shape-gate] FAIL — orb is NOT round (${stats.maxDeviationPct.toFixed(2)}% > ${threshold.toFixed(2)}%)`,
  );
  console.log(lines.join('\n'));
  if (opts.json) console.log(JSON.stringify(report));

  if (!pass) {
    throw new GateFailure(`max deviation ${stats.maxDeviationPct.toFixed(2)}% exceeds threshold ${threshold.toFixed(2)}%`);
  }
  return report;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  try {
    runGate(args.imagePath, args);
    process.exit(0);
  } catch (err) {
    if (err instanceof GateFailure) {
      console.error(`[orb-shape-gate] FAIL: ${err.message}`);
    } else {
      console.error(`[orb-shape-gate] FAIL: unexpected error: ${err.message}`);
    }
    if (args.debug && err.stack) console.error(err.stack);
    process.exit(6);
  }
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (isMain) {
  main();
}

export { runGate, GateFailure, DEFAULT_THRESHOLD_PCT, DEFAULT_MIN_PX_FRACTION, MIN_RAYS };
