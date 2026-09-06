import {
  RESPONSE_ENVELOPE_VERSION,
  isKnownWidget,
  type ResponseEnvelope,
  type StepCardWidget,
} from './lld/ResponseEnvelope';
import { stimulationForState, type StimulationModel } from './presence/StimulationController';

export type CloudVoiceState = 'idle' | 'connecting' | 'listening' | 'thinking' | 'speaking' | 'error';

export type OrbVisualState = 'booting' | 'listening' | 'thinking' | 'working' | 'success' | 'paused' | 'error';
export type MicStatus = 'pending' | 'active' | 'denied' | 'error';
export type RelayStatus = 'connecting' | 'ready' | 'closed' | 'degraded';

export interface OrbLifecycleSignals {
  readonly micStatus?: MicStatus;
  readonly relayStatus?: RelayStatus;
  readonly speechStatus?: 'idle' | 'starting' | 'speaking' | 'error';
  /**
   * True only once the relay has sent a `listening_confirmed` delivery receipt for the current
   * turn (RelayClient's `onListeningConfirmed`). Local mic capture opening (`micStatus === 'active'`)
   * is not a substitute — the mic can be capturing while the relay never actually accepted the
   * session (dead socket, identity mismatch, provider outage before the receipt arrives).
   */
  readonly listeningConfirmed?: boolean;
}

export interface OrbAnimationPreset {
  readonly speed: number;
  readonly breathPeriodMs: number;
  readonly breathDepth: number;
  readonly glow: number;
  readonly colors: readonly [string, string, string];
}

export interface OrbVisualModel {
  readonly state: OrbVisualState;
  readonly opacity: number;
  readonly scale: number;
  readonly backgroundColor: string;
  readonly animation: OrbAnimationPreset;
}

export const IDLE_ENVELOPE: ResponseEnvelope = {
  v: RESPONSE_ENVELOPE_VERSION,
  session_id: 'local-idle',
  turn_id: 'local-idle-0',
  seq: 0,
  orb: {
    emotion: 'present',
    intensity: 0.45,
    animation: 'breathe',
    bed: { color: 'brown', gain_db: -18 },
  },
  session: { state: 'IDLE_PRESENT', step_index: 0, steps_total: 0 },
  meta: {
    model_version: 'none',
    prompt_version: 'none',
    source: 'cache_hit',
    latency_ms: 0,
    cost_paise: 0,
    trace_id: 'local-idle',
  },
};

export interface ScreenModel {
  readonly stateLabel: string;
  readonly stepLabel: string;
  readonly speechText: string;
  readonly voiceState: CloudVoiceState;
  readonly voiceVolume: number;
  readonly orbVolume: number;
  readonly stimulation: StimulationModel;
  readonly orbStyle: {
    readonly opacity: number;
    readonly scale: number;
    readonly backgroundColor: string;
  };
  readonly orbVisual: OrbVisualModel;
}

function stepWidget(envelope: ResponseEnvelope): StepCardWidget | null {
  const widget = envelope.widgets?.find((w) => isKnownWidget(w) && w.type === 'step_card');
  return widget && isKnownWidget(widget) && widget.type === 'step_card' ? widget : null;
}

function clampVolume(value: number): number {
  return Math.max(0, Math.min(value, 1));
}

const DEFAULT_LIFECYCLE: Required<OrbLifecycleSignals> = {
  micStatus: 'pending',
  relayStatus: 'connecting',
  speechStatus: 'idle',
  listeningConfirmed: false,
};

export const ORB_VISUAL_PRESETS: Readonly<Record<OrbVisualState, OrbAnimationPreset>> = {
  booting: {
    speed: 3.5,
    breathPeriodMs: 5_500,
    breathDepth: 0.045,
    glow: 0.28,
    colors: ['#6576e8', '#aebcff', '#f5f7ff'],
  },
  listening: {
    speed: 5.5,
    breathPeriodMs: 4_200,
    breathDepth: 0.08,
    glow: 0.42,
    colors: ['#448bd6', '#8ed5ff', '#f2fbff'],
  },
  thinking: {
    speed: 7,
    breathPeriodMs: 3_900,
    breathDepth: 0.05,
    glow: 0.36,
    colors: ['#5c50e6', '#9892f5', '#e5e6ff'],
  },
  working: {
    speed: 4.6,
    breathPeriodMs: 6_500,
    breathDepth: 0.04,
    glow: 0.3,
    colors: ['#278f89', '#7ed2c7', '#effffb'],
  },
  success: {
    speed: 4,
    breathPeriodMs: 2_400,
    breathDepth: 0.1,
    glow: 0.55,
    colors: ['#58b89f', '#b5f1d5', '#fff9e9'],
  },
  paused: {
    speed: 1.3,
    breathPeriodMs: 7_500,
    breathDepth: 0.015,
    glow: 0.12,
    colors: ['#53627f', '#8997b5', '#d9e0ef'],
  },
  error: {
    // Warm degraded signal: noticeable without alarm-red strobing.
    speed: 2.3,
    breathPeriodMs: 3_100,
    breathDepth: 0.055,
    glow: 0.3,
    colors: ['#c87552', '#f0b08a', '#fff0df'],
  },
};

function voiceState(envelope: ResponseEnvelope): CloudVoiceState {
  if (envelope.orb.animation === 'none' && envelope.session.state !== 'INTERRUPTED') return 'error';
  if (envelope.session.state === 'INTAKE') return 'listening';
  if (envelope.session.state === 'STEP_PRESENT') return 'speaking';
  if (envelope.session.state === 'WORKING') return 'thinking';
  return 'idle';
}

