/**
 * Deterministic client-side VAD gate (`docs/BUILD-DIGEST.md` §1/§3).
 *
 * This is not native mic capture. It is the pure state machine that consumes classified audio
 * frames and decides when the relay socket should open, stay open, or close after endpoint. The
 * native layer can be swapped later without changing the product contract.
 */

import {
  ENDPOINT_INCOMPLETE_MAX_MS,
  VAD_ONSET_MIN_MS,
  VAD_POST_ENDPOINT_CLOSE_MS,
  VAD_SILENCE_PROBABILITY,
  VAD_SPEECH_PROBABILITY,
} from './contracts';

/**
 * Which classifier produced a frame's `speech_probability`. Carried on the frame itself (rather
 * than only logged) so the fallback is detectable by any downstream reader — telemetry, a test, a
 * future observability consumer — without needing a side-channel log line per frame. `'silero-onnx'`
 * is declared here so the port has a name to swap to; it is not implemented in this tree today (see
 * `docs/adr/0013-vad-and-endpointing-architecture.md` — the dependency it needs is not installed).
 */
export type VadSource = 'heuristic-rms' | 'heuristic-multifeature' | 'silero-onnx';

export interface VadFrame {
  readonly at_ms: number;
  /** 0..1 probability from the native/WebRTC classifier. */
  readonly speech_probability: number;
  /** Which classifier produced `speech_probability`. Optional so existing callers/tests are unaffected. */
  readonly vad_source?: VadSource;
}

export interface VadGateState {
  readonly phase: 'idle' | 'candidate' | 'listening';
  readonly candidate_started_at_ms: number | null;
  readonly turn_started_at_ms: number | null;
  readonly last_voice_at_ms: number | null;
  /**
   * One-shot latch: has the eager pre-close signal already fired for the current run of
   * post-endpoint quiet? Optional (defaults to falsy when absent) so any pre-existing hand-built
   * state literal in a test elsewhere in the tree stays valid without this field.
   */
  readonly eager_end_of_turn_signaled?: boolean;
}

export const INITIAL_VAD_GATE_STATE: VadGateState = {
  phase: 'idle',
  candidate_started_at_ms: null,
  turn_started_at_ms: null,
  last_voice_at_ms: null,
  eager_end_of_turn_signaled: false,
};

/**
 * How long into post-endpoint quiet the gate signals "probably done" before the confirmed
 * `end_of_turn` at `VAD_POST_ENDPOINT_CLOSE_MS` (800ms). This is the audio-domain half of the
 * eager/confirmed pair described in `docs/adr/0013-vad-and-endpointing-architecture.md`: it needs
 * no STT round trip, so it is architecturally the EARLIEST "user has probably stopped" signal
 * available in this pipeline — the one a filler-covering consumer should prefer over waiting for
 * either the confirmed VAD close or a semantic endpoint on text.
 *
 * Chosen, not measured: the owner's hard budget is time-to-continuous-audio <= 250ms after the user
 * stops talking, and the modal human inter-speaker gap is ~0-200ms (Stivers et al., PNAS 2009), so
 * this fires with real margin (~100ms) before that 250ms wall rather than at some fraction of the
 * unrelated 800ms confirmed threshold. Configurable via `reduceVadGate`'s third parameter — this is
 * a product tuning knob, not a hardcoded constant a caller cannot override.
 */
export const VAD_EAGER_POST_ENDPOINT_MS = 150;

export interface VadGateConfig {
  readonly eagerPostEndpointMs?: number;
}

export type VadDecision =
  | { readonly kind: 'ignore'; readonly reason: 'invalid_frame' | 'quiet' | 'candidate_reset' }
  | { readonly kind: 'hold'; readonly reason: 'candidate' | 'voice_active' | 'post_endpoint_quiet' }
  | { readonly kind: 'start_listening' }
  /**
   * Fired at most once per post-endpoint quiet run, strictly before the confirmed `end_of_turn`.
   * Non-committal: state stays `'listening'` and audio keeps flowing exactly as it did on a plain
   * `hold`/`post_endpoint_quiet` — this is a hint for a filler-covering consumer (Track J), never a
   * turn-completion signal. See the module doc above and the ADR for the exact contract.
   */
  | { readonly kind: 'eager_end_of_turn'; readonly reason: 'post_endpoint_quiet_eager' }
  | { readonly kind: 'end_of_turn'; readonly reason: 'post_endpoint' | 'hard_cap' };

