export interface SpeechPort {
  readonly speak: (text: string, voiceId: string, emotion: string) => Promise<void> | void;
  readonly stop?: () => Promise<void> | void;
}

interface NativeSpeechModule {
  readonly speak: (text: string) => Promise<void>;
  readonly stop: () => Promise<void>;
}

function loadNativeSpeech(): NativeSpeechModule {
  const { NativeModules } = require('react-native') as {
    readonly NativeModules?: { readonly OrbSpeech?: NativeSpeechModule };
  };
  const nativeSpeech = NativeModules?.OrbSpeech;
  if (!nativeSpeech) throw new Error('NativeModules.OrbSpeech is not registered');
  return nativeSpeech;
}

export function createNativeSpeechPort(): SpeechPort {
  return {
    async speak(text, voiceId, emotion) {
      void voiceId;
      void emotion;
      try {
        await loadNativeSpeech().speak(text);
        console.info(`focus-orb:speak ${text}`);
      } catch (error) {
        console.error(`focus-orb:speech-error ${error instanceof Error ? error.message : String(error)}`);
      }
    },
    async stop() {
      await loadNativeSpeech().stop();
    },
  };
}
