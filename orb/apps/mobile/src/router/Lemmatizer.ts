/**
 * A tiny, deterministic morphological normalizer — the T0/interim stand-in for a real POS/lemma
 * library, so verb-set membership (`IntentClassifier.ts`'s `TASK_VERBS`) survives ordinary English
 * inflection instead of failing on every conjugated form.
 *
 * WHY THIS EXISTS (see the track report for the full vetting): a pure keyword/first-word `Set`
 * lookup is exactly the class of bug this file fixes. `TASK_VERBS.has('opened')` was `false` even
 * though `TASK_VERBS.has('open')` is `true` — "Opened the file already", "Sending the email now",
 * and "paying the electricity bill" all failed to be recognized as task utterances. Adding more
 * literal strings to `TASK_VERBS` (`'opened'`, `'sending'`, `'paying'`, ...) would have "fixed" only
 * those three instances and left the next conjugation broken — the same shape of bug as the
 * `isConversationalIntake` regex this track was sent to repair.
 *
 * INTERIM, CLEARLY LABELLED: this is NOT a real lemmatizer/POS tagger. It is a bounded suffix
 * stripper covering regular English inflection (-s/-es, -ed, -ing, plus the silent-e and
 * doubled-consonant spelling changes: "filing"->"file", "planning"->"plan"). It has no dictionary,
 * cannot resolve irregular verbs ("wrote"->"write"), and understands no language but English.
 *
 * SWAP-IN POINT: `Lemmatizer` is a plain function type so a real deterministic NLP library slots in
 * with zero call-site changes — every caller in `IntentClassifier.ts` takes an injectable
 * `Lemmatizer` defaulting to `suffixStripLemmatizer`. Recommended real implementation, in order:
 *   1. `compromise` (npm, MIT, github.com/spencermountain/compromise). Verified 2026-08-28: latest
 *      14.16.0, released within the last ~2 months, ~250k npm downloads/month, no known CVE/security
 *      advisory. Self-contained (no separate model package). Use `nlp(word).verbs().toInfinitive()`
 *      (or `.out('text')` / `.tags` for POS-gated imperative-vs-question detection) and adapt its
 *      output into this function's `readonly string[]` shape.
 *   2. `wink-nlp` (npm, MIT, github.com/winkjs/wink-nlp). Verified 2026-08-28: latest 2.4.0, ~14
 *      months old, lighter/faster, but needs a separate `wink-eng-lite-web-model` peer package.
 * Neither is installed in this repo (`node_modules` has no `compromise`/`wink-nlp`/`wink-eng-lite-*`
 * as of this change), and adding one means a `package.json` edit — outside this track's owned files
 * while Track A owns dependency repair. See the track report for the explicit STOP-and-report.
 *
 * Both candidates are deterministic, local, rule-based NLP — no network call, no model inference —
 * so swapping either in preserves the product's zero-LLM-decisions control-path invariant
 * (`docs/BUILD-DIGEST.md` §4): routing stays a pure function of transcript + state either way.
 *
 * Pure: no I/O, no randomness, no clock. Same token always returns the same candidate set.
 */

export type Lemmatizer = (token: string) => readonly string[];

const VOWELS = new Set(['a', 'e', 'i', 'o', 'u']);

/**
 * Spelling variants for a stem recovered by stripping a 2- or 3-character suffix (-ed / -ing):
 * the bare stem, the silent-e-restored form ("fil" -> "file"), and — only when the stem ends in a
 * doubled consonant — the single-consonant form ("plann" -> "plan"). Order doesn't matter; callers
 * union every candidate into a set.
 */
function spellingVariants(stem: string): readonly string[] {
  const out = [stem, `${stem}e`];
  const last = stem[stem.length - 1];
  const secondLast = stem[stem.length - 2];
  if (stem.length > 2 && last !== undefined && last === secondLast && !VOWELS.has(last)) {
    out.push(stem.slice(0, -1));
  }
  return out;
}

/**
 * The T0 default `Lemmatizer`. See the file doc comment for what this is standing in for.
 *
 *   suffixStripLemmatizer('opened')   -> ['opened', 'open', 'opene']
 *   suffixStripLemmatizer('sending')  -> ['sending', 'send', 'sende']
 *   suffixStripLemmatizer('paying')   -> ['paying', 'pay', 'paye']
 *   suffixStripLemmatizer('planning') -> ['planning', 'plann', 'planne', 'plan']
 *   suffixStripLemmatizer('taxes')    -> ['taxes', 'tax']   (not a verb; harmless over-generation)
 */
export const suffixStripLemmatizer: Lemmatizer = (rawToken: string): readonly string[] => {
  const token = rawToken.toLowerCase();
  const candidates = new Set<string>([token]);
  if (token.endsWith('ied') && token.length > 4) {
    candidates.add(`${token.slice(0, -3)}y`); // studied -> study
  }
  if (token.endsWith('ing') && token.length > 4) {
    for (const c of spellingVariants(token.slice(0, -3))) candidates.add(c);
  }
  if (token.endsWith('ed') && token.length > 3) {
    for (const c of spellingVariants(token.slice(0, -2))) candidates.add(c);
  }
  if (token.endsWith('ies') && token.length > 4) {
    candidates.add(`${token.slice(0, -3)}y`); // (kept distinct from -ing/-ed cases above)
  } else if (token.endsWith('es') && token.length > 3) {
    candidates.add(token.slice(0, -2));
  }
  if (token.endsWith('s') && !token.endsWith('ss') && token.length > 2) {
    candidates.add(token.slice(0, -1));
  }
  return [...candidates];
};
