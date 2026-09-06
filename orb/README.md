# The Focus Orb (Phase 1)

A React Native voice body-doubling companion for ADHD — one screen, one orb, a never-silent
on-device noise bed, and a cheap-LLM crew that only ever fills language, never decides state.

Read [`AGENTS.md`](AGENTS.md) first, then [`docs/BUILD-DIGEST.md`](docs/BUILD-DIGEST.md) — the
full build spec extracted from the blueprint. The blueprint itself
(`../../../blueprints/ADHD-Focus-Orb-L8-Deep-Dive/`) is read-only; it is never edited by
build work. The end-to-end implementation contract for the active greeting, pure technical
conversation, ADHD task loop, five-minute time boxes, and low-latency voice path is
[`docs/L8-IMPLEMENTATION-DETAIL.md`](docs/L8-IMPLEMENTATION-DETAIL.md).
The focused model-selection, golden-dataset, and response-quality evaluation plan is
[`docs/LLM-SELECTION-AND-EVALS.md`](docs/LLM-SELECTION-AND-EVALS.md).
The interaction-model research and “steal / do not steal” decisions are in
[`docs/INTERACTION-MODELS-RESEARCH.md`](docs/INTERACTION-MODELS-RESEARCH.md).
The 42–6–42 study-with-me co-presence template is in
[`docs/STUDY-WITH-ME-REFERENCE-TEMPLATE.md`](docs/STUDY-WITH-ME-REFERENCE-TEMPLATE.md).
The consolidated L8 change plan is in
[`docs/L8-CHANGE-PLAN.md`](docs/L8-CHANGE-PLAN.md).

```
npm install
npm run verify   # boundary-lint + typecheck + TS/Python/Rust tests
npm run smoke:t0 # starts sidecar + relay, drives the T0 HTTP path, then stops both
npm run smoke:realtime # starts Rust relay and proves WebSocket STT/TTS chunks
npm run mobile:bundle # validates a release JavaScript bundle through Metro
npm run mobile:start  # starts Metro for a native host
npm run native:preflight
npm run ios:build:check
npm run android:build:check
npm run smoke:android:e2e # builds, installs, launches, asserts orb-only UI + native speech
```

The mobile package is configured for the React Native CLI, `react-native-audio-api`, and owned
native audio hooks. `OrbSpeech` bridges platform TTS so the app speaks instead of rendering step
text. Android also has a small native `OrbPresence`/`OrbGreeting` bootstrap so strict app-open
audio budgets are met before React finishes mounting; see `docs/adr/0008-native-android-presence-bootstrap.md`.
The `ios/` and `android/` host projects are generated from the React Native 0.83.0 template for
`OrbMobile`. A simulator/device run requires the platform toolchain, CocoaPods for iOS, Android
SDK/Gradle for Android, and the audio API native dependency.
Current local evidence: `pod install` succeeds and `react-native config` discovers both hosts, but
`native:preflight` fails until CoreSimulator responds after the pending macOS/Xcode component
update. Android SDK 36 is configured through `/opt/homebrew/share/android-commandlinetools`;
`android:build:check` packages the local JS bundle and builds the debug APK, and
`ORB_STRICT_AUDIO_BUDGETS=1 npm run smoke:android:e2e` runs that APK end to end in the
`FocusOrb_API36` emulator with a dark, orb-only cloud surface, native TTS queue proof, captured
UI/log evidence, and green native startup budgets.

If your shell cannot find Cargo, the npm script prepends `$HOME/.cargo/bin` for the Rust gate. In
non-login automation, use:

```
PATH=$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin npm run verify
```

For code traversal, start with [`docs/JUNIOR-TRAVERSAL.md`](docs/JUNIOR-TRAVERSAL.md).

## Orb shape gate

`npm run test`/`typecheck` can stay 100% green while the orb renders as a faceted polygon or a
clipped/lobed blob — nothing in a JS/TS unit test observes what a GPU actually rasterized to the
screen. `scripts/orb-shape-gate.mjs` closes that gap by measuring circularity from **actual
rendered pixels** of a screenshot, with no code-level shortcut possible:

```sh
node scripts/orb-shape-gate.mjs <screenshot.png> [--rays N] [--threshold PCT] [--json] [--debug]
```

