import { mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';

mkdirSync('android/app/src/main/assets', { recursive: true });
mkdirSync('android/app/src/main/res', { recursive: true });

const result = spawnSync(
  'npx',
  [
    'react-native',
    'bundle',
    '--platform',
    'android',
    '--dev',
    'false',
    '--entry-file',
    'index.js',
    '--bundle-output',
    'android/app/src/main/assets/index.android.bundle',
    '--assets-dest',
    'android/app/src/main/res',
    '--config',
    'metro.config.js',
  ],
  { stdio: 'inherit' },
);

process.exit(result.status ?? 1);
