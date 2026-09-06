import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';

const AVD_NAME = process.env.ORB_ANDROID_AVD ?? 'FocusOrb_API36';
const SDK_ROOT = process.env.ANDROID_SDK_ROOT ?? process.env.ANDROID_HOME ?? '/opt/homebrew/share/android-commandlinetools';
const EMULATOR = `${SDK_ROOT}/emulator/emulator`;
const APK = 'android/app/build/outputs/apk/debug/app-debug.apk';
const PACKAGE = 'com.orbmobile';
const ACTIVITY = 'com.orbmobile/.MainActivity';
const EVIDENCE_DIR = 'docs/evidence/android-e2e';
const PRESENCE_READY_BUDGET_MS = 120;
const SPEECH_REQUEST_BUDGET_MS = 250;
const STRICT_AUDIO_BUDGETS = process.env.ORB_STRICT_AUDIO_BUDGETS === '1';

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    stdio: options.stdio ?? 'pipe',
    timeout: options.timeout,
    env: {
      ...process.env,
      ANDROID_HOME: SDK_ROOT,
      ANDROID_SDK_ROOT: SDK_ROOT,
      PATH: `/opt/homebrew/bin:${process.env.HOME}/.cargo/bin:${process.env.PATH ?? ''}`,
    },
  });
  if (result.status !== 0 || result.error) {
    throw new Error(
      `${command} ${args.join(' ')} failed: ${result.error?.message ?? result.stderr ?? result.stdout}`,
    );
  }
  return result.stdout?.trim() ?? '';
}

function tryRun(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    stdio: options.stdio ?? 'pipe',
    timeout: options.timeout,
    env: {
      ...process.env,
      ANDROID_HOME: SDK_ROOT,
      ANDROID_SDK_ROOT: SDK_ROOT,
      PATH: `/opt/homebrew/bin:${process.env.HOME}/.cargo/bin:${process.env.PATH ?? ''}`,
    },
  });
  return {
    ok: result.status === 0 && !result.error,
    stdout: result.stdout?.trim() ?? '',
    stderr: result.error?.message ?? result.stderr?.trim() ?? '',
  };
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitUntil(label, timeoutMs, probe) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const value = await probe();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await wait(1_000);
  }
  throw lastError ?? new Error(`${label} timed out after ${timeoutMs}ms`);
}

function deviceOnline() {
  return tryRun('adb', ['devices']).stdout.split('\n').some((line) => /\bdevice$/.test(line));
}

