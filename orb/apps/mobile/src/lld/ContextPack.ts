/**
 * The in-memory context pack loaded at app-open (`docs/BUILD-DIGEST.md` §2).
 *
 * Retrieval is lexical-first, cosine-second, and there is deliberately **no vector database**: §7
 * rejects one at this scale (≤500 vectors/user), and the revisit trigger is >5,000 vectors/user or
 * cross-user retrieval. A linear scan over 50 int8 vectors is microseconds — a DB here would add
 * an operational dependency to save nothing.
 *
 * Pure: the caller supplies the pack and the query. No I/O, no clock.
 */

import type { AtomizerOutput } from '../shared/atomizer-schema';

/** §2's ~47–60KB pack. */
export interface ContextPack {
  readonly profile: { readonly user_id: string };
  readonly recent_tasks: readonly RecentTask[];
  readonly open_loops: readonly string[];
  readonly open_session: { readonly session_id: string } | null;
}

export interface RecentTask {
  readonly task_text: string;
  /** The steps this task produced last time, reusable verbatim on a lexical hit. */
  readonly steps: AtomizerOutput;
  /** 384-dim int8, quantised (§2). Length is not enforced here — see `cosineInt8`. */
  readonly embedding: readonly number[];
}

/** §2 — a lexical hit at or above this is reused verbatim, no model call at all. */
export const LEXICAL_REUSE_THRESHOLD = 0.85;
/** §2 — a cosine hit at or above this seeds the atomizer with the prior steps. */
export const COSINE_SEED_THRESHOLD = 0.8;

export type Retrieval =
  | { readonly kind: 'reuse'; readonly task: RecentTask; readonly score: number }
  | { readonly kind: 'seed'; readonly task: RecentTask; readonly score: number }
  | { readonly kind: 'cold' };

function tokens(text: string): Set<string> {
  return new Set(
    text
      .toLowerCase()
      .replace(/[^a-z0-9\s]/g, ' ')
      .split(/\s+/)
      .filter((t) => t.length > 0),
  );
}

/** Jaccard overlap. Symmetric and bounded 0..1, so the threshold means the same thing both ways. */
export function lexicalScore(a: string, b: string): number {
  const ta = tokens(a);
  const tb = tokens(b);
  if (ta.size === 0 || tb.size === 0) return 0;
  let intersection = 0;
  for (const t of ta) if (tb.has(t)) intersection += 1;
  const union = ta.size + tb.size - intersection;
  return union === 0 ? 0 : intersection / union;
}

/**
 * Cosine over quantised vectors. Returns 0 for mismatched or zero-magnitude vectors rather than
 * NaN — a corrupt embedding must degrade to "no match" (cold atomize), never poison the ranking
 * with NaN, which compares false against every threshold and would silently disable retrieval.
 */
export function cosineInt8(a: readonly number[], b: readonly number[]): number {
  if (a.length === 0 || a.length !== b.length) return 0;
  let dot = 0;
  let magA = 0;
  let magB = 0;
  for (let i = 0; i < a.length; i++) {
    const x = a[i] as number;
    const y = b[i] as number;
    dot += x * y;
    magA += x * x;
    magB += y * y;
  }
  if (magA === 0 || magB === 0) return 0;
  return dot / (Math.sqrt(magA) * Math.sqrt(magB));
}

/**
 * Lexical first (§2): an exact-ish repeat of a task the user has done before should cost zero
 * tokens and zero latency, which matters more than a marginally better semantic match.
 */
export function retrieve(
  pack: ContextPack,
  queryText: string,
  queryEmbedding: readonly number[],
): Retrieval {
  let bestLexical: { task: RecentTask; score: number } | null = null;
  for (const task of pack.recent_tasks) {
    const score = lexicalScore(queryText, task.task_text);
    if (bestLexical === null || score > bestLexical.score) bestLexical = { task, score };
  }
  if (bestLexical !== null && bestLexical.score >= LEXICAL_REUSE_THRESHOLD) {
    return { kind: 'reuse', task: bestLexical.task, score: bestLexical.score };
  }

  let bestCosine: { task: RecentTask; score: number } | null = null;
  for (const task of pack.recent_tasks) {
    const score = cosineInt8(queryEmbedding, task.embedding);
    if (bestCosine === null || score > bestCosine.score) bestCosine = { task, score };
  }
  if (bestCosine !== null && bestCosine.score >= COSINE_SEED_THRESHOLD) {
    return { kind: 'seed', task: bestCosine.task, score: bestCosine.score };
  }

  return { kind: 'cold' };
}
