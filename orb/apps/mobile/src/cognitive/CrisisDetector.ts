/**
 * Crisis / self-harm detection — a deterministic, offline safety short-circuit.
 *
 * `docs/BUILD-DIGEST.md` does not yet own a section for this (it is a named Phase 1 gap in
 * `blueprints/ADHD-Focus-Orb-L8-Deep-Dive`). This is the first real version, not a complete
 * clinical solution: a conservative, rule-based (regex) scan over a transcript for clear,
 * unambiguous self-harm/suicide crisis language.
 *
 * Pure function, zero I/O, zero wall-clock, zero model call (AGENTS.md determinism invariant /
 * §4 of the digest): this must be callable from a hot path and unit-testable bit-for-bit.
 *
 * ── Design reasoning for the exact patterns chosen ──────────────────────────────────────────
 *
 * This product's whole audience is ADHD users venting frustration at tasks — "this is killing
 * me", "I want to die, this essay is never going to end", "I could just kill this todo list" are
 * completely normal, non-clinical hyperbole in this domain. A detector tuned for recall would
 * false-positive constantly on exactly the population it serves, and every false positive means
 * refusing to help with the task AND breaking the deterministic control-path promise for no
 * reason. So this is deliberately precision-first: it only fires on phrasing that is not
 * plausible as task hyperbole, specifically:
 *
 *   1. First-person suicidal intent tied to the person, not a task/object — "I want to kill
 *      myself", "I'm going to end my life", "I don't want to live anymore". The reflexive
 *      pronoun (myself/my life) is what rules out "this essay is killing me" — hyperbole always
 *      attaches the verb to the task ("this is killing me"), never a reflexive self-target
 *      ("I want to kill myself"). This is the single highest-value discriminator: requiring a
 *      first-person reflexive target on a threat-of-lethal-action verb is what most cheap
 *      keyword detectors skip, and it is exactly what causes the false-positive rate to explode.
 *   2. Explicit means/plan statements — "I have pills to overdose", "I'm going to cut myself",
 *      naming a method alongside self-harm intent. Task hyperbole never mentions a method.
 *   3. Direct suicidal-ideation phrasing with no task object in the same clause — "I want to
 *      kill myself", "I want to die" said standalone. Note "I want to die" bare IS included
 *      (it's genuinely ambiguous with hyperbole like "I want to die, this essay is never going
 *      to end") — but only when it is NOT immediately followed by a task/reason clause that
 *      reads as hyperbole. Given the safety asymmetry (a missed crisis is catastrophic; an extra
 *      "are you okay" is mildly annoying and this message explicitly does not gatekeep further
 *      help-seeking), bare unqualified "I want to die" / "I don't want to be alive" IS treated as
 *      a positive. What is excluded is the task-attached form ("X is killing me", "kill this
 *      task/list/essay").
 *
 * Deliberately NOT matched (kept as true negatives, see the test file):
 *   - "this task is killing me", "this deadline is killing me" — task-attached "kill/die" verbs.
 *   - "I could kill this todo list" / "I'm dying to finish this" — non-reflexive or idiomatic.
 *   - "I'm so done with this" / "I give up" — frustration/avoidance language, not crisis.
 *
 * This is intentionally conservative (high precision, imperfect recall) per the task brief: err
 * toward not triggering on normal ADHD frustration language. It is a first real version of a
 * safety net, not a clinical triage tool — a human reviews the response copy before any real
 * user sees it, and this module does not claim to catch every crisis utterance.
 */

/**
 * First-person reflexive self-harm/suicide intent or means statements. Each pattern requires an
 * explicit self-referential target (myself, my life, me — not a task object) alongside a lethal
 * action verb, so "this essay is killing me" cannot match but "I want to kill myself" can.
 */
const REFLEXIVE_INTENT_PATTERNS: readonly RegExp[] = [
  // "I want to / am going to / will kill myself"
  /\bi\s*('m|\bam)?\s*(want to|wanna|am going to|'m going to|will|plan to|going to)\s+(kill|end)\s+(myself|my life)\b/i,
  // "kill myself" / "end my life" / "end it all" without the leading intent verb (e.g. "thinking about killing myself")
  /\b(kill(ing)?\s+myself|end(ing)?\s+my\s+life|end(ing)?\s+it\s+all)\b/i,
  // Explicit means/plan: "I have pills to overdose", "going to cut myself", "going to overdose"
  /\bi\s*('m|\bam)?\s*(going to|about to|planning to)\s+(overdose|cut myself|hang myself|jump off|shoot myself)\b/i,
  /\bi\s+have\s+(pills|a gun|a knife|rope)\s+to\s+(overdose|kill myself|end (it|my life))\b/i,
  // "I don't want to live anymore" / "I don't want to be alive"
  /\bi\s+don'?t\s+want\s+to\s+(live|be alive)\s+(anymore|any\s*more)?\b/i,
  // Bare "I want to die" / "I don't want to live" with no trailing task/reason clause attached by
  // a comma+task pattern is handled below (needs the negative lookahead against hyperbole).
];

/**
 * Bare suicidal-ideation phrasing ("I want to die") is ambiguous with hyperbole ("I want to die,
 * this essay is never going to end"). We match it only when NOT immediately followed by a comma
 * or conjunction introducing a task/reason clause — that trailing clause is the hyperbole tell.
 */
const BARE_IDEATION_PATTERN = /\bi\s+(want to|wish i could)\s+die\b(?!\s*[,]?\s*(this|that|my|because|from|over|doing|trying))/i;

/**
 * Explicit suicide-plan/attempt disclosures that don't fit the reflexive-verb shape above but are
 * unambiguous, e.g. "I'm going to end my life tonight", "I tried to kill myself".
 */
const DISCLOSURE_PATTERNS: readonly RegExp[] = [
  /\bi\s+tried\s+to\s+(kill myself|end my life|end it all)\b/i,
  /\bi'?m\s+suicidal\b/i,
  /\bi\s+have\s+a\s+suicide\s+plan\b/i,
];

const ALL_PATTERNS: readonly RegExp[] = [
  ...REFLEXIVE_INTENT_PATTERNS,
  BARE_IDEATION_PATTERN,
  ...DISCLOSURE_PATTERNS,
];

/**
 * Pure, deterministic scan for clear, unambiguous self-harm/suicide crisis language. No wall
 * clock, no I/O, no model call. Returns true only on high-confidence matches (see module doc for
 * the precision-first reasoning); false does not mean "definitely safe", it means "not confident
 * enough to short-circuit the session" — this is a first real version of the safety net, not a
 * complete clinical solution.
 */
export function detectCrisis(transcript: string): boolean {
  if (transcript.trim().length === 0) return false;
  return ALL_PATTERNS.some((pattern) => pattern.test(transcript));
}