function cloudStyle(state: CloudVoiceState, volume: number): ScreenModel['orbStyle'] {
  if (state === 'listening') {
    return {
      opacity: 0.58 + volume * 0.22,
      scale: 1.02 - volume * 0.2,
      backgroundColor: '#79a9ff',
    };
  }
  if (state === 'speaking') {
    return {
      opacity: 0.7 + volume * 0.24,
      scale: 0.9 + volume * 0.26,
      backgroundColor: '#9f8cff',
    };
  }
  if (state === 'error') {
    return { opacity: 0.28, scale: 0.78, backgroundColor: '#f38b8b' };
  }
  if (state === 'thinking') {
    return { opacity: 0.56, scale: 0.88, backgroundColor: '#91b9f4' };
  }
  return { opacity: 0.2, scale: 0.74, backgroundColor: '#a8c7f7' };
}

export function orbVisualStateFromEnvelope(
  envelope: ResponseEnvelope,
  lifecycle: OrbLifecycleSignals = DEFAULT_LIFECYCLE,
): OrbVisualState {
  const signals = { ...DEFAULT_LIFECYCLE, ...lifecycle };
  const sessionState = envelope.session.state;

  if (signals.speechStatus === 'error') return 'error';
  if (signals.speechStatus === 'speaking') return 'thinking';
  if (signals.speechStatus === 'starting') return 'thinking';
  if (sessionState === 'INTERRUPTED') return 'paused';
  if (sessionState === 'STEP_DONE' || sessionState === 'SESSION_DONE') return 'success';
  if (
    envelope.orb.animation === 'none' ||
    signals.micStatus === 'denied' ||
    signals.micStatus === 'error' ||
    signals.relayStatus === 'degraded' ||
    signals.relayStatus === 'closed'
  ) {
    return 'error';
  }

  if (sessionState === 'IDLE_PRESENT') return 'booting';
  if (sessionState === 'INTAKE' || sessionState === 'CLARIFY') {
    // Local mic capture opening is not evidence the relay is listening — only the
    // `listening_confirmed` delivery receipt is (App.tsx wires it through RelayClient's
    // `onListeningConfirmed`). Without this, the orb showed "listening" against a relay that
    // never acknowledged the session at all.
    return signals.micStatus === 'active' && signals.listeningConfirmed ? 'listening' : 'booting';
  }
  if (sessionState === 'STEP_PRESENT') return 'thinking';
  if (sessionState === 'WORKING' || sessionState === 'CHECK_IN') return 'working';
  return 'thinking';
}

function orbVisualModel(
  state: OrbVisualState,
  volume: number,
): OrbVisualModel {
  const preset = ORB_VISUAL_PRESETS[state];
  const intensity = 0.85 + volume * 0.15;
  const styleByState: Readonly<Record<OrbVisualState, Pick<OrbVisualModel, 'opacity' | 'scale' | 'backgroundColor'>>> = {
    booting: { opacity: 0.68 * intensity, scale: 0.9, backgroundColor: preset.colors[0] },
    listening: { opacity: 0.78 * intensity, scale: 0.98, backgroundColor: preset.colors[0] },
    thinking: { opacity: 0.75 * intensity, scale: 0.94, backgroundColor: preset.colors[0] },
    working: { opacity: 0.74 * intensity, scale: 0.98, backgroundColor: preset.colors[0] },
    success: { opacity: 0.9 * intensity, scale: 1.06, backgroundColor: preset.colors[0] },
    paused: { opacity: 0.42 * intensity, scale: 0.82, backgroundColor: preset.colors[0] },
    error: { opacity: 0.62 * intensity, scale: 0.9, backgroundColor: preset.colors[0] },
  };
  return { state, animation: preset, ...styleByState[state] };
}

export function screenModelFromEnvelope(
  envelope: ResponseEnvelope,
  lifecycle: OrbLifecycleSignals = DEFAULT_LIFECYCLE,
  stimulationElapsedMs = 0,
): ScreenModel {
  const step = stepWidget(envelope);
  const state = lifecycle.speechStatus === 'error'
    ? 'error'
    : lifecycle.speechStatus === 'speaking' || lifecycle.speechStatus === 'starting'
      ? 'speaking'
      : voiceState(envelope);
  const volume = clampVolume(envelope.orb.intensity);
  const visualState = orbVisualStateFromEnvelope(envelope, lifecycle);
  const stimulation = stimulationForState(visualState, stimulationElapsedMs);
  // Keep the backend intensity meaningful while adding a bounded, state-aware novelty signal.
  const orbVolume = clampVolume(volume * 0.65 + stimulation.level * 0.35);
  return {
    stateLabel: envelope.session.state.replaceAll('_', ' '),
    stepLabel: step
      ? `${step.index}/${step.total} ${step.text}`
      : envelope.session.steps_total > 0
        ? `${envelope.session.step_index}/${envelope.session.steps_total}`
        : 'Ready when you are.',
    speechText: envelope.speech?.text ?? "I'm here. Let's keep it small.",
    voiceState: state,
    voiceVolume: volume,
    orbVolume,
    stimulation,
    orbStyle: cloudStyle(state, orbVolume),
    orbVisual: orbVisualModel(visualState, orbVolume),
  };
}
