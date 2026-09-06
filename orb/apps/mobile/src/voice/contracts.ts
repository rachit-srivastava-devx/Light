/**
 * Voice I/O plane contracts — the client half of the wire protocol `backend/relay-rs/src/protocol.rs`
 * defines. These two files are mirrors; a change to either is a breaking change to both.
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §1 (module map), §3 (latency budgets), §5 (pause contract).
 */

/** Client -> relay. Mirrors `ClientFrame` in relay-rs (serde tag: "type", snake_case). */
export type ClientFrame =
  | { readonly type: 'start_listening'; readonly tenant_id: string; readonly session_id: string }
  | { readonly type: 'end_of_turn'; readonly tenant_id: string; readonly session_id: string }
  | {
      readonly type: 'speak';
      readonly tenant_id: string;
      readonly session_id: string;
      readonly text: string;
      readonly voice_id: string;
      readonly emotion: string;
    }
  | { readonly type: 'barge_in'; readonly tenant_id: string; readonly session_id: string }
  | { readonly type: 'pause'; readonly tenant_id: string; readonly session_id: string };

/** Relay -> client. Mirrors `ServerFrame`. */
export type ServerFrame =
  | {
      /** Delivery receipt for `start_listening`: the relay accepted the session and opened the
       * STT stream. The client's "listening" UI state must gate on this, not on local mic-open
       * alone — a mic can be capturing with nothing actually listening on the other end. */
      readonly type: 'listening_confirmed';
      readonly tenant_id: string;
      readonly session_id: string;
    }
  | {
      readonly type: 'transcript';
      readonly tenant_id: string;
      readonly session_id: string;
      readonly text: string;
      readonly is_final: boolean;
    }
  | { readonly type: 'speech_starting'; readonly tenant_id: string; readonly session_id: string }
  | { readonly type: 'speech_complete'; readonly tenant_id: string; readonly session_id: string }
  | { readonly type: 'closing'; readonly tenant_id: string; readonly session_id: string; readonly reason: CloseReason }
  | {
      /**
       * §5 audio-bed invariant — mirrors relay-rs's `ServerFrame::AudioBedFallback`
       * (`backend/relay-rs/src/protocol.rs`). Sent when the voice provider fails but the session
       * stays alive (`session.rs`'s `Phase::Degraded`): unlike `closing`, the socket is NOT closed.
       * The client must keep the audio bed running and announce the outage once, not tear the
       * session down as though this were a `closing` frame.
       */
      readonly type: 'audio_bed_fallback';
      readonly tenant_id: string;
      readonly session_id: string;
    };

/**
 * §5 — these must stay distinct on the client too: a `user_pause` means silence the user asked
 * for (the bed stops, correctly); a `provider_failure` means the bed keeps running and the orb
 * says so once. Collapsing them would turn an outage into a silent app.
 */
export type CloseReason = 'user_pause' | 'provider_failure';

/** §3 — user-stops-speaking to orb-first-audio. */
export const DETERMINISTIC_TURN_P50_MS = 250;
export const CONVERSATIONAL_TURN_P50_MS = 600;
export const CONVERSATIONAL_TURN_P99_MS = 1_200;
/** §3 — zero think-time silence: audio within this budget, always. */
export const THINK_TIME_COVER_BUDGET_MS = 300;
/** §3 — barge-in must yield the floor within this budget. */
export const BARGE_IN_YIELD_BUDGET_MS = 100;
/** §3 — minimum speech duration distinguishing a real barge-in from a backchannel. */
export const BARGE_IN_MIN_SPEECH_MS = 200;
/** §1 — backchannels fire on mid-utterance pauses in this window. */
export const BACKCHANNEL_PAUSE_MIN_MS = 250;
export const BACKCHANNEL_PAUSE_MAX_MS = 600;
/** §1 — at most this many backchannels per utterance, never consecutive. */
export const BACKCHANNEL_MAX_PER_UTTERANCE = 2;
/** §1 — no backchannel in the first seconds of an utterance (it reads as interrupting). */
export const BACKCHANNEL_SUPPRESS_FIRST_MS = 3_000;
/** §1 — semantic endpointer: a complete thought endpoints after this pause. */
export const ENDPOINT_COMPLETE_PAUSE_MS = 150;
/** §1 — relay socket closes after roughly this much post-endpoint quiet. */
export const VAD_POST_ENDPOINT_CLOSE_MS = 800;
/** Client-side VAD needs a short sustained onset so clicks and breaths do not open billing. */
export const VAD_ONSET_MIN_MS = 120;
/** Speech probability thresholds use hysteresis so one noisy frame does not flap the socket. */
export const VAD_SPEECH_PROBABILITY = 0.6;
export const VAD_SILENCE_PROBABILITY = 0.35;
/** §1 — hard cap on listening through an incomplete thought. */
export const ENDPOINT_INCOMPLETE_MAX_MS = 30_000;
