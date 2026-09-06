import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

const registry = (p: string) =>
  fileURLToPath(new URL(`../../../registry/services/${p}`, import.meta.url));

export default defineConfig({
  resolve: {
    // Mirrors backend/gateway-sidecar/tsconfig.json's `paths`. The registry packages ship
    // TypeScript sources with no build step and their `exports` maps point at .js files that
    // don't exist on disk, so both the typechecker and the test runner have to be told where the
    // real sources are. Keep these two lists in sync — a drift shows up as tsc-green/vitest-red.
    alias: [
      {
        find: /^@pe\/voice-realtime\/adapters\/(.*)$/,
        replacement: registry('voice-realtime/src/voice-realtime/adapters/$1/index.ts'),
      },
      {
        find: '@pe/voice-realtime',
        replacement: registry('voice-realtime/src/voice-realtime/contracts/index.ts'),
      },
      {
        find: /^@pe\/llm-gateway\/adapters\/(.*)$/,
        replacement: registry('llm-gateway/src/llm-gateway/adapters/$1/index.ts'),
      },
      {
        find: '@pe/llm-gateway',
        replacement: registry('llm-gateway/src/llm-gateway/contracts/index.ts'),
      },
    ],
  },
  test: {
    include: [
      'apps/mobile/**/*.test.ts',
      'apps/mobile/**/*.test.tsx',
      'backend/gateway-sidecar/**/*.test.ts',
      'backend/voice-provider-sidecar/**/*.test.ts',
    ],
    exclude: ['**/node_modules/**'],
    // voice-provider-sidecar's ORB_STT_PROVIDER/ORB_TTS_PROVIDER have no implicit default (a
    // missing var is a typed startup error, not a silent fake fallback — see config.ts). Tests
    // that import src/index.ts (its module-scope `createSttBackendFromEnv()` /
    // `createTtsBackendFromEnv()` calls run at import time) need an explicit opt-in the same way
    // a real deployment would provide one, so declare it here rather than relying on an implicit
    // default that no longer exists.
    env: {
      ORB_STT_PROVIDER: 'fake',
      ORB_TTS_PROVIDER: 'fake',
    },
  },
});
