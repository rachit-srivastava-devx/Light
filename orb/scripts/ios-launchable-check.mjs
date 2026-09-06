// Asserts a built iOS .app can actually LOAD JAVASCRIPT — i.e. that it will render the product
// rather than React Native's red "No script URL provided" box.
//
// The defect this closes (observed 2026-08-28): `xcodebuild` reported `** BUILD SUCCEEDED **`, the
// app installed and launched on a real simulator, and the screen showed only:
//     No script URL provided. Make sure the packager is running or you have embedded a JS bundle
//     in your application bundle.  unsanitizedScriptURLString = (null)
// A native build succeeding is a PROXY. The property is "a person opening this app sees the app".
//
// React Native resolves its JS differently per configuration: a Debug build loads from the Metro
// packager, a Release build loads an embedded `main.jsbundle`. So "a bundle exists" is the wrong
// question. This script asks the right one, and — importantly — makes the answer EXPLICIT instead
// of leaving it to be discovered at launch:
//
//   * embedded `main.jsbundle` present  -> self-contained, launchable anywhere. PASS.
//   * no embedded bundle, packager reachable -> launchable HERE AND NOW, but not self-contained.
//     PASS only with --allow-packager, and it says so loudly.
//   * no embedded bundle, no packager    -> this build renders a red box. FAIL.
//
// Usage:
//   node scripts/ios-launchable-check.mjs <path-to-.app> [--allow-packager] [--packager-url URL]

import { existsSync, statSync } from 'node:fs';
import { join } from 'node:path';

const args = process.argv.slice(2);
const appPath = args.find((a) => !a.startsWith('--'));
const allowPackager = args.includes('--allow-packager');
const urlFlagIdx = args.indexOf('--packager-url');
const packagerUrl = urlFlagIdx >= 0 ? args[urlFlagIdx + 1] : 'http://127.0.0.1:8081/status';

// A bundle far below this is a truncated/empty artifact, not a real app bundle. RN bundles for a
// non-trivial app run to megabytes; 200 KB is a deliberately generous floor chosen to catch the
// "file exists but is empty/aborted" case without guessing at this app's real size.
const MIN_BUNDLE_BYTES = 200 * 1024;

function fail(msg) {
  console.error(`[ios-launchable] FAIL ${msg}`);
  process.exit(1);
}

if (!appPath) fail('no .app path given — usage: node scripts/ios-launchable-check.mjs <path-to-.app>');
if (!existsSync(appPath)) fail(`.app does not exist: ${appPath}`);

const embedded = join(appPath, 'main.jsbundle');

if (existsSync(embedded)) {
  const bytes = statSync(embedded).size;
  if (bytes < MIN_BUNDLE_BYTES) {
    fail(`embedded main.jsbundle is only ${bytes} bytes (< ${MIN_BUNDLE_BYTES}) — truncated or empty, `
       + 'the app would fail to load JS at launch');
  }
  console.log(`[ios-launchable] PASS embedded main.jsbundle present (${bytes} bytes) — self-contained`);
  process.exit(0);
}

// No embedded bundle. The app can only launch if a packager is serving JS.
let packagerUp = false;
try {
  const res = await fetch(packagerUrl, { signal: AbortSignal.timeout(2000) });
  packagerUp = res.ok;
} catch {
  packagerUp = false;
}

if (!packagerUp) {
  fail(`no embedded main.jsbundle in ${appPath} AND no packager reachable at ${packagerUrl}.\n`
     + '       This build WILL render React Native\'s red "No script URL provided" screen.\n'
     + '       Fix: run `npm run ios:bundle:local` before building (release-style, self-contained),\n'
     + '       or start Metro (`npm run mobile:start`) and re-run with --allow-packager.');
}

if (!allowPackager) {
  fail(`no embedded main.jsbundle in ${appPath}. A packager IS reachable at ${packagerUrl}, so this\n`
     + '       build launches on THIS machine only and is not self-contained. Pass --allow-packager\n'
     + '       to accept that deliberately (dev workflow), or bundle first for a portable build.');
}

console.log(`[ios-launchable] PASS no embedded bundle, but packager reachable at ${packagerUrl} `
          + '(--allow-packager) — launchable here, NOT self-contained');
process.exit(0);