async function ensureDevice() {
  if (deviceOnline()) return null;
  if (!existsSync(EMULATOR)) {
    throw new Error(`Android emulator not found at ${EMULATOR}; install SDK package "emulator"`);
  }
  const avds = tryRun('avdmanager', ['list', 'avd']).stdout;
  if (!avds.includes(`Name: ${AVD_NAME}`)) {
    throw new Error(`AVD ${AVD_NAME} not found; create it with android-36/google_apis/arm64-v8a`);
  }
  const child = spawn(EMULATOR, ['-avd', AVD_NAME, '-no-snapshot', '-no-boot-anim'], {
    env: {
      ...process.env,
      ANDROID_HOME: SDK_ROOT,
      ANDROID_SDK_ROOT: SDK_ROOT,
      PATH: `/opt/homebrew/bin:${process.env.HOME}/.cargo/bin:${process.env.PATH ?? ''}`,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', (chunk) => process.stdout.write(`[emulator] ${chunk}`));
  child.stderr.on('data', (chunk) => process.stderr.write(`[emulator] ${chunk}`));

  await waitUntil('emulator device registration', 180_000, () => deviceOnline());
  await waitUntil('android boot', 180_000, () => {
    const booted = tryRun('adb', ['shell', 'getprop', 'sys.boot_completed']).stdout.replace(/\r/g, '');
    return booted === '1';
  });
  return child;
}

function dumpUi(target) {
  tryRun('adb', ['shell', 'rm', '-f', `/sdcard/${target}`], { timeout: 10_000 });
  run('adb', ['shell', 'uiautomator', 'dump', `/sdcard/${target}`], { timeout: 10_000 });
  run('adb', ['pull', `/sdcard/${target}`, `/tmp/${target}`], { timeout: 10_000 });
  return readFileSync(`/tmp/${target}`, 'utf8');
}

function screenshot(name) {
  run('sh', ['-lc', `adb exec-out screencap -p > /tmp/${name}.png`], { timeout: 10_000 });
}

function writeEvidence(name, content) {
  mkdirSync(EVIDENCE_DIR, { recursive: true });
  writeFileSync(`${EVIDENCE_DIR}/${name}`, content);
}

function assertIncludes(xml, text, label) {
  if (!xml.includes(text)) throw new Error(`${label} missing "${text}"`);
}

function assertNoAppVisibleText(xml) {
  const visibleAppTexts = [...xml.matchAll(/<node\b[^>]*package="com\.orbmobile"[^>]*text="([^"]+)"/g)]
    .map((match) => match[1])
    .filter((text) => text.length > 0);
  if (visibleAppTexts.length > 0) {
    throw new Error(`Expected orb-only UI, found visible app text: ${visibleAppTexts.join(', ')}`);
  }
}

function assertNoAppControls(xml) {
  const clickableNodes = [...xml.matchAll(/<node\b[^>]*package="com\.orbmobile"[^>]*clickable="true"/g)];
  if (clickableNodes.length > 0) throw new Error('Expected orb-only UI, found clickable app controls');
}

function parseElapsedMs(logs, marker) {
  const match = logs.match(new RegExp(`${marker}[^\\n]*elapsed_ms=(\\d+)`));
  if (!match) throw new Error(`Missing ${marker} elapsed_ms log`);
  return Number.parseInt(match[1], 10);
}

function assertBudget(label, observedMs, budgetMs) {
  if (!Number.isFinite(observedMs) || observedMs > budgetMs) {
    throw new Error(`${label} exceeded budget: ${observedMs}ms > ${budgetMs}ms`);
  }
}

function budgetStatus(label, observedMs, budgetMs) {
  const status = Number.isFinite(observedMs) && observedMs <= budgetMs ? 'pass' : 'fail';
  return `${label}=${status} observed_ms=${observedMs} budget_ms=${budgetMs}`;
}

async function main() {
  run('npm', ['run', 'android:build:check'], { stdio: 'inherit', timeout: 180_000 });
  const startedEmulator = await ensureDevice();

  try {
    run('adb', ['install', '-r', APK], { timeout: 60_000 });
    run('adb', ['shell', 'am', 'force-stop', PACKAGE]);
    tryRun('adb', ['logcat', '-c']);
    run('adb', ['shell', 'am', 'start', '-n', ACTIVITY]);

    const orbXml = await waitUntil('orb-only UI', 60_000, () => {
      const xml = dumpUi('focus-orb-android-orb-only.xml');
      return xml.includes('Focus Orb') ? xml : null;
    });
    assertIncludes(orbXml, 'Focus Orb ', 'orb-only screen');
    assertNoAppVisibleText(orbXml);
    assertNoAppControls(orbXml);
    writeEvidence('focus-orb-android-orb-only.xml', orbXml);
    screenshot('focus-orb-android-orb-only');
    copyFileSync('/tmp/focus-orb-android-orb-only.png', `${EVIDENCE_DIR}/focus-orb-android-orb-only.png`);

    const speechLog = await waitUntil('native TTS speech log', 45_000, () => {
      const logs = tryRun('sh', [
        '-lc',
        "adb logcat -d -s OrbPresence:I OrbGreeting:I OrbSpeech:I ReactNativeJS:I '*:S' | rg 'OrbPresence: started|OrbPresence: activity_ready|OrbGreeting: speech_request|OrbGreeting: queued|OrbSpeech: queued|focus-orb:speak Open the first small action|focus-orb:presence-ready|focus-orb:speech-request'",
      ]).stdout;
      return logs.includes('OrbPresence: activity_ready') && logs.includes('OrbGreeting: queued') ? logs : null;
    });
    writeEvidence('focus-orb-android-logcat.txt', speechLog);
    const presenceReadyMs = parseElapsedMs(speechLog, 'OrbPresence: activity_ready');
    const speechRequestMs = parseElapsedMs(speechLog, 'OrbGreeting: speech_request');
    const budgetLines = [
      budgetStatus('presence_ready', presenceReadyMs, PRESENCE_READY_BUDGET_MS),
      budgetStatus('speech_request', speechRequestMs, SPEECH_REQUEST_BUDGET_MS),
    ];
    writeEvidence('focus-orb-android-audio-budgets.txt', `${budgetLines.join('\n')}\n`);
    if (STRICT_AUDIO_BUDGETS) {
      assertBudget('presence ready', presenceReadyMs, PRESENCE_READY_BUDGET_MS);
      assertBudget('speech request', speechRequestMs, SPEECH_REQUEST_BUDGET_MS);
    }

    const logErrors = tryRun('sh', [
      '-lc',
      "adb logcat -d -t 1000 | rg 'FATAL EXCEPTION|Unable to load|TypeError|ReferenceError|Invariant Violation|focus-orb:speech-error| E[/ ]ReactNativeJS'",
    ]).stdout;
    if (logErrors.trim()) throw new Error(`Android logcat reported runtime errors:\n${logErrors}`);

    console.log('smoke:android:e2e passed');
    console.log('screenshots: /tmp/focus-orb-android-orb-only.png');
    console.log(`budgets: ${budgetLines.join('; ')}`);
    if (!STRICT_AUDIO_BUDGETS) console.log('budget enforcement: warn-only; set ORB_STRICT_AUDIO_BUDGETS=1 to fail on audio-budget misses');
    console.log(`evidence: ${EVIDENCE_DIR}`);
  } finally {
    if (startedEmulator) {
      tryRun('adb', ['emu', 'kill']);
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
