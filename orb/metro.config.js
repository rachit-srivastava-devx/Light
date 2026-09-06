import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { getDefaultConfig, mergeConfig } = require('@react-native/metro-config');

const projectRoot = dirname(fileURLToPath(import.meta.url));
const mobileRoot = resolve(projectRoot, 'apps/mobile');
const registryRoot = resolve(projectRoot, '../../..', 'registry');

const workspaceSourceEntrypoints = new Map([
  [
    '@pe/realtime-voice',
    resolve(registryRoot, 'features/realtime-voice/src/realtime-voice/contracts/index.ts'),
  ],
  [
    '@pe/realtime-voice/adapters/memory',
    resolve(registryRoot, 'features/realtime-voice/src/realtime-voice/adapters/memory/index.ts'),
  ],
  [
    '@pe/voice-realtime',
    resolve(registryRoot, 'services/voice-realtime/src/voice-realtime/contracts/index.ts'),
  ],
  [
    '@pe/voice-realtime/adapters/memory',
    resolve(registryRoot, 'services/voice-realtime/src/voice-realtime/adapters/memory/index.ts'),
  ],
]);

export default mergeConfig(getDefaultConfig(projectRoot), {
  projectRoot,
  watchFolders: [mobileRoot, registryRoot],
  resolver: {
    nodeModulesPaths: [resolve(projectRoot, 'node_modules'), resolve(mobileRoot, 'node_modules')],
    resolveRequest(context, moduleName, platform) {
      const workspaceEntrypoint = workspaceSourceEntrypoints.get(moduleName);
      if (workspaceEntrypoint) return { type: 'sourceFile', filePath: workspaceEntrypoint };

      if (context.originModulePath.startsWith(registryRoot) && moduleName.endsWith('.js')) {
        try {
          return context.resolveRequest(context, moduleName.slice(0, -3), platform);
        } catch {
          // Fall through to Metro's normal error for non-TypeScript imports.
        }
      }

      return context.resolveRequest(context, moduleName, platform);
    },
  },
});