Exit `0` = round. Exit `6` = not round, **or** the measurement itself could not be trusted (no
orb-like blob found, too few of the `--rays` (>=180, default 360) angular samples found a
boundary, an unreadable/unsupported PNG). This gate never exits `0` on an empty or partial
measurement — see the script's own header comment for the full method and its documented
limitations (interlaced PNGs, concave-from-centroid shapes, colour is not checked, etc).

Producing a screenshot is out of this script's scope on purpose — a plain Node CLI cannot call
an MCP tool such as the iOS Simulator control tool, so it never takes its own screenshot:

- iOS Simulator, from an agent session with simulator MCP access: the `control` tool's
  `screenshot` action returns a PNG directly.
- iOS Simulator, from a plain shell: `xcrun simctl io booted screenshot out.png`
- Android emulator/device: `adb exec-out screencap -p > out.png`
- Any other source is fine too — this gate only cares what the pixels show, not how they were
  captured. Non-interlaced PNG only; every tool above produces that by default.

Method, in short: decode the PNG (hand-rolled decoder, `node:zlib` for the DEFLATE stream, no
new dependency) -> luminance -> **Otsu's method** picks the background/foreground split
automatically (not a hand-picked brightness cutoff) -> flood-fill connected components, the
largest bright blob is "the orb" (this is what makes stray status-bar icons/text get ignored —
they're small, separate blobs on the same near-black background) -> centroid of that blob's
actual pixel mass (not its bounding-box center, so a lobed/asymmetric shape isn't measured from
a misleading "middle") -> cast `--rays` rays from the centroid, walk outward, record where each
ray last reads "inside" before a run of background samples confirms it exited -> report
min/max/mean/stddev radius and the max deviation as a percentage of the mean, plus a compact
ASCII radius-vs-angle sparkline so a human can see facets directly in the terminal.

### Calibration (run before trusting the default threshold — and rerun if you change the math)

`node scripts/orb-shape-gate.test.mjs` regenerates all of this from scratch: it renders a true
circle and 12-sided/24-sided regular polygons (640px canvas, apothem/radius 240px, ~1px
antialiased edge, drawn from the closed-form polar equation `r(theta) = apothem / cos(phi)` for
a regular polygon) as real PNGs, then drives the **actual CLI as a subprocess**
(`execFileSync('node', ['scripts/orb-shape-gate.mjs', ...])`), not just internal functions — so
it proves the shipped script. It also unit-tests the underlying math directly (CRC32 against the
standard check value, a full PNG encode/decode round-trip, Otsu on a synthetic bimodal
histogram, connected components + centroid on a hand-built mask, the polygon polar formula) and
runs an informational (non-asserted) pass over whatever real orb evidence already exists in the
repo. Measured results from the last run:

| shape | source | max deviation | verdict |
|---|---|---:|---|
| synthetic true circle | generated, r=240 | 0.06% | PASS |
| REAL round orb, iOS | `evidence/ui-drive-070313/01-launch.png` | 0.09% | PASS |
| synthetic 24-gon | generated, apothem=240 | 0.50% | FAIL |
| REAL "AFTER" orb, iOS (2 of 4 captures) | `apps/mobile/evidence/AFTER_auroramist_fixed_{2,4}.png` | 0.07%, 0.13% | PASS |
| synthetic 12-gon | generated, apothem=240 | 2.35% | FAIL |
| REAL "BEFORE" orb, iOS (faceted) | `apps/mobile/evidence/BEFORE_auroramist_faceted_polygon_{1,2}.png` | 3.40%, 4.63% | FAIL |
| REAL "AFTER" orb, iOS (other 2 of 4 captures) | `apps/mobile/evidence/AFTER_auroramist_fixed_{1,3}.png` | 3.65%, 8.37% | FAIL |
| REAL non-round orb, Android (lobed "cloud") | `docs/evidence/android-e2e/focus-orb-android-orb-only.png` | 25.60% | FAIL |

The default `--threshold` is **0.25%** — the geometric mean of the best real round-orb reading
(0.09%, actual device pixels, not synthetic) and the mildest known-bad control tested (the
24-gon, 0.50%). That leaves ~2.8x margin above real round-orb noise and ~2.0x margin below even
the subtlest facet defect calibrated against, while sitting two orders of magnitude below the
actual regression observed on Android (25.60%). This noise floor scales with orb radius in
pixels (a smaller/lower-res screenshot has fewer pixels across the same angular span) — if you
point this at a much smaller capture, recalibrate `--threshold` against *that* resolution rather
than assuming 0.25% travels unchanged.

**A genuinely useful finding from this calibration, not a hypothetical:** of the four
`apps/mobile/evidence/AFTER_auroramist_fixed_*.png` screenshots captured in the same session
(by a parallel agent working on `AuroraMist.tsx`/`CloudOrb.tsx`), **two pass and two fail** at
this threshold. "AFTER" was not, in this evidence, uniformly round. The worse of the two
failures doesn't have an obviously flat edge to the naked eye at normal viewing size:

```
$ node scripts/orb-shape-gate.mjs apps/mobile/evidence/AFTER_auroramist_fixed_3.png
[orb-shape-gate] radius: min=288.40  max=325.40  mean=314.75  std=8.88
[orb-shape-gate] max deviation: 8.37% of mean radius  (threshold: 0.25%)
[orb-shape-gate] radius-vs-angle profile (0deg=east, going counter-clockwise):
  ▅▅▆▆▆▆▆▆▆▅▅▅▅▅▆▆▅▄▄▅▆▆▆▆▇▇▇▇▇▆▆▇▇▇▇▇▇██████▆▄▂▁▁▁▁▂▄▆█████▇▇▇▇▇▇▇█▇▇▇▆▆▅
[orb-shape-gate] FAIL — orb is NOT round (8.37% > 0.25%)
```

versus a passing capture from the same set, whose profile is noisy (natural gradient/blur
variation) rather than a clean repeating dip:

```
$ node scripts/orb-shape-gate.mjs apps/mobile/evidence/AFTER_auroramist_fixed_2.png
[orb-shape-gate] max deviation: 0.07% of mean radius  (threshold: 0.25%)
[orb-shape-gate] radius-vs-angle profile (0deg=east, going counter-clockwise):
  ▃▅▅▅▅▅▅▅▅▅▅▅▅▅▅▅▆▅▄▆▅▅▅▅▅▅▅▅▅▅▅▅▅▅▅▅▄▃▄▂▄▃▂▁▂▂▂▁▂▅▂▃▂▃▂▄▅▅▅▅▅▅▅▇▇▇█▅▆▆▆▄
```

and a synthetic 12-gon, for contrast — a clean, perfectly repeating sawtooth (12 identical
dips), the signature of a coarse regular polygon rather than an organic render:

```
  ▁▂▅▆▃▁▁▂▅▆▃▁▁▃▅▆▃▁▁▂▅▆▃▁▁▂▅▆▃▁▁▃▅▆▃▁▁▂▅▆▃▁▁▂▅▇▃▁▁▃▅▆▃▁▁▂▅▆▃▁▁▂▅▆▃▁▁▃▅▆▃▁
```

The sparkline's vertical axis is min/max-normalized **per image**, so it is a shape-pattern tool
(periodic sawtooth vs. patternless noise), not a cross-image magnitude comparison — read the
printed max-deviation percentage for that. A label of "fixed" is not the same claim as "the
pixels are round," and a quick glance at a screenshot on a near-black background is not much
more reliable than a green unit test for telling the two apart; this gate exists to make that
claim checkable instead of eyeballed.

### Files

- `scripts/orb-shape-gate.mjs` — the gate (decoder, Otsu threshold, connected components,
  ray casting, stats, ASCII sparkline, CLI). Zero new dependencies: PNG decoding uses only
  `node:zlib`'s `inflateSync` on the DEFLATE stream, with a hand-rolled chunk parser/unfilter.
- `scripts/orb-shape-gate.test.mjs` — unit tests for that math, plus the calibration run above.
  Deliberately a plain Node script, not a vitest suite: `vitest.config.ts`'s `test.include` is
  scoped to `apps/mobile` and the two backend sidecars, so a `scripts/**/*.test.ts` file would
  silently never run under `npm run test`.

## Fish Audio voice selection

List voices available to the configured Fish account and public library with `npm run fish:voices`.
Set the chosen Fish `reference_id` in the local ignored `.env`:

```sh
FISH_REFERENCE_ID_ORB_WARM_V1=<fish-reference-id>
```

The app keeps the stable `orb.warm.v1` route, while the voice sidecar resolves that alias to the
selected Fish voice. If a raw Fish `reference_id` is supplied in a speech envelope, it is passed
through unchanged. All voices receive the same bounded S2 pacing layer; explicit Fish markup from
the model is preserved.
