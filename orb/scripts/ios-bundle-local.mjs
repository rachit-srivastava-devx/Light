// iOS counterpart of scripts/android-bundle-local.mjs.
//
// Why this exists: `ios:build:check` used to run `xcodebuild` alone, while `android:build:check`
// ran `android:bundle:local` first. That asymmetry meant the iOS check could only ever prove the
// NATIVE half compiles — it structurally could not detect a missing or broken JS bundle. On
// 2026-08-28 a real drive launched an app that had passed `** BUILD SUCCEEDED **` and rendered
// only a red box: "No script URL provided. Make sure the packager is running or you have embedded
// a JS bundle in your application bundle. unsanitizedScriptURLString = (null)". 562 TS tests, 179
// Python tests and 27 Rust tests were green at the same moment.
//
// This produces the release-style embedded bundle. Pair it with scripts/ios-launchable-check.mjs,
// which asserts the built .app is actually launchable rather than assuming it.

import { mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';

const OUT_DIR = 'ios/build-js';
const OUT_BUNDLE = `${OUT_DIR}/main.jsbundle`;

mkdirSync(OUT_DIR, { recursive: true });
mkdirSync(`${OUT_DIR}/assets`, { recursive: true });

const result = spawnSync(
  'npx',
  [
    'react-native',
    'bundle',
    '--platform',
    'ios',
    '--dev',
    'false',
    '--entry-file',
    'index.js',
    '--bundle-output',
    OUT_BUNDLE,
    '--assets-dest',
    `${OUT_DIR}/assets`,
    '--config',
    'metro.config.js',
  ],
  { stdio: 'inherit' },
);

if (result.status !== 0) {
  console.error(`[ios-bundle] FAILED (exit ${result.status ?? 'signal'}) — no usable JS bundle produced`);
  process.exit(result.status ?? 1);
}

console.log(`[ios-bundle] OK -> ${OUT_BUNDLE}`);
process.exit(0);