export interface VadGateResult {
  readonly state: VadGateState;
  readonly decision: VadDecision;
}

function validFrame(frame: VadFrame): boolean {
  return (
    Number.isFinite(frame.at_ms) &&
    frame.at_ms >= 0 &&
    Number.isFinite(frame.speech_probability) &&
    frame.speech_probability >= 0 &&
    frame.speech_probability <= 1
  );
}

export function reduceVadGate(state: VadGateState, frame: VadFrame, config?: VadGateConfig): VadGateResult {
  if (!validFrame(frame)) {
    return { state, decision: { kind: 'ignore', reason: 'invalid_frame' } };
  }

  if (state.phase === 'idle') {
    if (frame.speech_probability < VAD_SPEECH_PROBABILITY) {
      return { state, decision: { kind: 'ignore', reason: 'quiet' } };
    }
    return {
      state: { ...state, phase: 'candidate', candidate_started_at_ms: frame.at_ms },
      decision: { kind: 'hold', reason: 'candidate' },
    };
  }

  if (state.phase === 'candidate') {
    if (frame.speech_probability < VAD_SILENCE_PROBABILITY) {
      return { state: INITIAL_VAD_GATE_STATE, decision: { kind: 'ignore', reason: 'candidate_reset' } };
    }

    const candidateStarted = state.candidate_started_at_ms;
    if (candidateStarted === null) {
      return { state: INITIAL_VAD_GATE_STATE, decision: { kind: 'ignore', reason: 'invalid_frame' } };
    }

    if (frame.at_ms - candidateStarted < VAD_ONSET_MIN_MS) {
      return {
        state: { ...state, last_voice_at_ms: frame.at_ms },
        decision: { kind: 'hold', reason: 'candidate' },
      };
    }

    return {
      state: {
        phase: 'listening',
        candidate_started_at_ms: null,
        turn_started_at_ms: candidateStarted,
        last_voice_at_ms: frame.at_ms,
      },
      decision: { kind: 'start_listening' },
    };
  }

  const turnStarted = state.turn_started_at_ms;
  if (turnStarted === null) {
    return { state: INITIAL_VAD_GATE_STATE, decision: { kind: 'ignore', reason: 'invalid_frame' } };
  }

  if (frame.at_ms - turnStarted >= ENDPOINT_INCOMPLETE_MAX_MS) {
    return { state: INITIAL_VAD_GATE_STATE, decision: { kind: 'end_of_turn', reason: 'hard_cap' } };
  }

  if (frame.speech_probability >= VAD_SILENCE_PROBABILITY) {
    return {
      // Voice resumed: clear the eager latch so a *later* quiet run can signal eager again. Without
      // this, one eager signal per utterance would be all a long, pause-y ADHD utterance ever gets.
      state: { ...state, last_voice_at_ms: frame.at_ms, eager_end_of_turn_signaled: false },
      decision: { kind: 'hold', reason: 'voice_active' },
    };
  }

  const lastVoice = state.last_voice_at_ms ?? turnStarted;
  if (frame.at_ms - lastVoice >= VAD_POST_ENDPOINT_CLOSE_MS) {
    return { state: INITIAL_VAD_GATE_STATE, decision: { kind: 'end_of_turn', reason: 'post_endpoint' } };
  }

  const eagerMs = config?.eagerPostEndpointMs ?? VAD_EAGER_POST_ENDPOINT_MS;
  if (!state.eager_end_of_turn_signaled && frame.at_ms - lastVoice >= eagerMs) {
    return {
      state: { ...state, eager_end_of_turn_signaled: true },
      decision: { kind: 'eager_end_of_turn', reason: 'post_endpoint_quiet_eager' },
    };
  }

  return { state, decision: { kind: 'hold', reason: 'post_endpoint_quiet' } };
}
