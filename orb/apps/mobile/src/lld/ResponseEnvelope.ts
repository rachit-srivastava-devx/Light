/**
 * The versioned response envelope — the one wire contract every plane writes into and the UI reads.
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §2 ("Response envelope"), including its three hard rules:
 *   1. clients must ignore unknown fields and unknown widget types;
 *   2. `orb` is always present;
 *   3. `meta.source ∈ {cache_hit, reuse, model}` is mandatory.
 * §8 puts this schema in Tier 0 (nothing depends on it being built, everything depends on its shape).
 */

import type { SessionState } from '../session/contracts';

/** §2 — `"v": 1`. Bump only with a migration note in `docs/adr/`; readers must reject other values. */
export const RESPONSE_ENVELOPE_VERSION = 1 as const;

/**
 * SPEC GAP (deliberate): §2 names one emotion (`celebratory`) and one animation (`swell`). The closed
 * sets are the state→(emotion, rate, energy) lookup table owned by `lld/ProsodyDirector.ts` (§1),
 * including its structurally-excluded shame-adjacent register (INV7). These aliases exist so that
 * narrowing them to a union later is a one-line change here, not a rename across every worktree.
 */
export type OrbEmotion = string;
export type OrbAnimation = string;

/** §1 `TtsProxy.ts` — the three providers Phase 1 fans text to. */
export type TtsProvider = 'fish' | 'sarvam' | 'cartesia';

/**
 * §5 degrade ladder step 3 ("premium TTS for novel text → cached premium phrases only") — audio is
 * either a pre-rendered phrase from the manifest or synthesized now from `speech.text`. Only the
 * cached branch has a `phrase_id`, so the discriminant is load-bearing.
 */
export type SpeechAudio =
  | { readonly mode: 'cached'; readonly phrase_id: string }
  | { readonly mode: 'novel' };

export interface SpeechDirective {
  readonly text: string;
  readonly audio: SpeechAudio;
  /** §1 `VoiceRouter.ts` — pinned per session by `selectVoice(region, locale)`, never re-picked mid-session. */
  readonly voice: { readonly provider: TtsProvider; readonly voice_id: string };
}

/** §1 `ColorMorph.ts` — the bed sweeps between exactly these two colours as an ambient status channel. */
export type BedColor = 'brown' | 'pink';

export interface OrbDirective {
  readonly emotion: OrbEmotion;
  /** 0..1. Not a count — dimensionless intensity, float is correct here. */
  readonly intensity: number;
  readonly animation: OrbAnimation;
  /** §2 — the bed is reported on every envelope because INV1 makes its state part of the contract. */
  readonly bed: { readonly color: BedColor; readonly gain_db: number };
}

/** §2 — the two widget types the envelope example defines. Extend by adding a member *and* a literal. */
export const KNOWN_WIDGET_TYPES = ['step_card', 'progress'] as const;
export type KnownWidgetType = (typeof KNOWN_WIDGET_TYPES)[number];

export interface StepCardWidget {
  readonly type: 'step_card';
  /** 1-based, matches `SessionSnapshot.step_index`. Integer (AGENTS.md invariant 6). */
  readonly index: number;
  /** Integer, 1..12 (§2 `steps_total`). */
  readonly total: number;
  /** The validated step text, passed by reference from the step gate — never re-generated (INV2). */
  readonly text: string;
}

export interface ProgressWidget {
  readonly type: 'progress';
  /** Integer count of completed steps. */
  readonly done: number;
  /** Integer, 1..12. */
  readonly total: number;
}

export type KnownWidget = StepCardWidget | ProgressWidget;

/**
 * The must-ignore-unknown escape hatch (§2): a newer relay may emit widget types this client build
 * has never heard of, and the client must render what it knows and drop the rest rather than fail
 * the turn. Renderers therefore narrow with `isKnownWidget` — `type` alone cannot discriminate,
 * because TypeScript cannot subtract literals from `string`.
 */
export interface UnknownWidget {
  readonly type: string;
  readonly [key: string]: unknown;
}

export type Widget = KnownWidget | UnknownWidget;

/**
 * The only executable line in this file: pure schema knowledge about the union declared directly
 * above it, with no product behaviour. Every widget renderer must gate on it.
 */
export function isKnownWidget(widget: Widget): widget is KnownWidget {
  return (KNOWN_WIDGET_TYPES as readonly string[]).includes(widget.type);
}

/** §2 — the envelope's `session` block; the projection of `SessionSnapshot` the wire carries. */
export interface SessionBlock {
  readonly state: SessionState;
  /** 1-based; `0` before any step is presented. Integer. */
  readonly step_index: number;
  /** Integer, 1..12 once atomized; `0` before. */
  readonly steps_total: number;
}

/**
 * §2 — mandatory on every envelope. `cache_hit` = phrase manifest / cached audio, `reuse` = context
 * pack match (§2 lexical ≥0.85 or cosine ≥0.80), `model` = a real LLM call happened. The eval and
 * cost gates (§6 cost-replay-200) read this field to attribute spend, so it is not advisory.
 */
export type ResponseSource = 'cache_hit' | 'reuse' | 'model';

export interface EnvelopeMeta {
  /** §4 — pinned model version string; upgrades are scheduled and eval-gated, never silent. */
  readonly model_version: string;
  /** §4 — pinned prompt version (git + stamp), e.g. `atomizer.v4`. */
  readonly prompt_version: string;
  readonly source: ResponseSource;
  /** Integer milliseconds; feeds the §3 latency gates (deterministic p50 ≤250ms / conversational p50 ≤600ms). */
  readonly latency_ms: number;
  /**
   * Money as an integer count of paise — never a float and never rupees (AGENTS.md invariant 6).
   * `0` for `cache_hit` / `reuse`.
   */
  readonly cost_paise: number;
  readonly trace_id: string;
}

export interface ResponseEnvelope {
  readonly v: typeof RESPONSE_ENVELOPE_VERSION;
  readonly session_id: string;
  readonly turn_id: string;
  /** Monotonic per session, integer (§2 example `"seq": 7`). */
  readonly seq: number;
  /** Optional: a turn may move the orb and the session without speaking (e.g. a silent step advance). */
  readonly speech?: SpeechDirective;
  /** §2 — "`orb` always present". Presence > intelligence > richness (§5). */
  readonly orb: OrbDirective;
  /** Absent and empty are the same thing to a client; unknown members are dropped, not fatal. */
  readonly widgets?: readonly Widget[];
  readonly session: SessionBlock;
  readonly meta: EnvelopeMeta;
}
