import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';

function run(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8', timeout: 5_000 });
  return {
    ok: result.status === 0 && !result.error,
    stdout: result.stdout.trim(),
    stderr: result.error?.message ?? result.stderr.trim(),
  };
}

function commandExists(command) {
  return run('sh', ['-lc', `command -v ${command}`]);
}

function check(name, ok, detail) {
  return { name, ok, detail };
}

const rnConfig = run('npx', ['react-native', 'config']);
const simctl = run('xcrun', ['simctl', 'list', 'devices']);
const defaultAndroidSdk = '/opt/homebrew/share/android-commandlinetools';
const androidSdk = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT || defaultAndroidSdk;
let config = {};
try {
  config = JSON.parse(rnConfig.stdout || '{}');
} catch {
  config = {};
}

const checks = [
  check('react-native config parses', rnConfig.ok, rnConfig.stderr || 'config parsed'),
  check('ios host discovered', Boolean(config.project?.ios), config.project?.ios?.sourceDir ?? 'missing'),
  check('android host discovered', Boolean(config.project?.android), config.project?.android?.sourceDir ?? 'missing'),
  check('xcodebuild installed', commandExists('xcodebuild').ok, commandExists('xcodebuild').stdout || 'missing'),
  check('cocoapods installed', commandExists('pod').ok, commandExists('pod').stdout || 'missing pod command'),
  check(
    'ios workspace present',
    existsSync('ios/OrbMobile.xcworkspace/contents.xcworkspacedata'),
    'ios/OrbMobile.xcworkspace',
  ),
  check('ios pods installed', existsSync('ios/Pods') && existsSync('ios/Podfile.lock'), 'ios/Pods + Podfile.lock'),
  check(
    'coresimulator healthy',
    simctl.ok,
    simctl.stderr || simctl.stdout.split('\n')[0] || 'xcrun simctl list devices',
  ),
  check(
    'android sdk configured',
    existsSync(androidSdk) || existsSync('android/local.properties'),
    existsSync(androidSdk) ? androidSdk : 'missing ANDROID_HOME/ANDROID_SDK_ROOT/android/local.properties',
  ),
  check('gradle wrapper present', existsSync('android/gradlew'), 'android/gradlew'),
  check('ios podfile present', existsSync('ios/Podfile'), 'ios/Podfile'),
];

for (const item of checks) {
  console.log(`${item.ok ? 'PASS' : 'FAIL'} ${item.name}: ${item.detail}`);
}

if (checks.some((item) => !item.ok)) {
  process.exitCode = 1;
}
