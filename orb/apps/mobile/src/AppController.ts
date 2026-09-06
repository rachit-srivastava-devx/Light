import type { ContextPack } from './lld/ContextPack';
import type { ResponseEnvelope } from './lld/ResponseEnvelope';
import type { T0FocusSessionInput, T0FocusSessionRuntime } from './runtime/T0FocusSession';
import type { VoiceLoopController, VoiceLoopEvent } from './runtime/VoiceLoopController';
import type { SpeechPort } from './voice/SpeechPort';
import { speakFailure } from './voice/SpeechFailure';

export interface FocusOrbActions {
  readonly start: () => Promise<void>;
  readonly pause: () => Promise<void>;
}

export type SessionWarmup = (
  identity: Pick<T0FocusSessionInput, 'tenant_id' | 'user_id' | 'session_id'>,
) => Promise<ContextPack>;

export interface FocusOrbActionOverrides {
  readonly onStart?: () => void;
  readonly onPause?: () => void;
  readonly voiceLoop?: VoiceLoopController;
  readonly speaker?: SpeechPort;
  readonly warmup?: SessionWarmup;
}

export const LOCAL_MVP_GREETING = 'Good morning, Rachit';
export const LOCAL_MVP_GREETING_VOICE_ID = 'orb.warm.v1';
export const LOCAL_MVP_GREETING_EMOTION = 'upbeat';

/** The minimal JSON surface used by the mobile bootstrap. */
export interface WarmupFetcherResponse {
  readonly ok: boolean;
  readonly status: number;
  json(): Promise<unknown>;
}

export type WarmupFetcher = (
  url: string,
  init: { readonly method: 'POST'; readonly headers: Readonly<Record<string, string>>; readonly body: string },
) => Promise<WarmupFetcherResponse>;

/**
 * Load the T0 context pack before the first real turn. The identity is passed through unchanged;
 * this client does not invent or migrate identity while the local MVP is running.
 */
export async function requestSessionWarmup(
  baseUrl: string,
  identity: Pick<T0FocusSessionInput, 'tenant_id' | 'user_id' | 'session_id'>,
  fetcher: WarmupFetcher = fetch as unknown as WarmupFetcher,
): Promise<ContextPack> {
  const response = await fetcher(`${baseUrl.replace(/\/+$/, '')}/v1/session/warmup`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(identity),
  });
  if (!response.ok) throw new Error(`session warmup failed with HTTP ${response.status}`);

  const body = asRecord(await response.json(), 'warmup response');
  const profile = asRecord(body.profile, 'warmup profile');
  const userId = requiredString(profile, 'user_id');
  const recentTasks = Array.isArray(body.recent_tasks) ? body.recent_tasks : [];
  const embeddings = Array.isArray(body.embeddings) ? body.embeddings : [];
  const recent_tasks = recentTasks.map((raw, index) => {
    const task = asRecord(raw, `warmup recent_tasks[${index}]`);
    const steps = asRecord(task.steps, `warmup recent_tasks[${index}].steps`);
    const embedding = Array.isArray(task.embedding) ? task.embedding : embeddings[index];
    return {
      task_text: requiredString(task, 'task_text'),
      steps: steps as unknown as ContextPack['recent_tasks'][number]['steps'],
      embedding: Array.isArray(embedding) ? embedding.filter((value): value is number => typeof value === 'number') : [],
    };
  });
  const openLoops = Array.isArray(body.open_loops)
    ? body.open_loops.filter((value): value is string => typeof value === 'string')
    : [];
  const openSession = asRecordOrNull(body.open_session);
  const openSessionId = openSession && typeof openSession.session_id === 'string' ? openSession.session_id : null;

  return {
    profile: { user_id: userId },
    recent_tasks,
    open_loops: openLoops,
    open_session: openSessionId === null ? null : { session_id: openSessionId },
  };
}

function asRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  throw new Error(`Expected ${label} to be an object`);
}

function asRecordOrNull(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function requiredString(record: Record<string, unknown>, key: string): string {
  const value = record[key];
  if (typeof value !== 'string' || value.length === 0) throw new Error(`Expected ${key} to be a non-empty string`);
  return value;
}

export function createFocusOrbActions(
  runtime: T0FocusSessionRuntime,
  setEnvelope: (envelope: ResponseEnvelope) => void,
  overrides: FocusOrbActionOverrides = {},
): FocusOrbActions {
  const applyEnvelopes = (events: readonly VoiceLoopEvent[]) => {
    for (const event of events) {
      if (event.kind === 'envelope') setEnvelope(event.envelope);
    }
  };

  let startPromise: Promise<void> | null = null;
  let greetingSpoken = false;

  const speakLaunchGreeting = (): void => {
    if (greetingSpoken || !overrides.speaker) return;
    greetingSpoken = true;
    void Promise.resolve(
      overrides.speaker.speak(LOCAL_MVP_GREETING, LOCAL_MVP_GREETING_VOICE_ID, LOCAL_MVP_GREETING_EMOTION),
    );
  };

  const launch = async (): Promise<void> => {
    overrides.onStart?.();
    // Start the request before speaking so warmup overlaps the audible launch event. The real
    // listening loop is opened only after this promise resolves, so no first turn can race it.
    const warmupPromise = overrides.warmup?.({
      tenant_id: runtimeIdentity(runtime).tenant_id,
      user_id: runtimeIdentity(runtime).user_id,
      session_id: runtimeIdentity(runtime).session_id,
    });
    speakLaunchGreeting();
    try {
      const contextPack = await warmupPromise;
      if (contextPack !== undefined) runtime.setContextPack?.(contextPack);
    } catch (error) {
      console.error(`focus-orb:warmup-failed ${error instanceof Error ? error.message : String(error)}`);
      await speakFailure(overrides.speaker, 'provider_failure');
    }

    if (overrides.voiceLoop) {
      applyEnvelopes(await overrides.voiceLoop.start());
      return;
    }
    setEnvelope(await runtime.start());
  };

  return {
    start() {
      startPromise ??= launch();
      return startPromise;
    },
    async pause() {
      overrides.onPause?.();
      if (overrides.voiceLoop) {
        applyEnvelopes(await overrides.voiceLoop.pause());
        return;
      }
      setEnvelope(await runtime.pause());
    },
  };
}

function runtimeIdentity(runtime: T0FocusSessionRuntime): Pick<T0FocusSessionInput, 'tenant_id' | 'user_id' | 'session_id'> {
  // The runtime exposes identity through the trace-bearing envelopes, but launch has no envelope
  // until after warmup. T0FocusSessionRuntime keeps this accessor for the app boundary.
  return runtime.identity();
}
