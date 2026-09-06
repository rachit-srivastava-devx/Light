/**
 * IntentClassifier — rule-first keyword match on the 6 intent labels (`router/contracts.ts`'s
 * `IntentLabel`). Falls to a cheap LLM only when the rule-first pass is ambiguous (§2/§4: an LLM
 * may classify an ambiguous intent, but never picks a route itself).
 *
 * Source of truth: `docs/BUILD-DIGEST.md` §2 (`Intent labels: {done, next, stuck, pause, question,
 * chitchat}`) and §4 (determinism: the router and its inputs stay boxed; ambiguous-intent
 * classification is the one place an LLM is allowed in this plane, and Tier 4 owns wiring it).
 *
 * This file only implements the deterministic rule-first path. The "falls to cheap LLM when
 * ambiguous" path is a typed extension point (`AmbiguousIntentResolver`) — no LLM call is wired
 * here; that is `backend/relay-py/src/orb_relay/proxy/atomizer.py`'s / the Tier-4 crew's job (BUILD-DIGEST §8
 * Tier 4, via `@pe/llm-gateway`, C9). Calling the default resolver throws a clearly-labeled
 * not-implemented marker.
 */

import type { IntentLabel } from './contracts';
import intentRules from '../shared/intent-rules.v1.json';
import chatResponses from '../shared/chat-responses.v1.json';
import { suffixStripLemmatizer, type Lemmatizer } from './Lemmatizer';

/**
 * Rule-first keyword sets, lower-cased, checked as whole-word substring matches. First match wins.
 * Order matters: `done` is checked before `next` so "done, next one" resolves to `done` (the more
 * committing signal) rather than `next`; `stuck` is checked before `question` so "I'm stuck, what
 * do I do" resolves to `stuck` rather than `question`.
 */
const RULES = intentRules.rules as ReadonlyArray<{
  readonly label: IntentLabel;
  readonly keywords: readonly string[];
}>;
const COMPLETION_WORDS = `(${intentRules.completion_words.map(escapeRegex).join('|')})`;
const NEGATED_PREFIXES = `(${intentRules.negated_completion_prefixes.map(escapeRegex).join('|')})`;
const NEGATED_COMPLETION = new RegExp(`\\b${NEGATED_PREFIXES}\\s+${COMPLETION_WORDS}\\b`);
const QUESTION_FORM_PREFIXES = intentRules.question_form_prefixes.map((prefix) => prefix.toLowerCase());
const TASK_REQUEST_PREFIXES = intentRules.task_request_prefixes.map((prefix) => `${prefix.toLowerCase()} `);
const TASK_VERBS = new Set([
  'pay', 'open', 'send', 'write', 'call', 'book', 'organize', 'clean', 'prepare', 'review',
  'file', 'buy', 'schedule', 'email', 'finish', 'make', 'do', 'take', 'go', 'set', 'plan',
  'download', 'submit', 'reply', 'read', 'fix', 'work', 'start',
]);
const SOCIAL_UTTERANCES = chatResponses.utterances as Readonly<{
  readonly greeting: readonly string[];
  readonly presence: readonly string[];
}>;

/** Track B, versioned open-domain-intake rule data — see `intent-rules.v1.json`'s own comment. */
const OPEN_DOMAIN = intentRules.intake_open_domain_v1 as Readonly<{
  readonly teach_markers: readonly string[];
  readonly opinion_markers: readonly string[];
  readonly meta_orb_markers: readonly string[];
  readonly feeling_markers: readonly string[];
  readonly reactive_interjections: readonly string[];
  readonly ambiguous_fragment_markers: readonly string[];
  readonly hinglish_markers: readonly string[];
  readonly conversational_framing_markers: readonly string[];
}>;

function escapeRegex(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** Same word-boundary shape as `containsKeyword`'s phrase branch, exported for multi-word marker
 * lists (`OPEN_DOMAIN.*`) where lemma-matching a whole phrase makes no sense. */
function includesPhrase(normalizedLower: string, phrase: string): boolean {
  return new RegExp(`(^|[^a-z0-9'])${escapeRegex(phrase)}([^a-z0-9']|$)`, 'u').test(normalizedLower);
}

function matchesAny(normalizedLower: string, phrases: readonly string[]): boolean {
  return phrases.some((phrase) => includesPhrase(normalizedLower, phrase));
}

/**
 * Word-aware keyword match. Multi-word phrases still match as phrases, but substring false
 * positives such as `unstuck` and `however` no longer fire a route.
 *
 * Single-word keywords are also matched against each utterance token's `lemmatize` candidates, so
 * `"pay"` matches `"paying"`/`"paid"` and not just the bare form (see `Lemmatizer.ts` for why this
 * exists instead of enumerating conjugations here). Multi-word phrases skip lemma-matching
 * entirely — inflecting a whole phrase is not a thing this needs to do.
 */
function containsKeyword(
  utteranceLower: string,
  keyword: string,
  lemmatize: Lemmatizer = suffixStripLemmatizer,
): boolean {
  if (keyword === '?') return utteranceLower.includes('?');
  if (keyword.includes(' ')) {
    return new RegExp(`(^|[^a-z0-9'])${escapeRegex(keyword)}([^a-z0-9']|$)`).test(utteranceLower);
  }
  if (includesPhrase(utteranceLower, keyword)) return true;
  const tokens = utteranceLower.match(/[a-z0-9']+/g) ?? [];
  return tokens.some((token) => token !== keyword && lemmatize(token).includes(keyword));
}

function normalizeSocialUtterance(utterance: string): string {
  return utterance
    .toLowerCase()
    .replace(/[’]/gu, "'")
    .replace(/[^a-z0-9']+/gu, ' ')
    .trim()
    .replace(/\s+/gu, ' ');
}

/** Exact-match social phrases stay local; longer sentences remain eligible for task routing. */
export function classifySocialUtterance(utterance: string): 'greeting' | 'presence' | null {
  const normalized = normalizeSocialUtterance(utterance);
  if (SOCIAL_UTTERANCES.greeting.includes(normalized)) return 'greeting';
  if (SOCIAL_UTTERANCES.presence.includes(normalized)) return 'presence';
  return null;
}

/**
 * Result of the rule-first pass. `matched` is `false` only when no rule fired.
 */
export type RuleClassification =
  | { readonly matched: true; readonly label: IntentLabel }
  | { readonly matched: false; readonly candidates: readonly IntentLabel[] };

/** Pure rule-first pass: no I/O, no model call. First-rule-that-fires wins (see RULES ordering note). */
export function classifyByRule(utterance: string): RuleClassification {
  const lower = utterance.toLowerCase();
  if (classifySocialUtterance(utterance) !== null) return { matched: true, label: 'chitchat' };
  // A question-form prefix ("can I", "could I", "should I", "do I", "am I allowed to") followed
  // by "?" is unambiguously a question, even when it also contains a word that would otherwise
  // fire an earlier rule (e.g. "can I stop?" contains "stop", the `pause` keyword; "could I get
  // help with my taxes?" contains "help", the `stuck` keyword). Route straight to `question`
  // instead of falling through to those keyword rules — and, critically, instead of returning
  // `{matched: false, candidates: []}` (B5 fix): that empty-candidates shape used to short-circuit
  // BEFORE the `question` rule's own "?" keyword could ever fire, so every question-form-prefix
  // utterance fed classifyIntent's default resolver zero candidates and threw
  // AmbiguousIntentNotWiredError on ordinary question speech instead of routing question ->
  // fast_voice (BUILD-DIGEST §2/§4).
  if (lower.includes('?') && QUESTION_FORM_PREFIXES.some((prefix) => lower.trim().startsWith(prefix))) {
    return { matched: true, label: 'question' };
  }
  for (const rule of RULES) {
    if (rule.label === 'done' && NEGATED_COMPLETION.test(lower)) continue;
    if (rule.keywords.some((kw) => containsKeyword(lower, kw))) {
      return { matched: true, label: rule.label };
    }
  }
  return { matched: false, candidates: [] };
}

/**
 * Detects a new task stated during an active session instead of guessing chitchat.
 *
 * @param lemmatize injectable so a real morphological analyzer swaps in without touching call
 *   sites (see `Lemmatizer.ts`). Defaults to the T0 suffix-stripper.
 */
/**
 * Conversation CONTROL: the user steering the exchange rather than naming work.
 *
 * Live report from a real voice session (2026-08-28): *"not able to stop the conversation in
 * between, divert or have a conversation. right now its just one by one messaging not a
 * conversation completely."* Probing the router with the owner's own phrasings showed why —
 * `wait stop`, `hold on`, `no not that`, `forget it, different topic` and `can we just chat` ALL
 * classified as `'task'`, which loads `focus-companion.v1.md`, whose job is to name a smallest
 * step. So asking the orb to stop produced a task step, and so did asking it to change subject.
 *
 * Two separate causes, both fixed here: these phrasings matched nothing, and
 * `classifyIntakeUtterance`'s fall-through default was `'task'`.
 *
 * Checked BEFORE `isLikelyTaskRequest`, deliberately inverting this file's original B3 ordering
 * note. Steering the conversation outranks naming work: "stop" must never become a step, and the
 * cost of the old order was that it always did.
 */
export type ConversationControlKind = 'stop' | 'reject' | 'divert' | 'chat_invitation';

const CONTROL = intentRules.conversation_control_v1 as Readonly<{
  readonly stop_markers: readonly string[];
  readonly reject_markers: readonly string[];
  readonly divert_markers: readonly string[];
  readonly chat_invitation_markers: readonly string[];
}>;

/**
 * Which control the utterance is, or null.
 *
 * `stop` and `reject` match on the WHOLE utterance (after trailing punctuation) rather than by
 * substring, because "no" and "stop" are common inside real sentences — "no, the kitchen one" is a
 * clarification and "stop by the shop" is a task. `divert` and `chat_invitation` are phrases
 * distinctive enough to match anywhere.
 */
export function classifyConversationControl(utterance: string): ConversationControlKind | null {
  const normalized = utterance
    .toLowerCase()
    .replace(/[\u2019]/gu, "'")
    .trim()
    .replace(/\s+/gu, ' ')
    .replace(/[.,!?]+$/u, '');
  if (!normalized) return null;
  // Longest-first so "let's talk about something else" wins over the bare "something else".
  const contains = (list: readonly string[]): boolean =>
    [...list]
      .sort((a, b) => b.length - a.length)
      .some((marker) => normalized === marker || normalized.includes(marker));
  const isWhole = (list: readonly string[]): boolean =>
    list.some((marker) => normalized === marker);
  // "wait stop" / "no nope" — a short utterance built ONLY from markers is still that control, but
  // requiring every word to be a marker keeps "stop by the shop and get milk" out.
  const isAllMarkers = (list: readonly string[]): boolean => {
    const words = normalized.split(' ');
    if (words.length === 0 || words.length > 3) return false;
    return words.every((word) => list.includes(word));
  };
  if (contains(CONTROL.chat_invitation_markers)) return 'chat_invitation';
  if (contains(CONTROL.divert_markers)) return 'divert';
  if (isWhole(CONTROL.stop_markers) || isAllMarkers(CONTROL.stop_markers)) return 'stop';
  if (isWhole(CONTROL.reject_markers) || isAllMarkers(CONTROL.reject_markers)) return 'reject';
  return null;
}

export function isLikelyTaskRequest(
  utterance: string,
  lemmatize: Lemmatizer = suffixStripLemmatizer,
): boolean {
  const normalized = utterance.toLowerCase().replace(/[’]/gu, "'").trim().replace(/\s+/gu, ' ');
  if (!normalized) return false;
  // "let's" / "let us" are invitations, not task statements: "let's create anything" and "let's
  // talk about AI" are collaboration, while "let's clean the kitchen" is work. Treating the prefix
  // alone as a task made every creative or exploratory opening an atomization request — the owner's
  // "when I asked to 'let's create anything' it simply says what is the smallest part to start".
  // So a `let's` prefix now requires an actual task verb after it; every other prefix is unchanged.
  const COLLABORATIVE_PREFIXES = new Set(["let's ", 'lets ', 'let us ']);
  const hasTaskPrefix = TASK_REQUEST_PREFIXES.some((prefix) => {
    if (!normalized.startsWith(prefix) || normalized.length <= prefix.length) return false;
    if (!COLLABORATIVE_PREFIXES.has(prefix)) return true;
    const rest = normalized.slice(prefix.length).replace(/^(just|please|maybe)\s+/u, '');
    const verb = rest.split(' ', 1)[0];
    return verb !== undefined && lemmatize(verb).some((candidate) => TASK_VERBS.has(candidate));
  });
  if (hasTaskPrefix) {
    const result = classifyByRule(normalized);
    // A task such as "I need to say hello" contains a social word but is still a task. Explicit
    // controls/questions remain protected from the task-prefix shortcut.
    //
    // `stuck` joins `chitchat` here because "help me clean the kitchen" matched the stuck rule (on
    // "help") and was therefore NOT treated as a task — it routed to converse while "let's create
    // anything" routed to task, i.e. exactly inverted. Someone who says "help me <activity>" is
    // naming work AND is stuck; both are true, and the task reading is the useful one.
    return !result.matched || result.label === 'chitchat' || result.label === 'stuck';
  }
  // Users usually say the task as a bare imperative: "pay the electricity bill" or "open Slack".
  // Treating those as unrecognized chitchat was the reason valid speech never reached atomize.
  // Question-shaped utterances remain conversation, even when they start with a task verb.
  if (normalized.includes('?') || QUESTION_FORM_PREFIXES.some((prefix) => normalized.startsWith(prefix))) {
    return false;
  }
  const firstWord = normalized.replace(/^(just|please)\s+/u, '').split(' ', 1)[0];
  if (firstWord === undefined) return false;
  // Lemma-aware: "Opened the file already" / "Sending the email now" / "paying the electricity
  // bill" all have a conjugated first word that a bare `TASK_VERBS.has(firstWord)` would miss even
  // though the base verb (open/send/pay) is already in the set (Lemmatizer.ts's motivating bugs).
  return lemmatize(firstWord).some((candidate) => TASK_VERBS.has(candidate));
}

/** Track B (ORB-ACCEPTANCE-CONTRACT B1/B4) — where a FIRST utterance at INTAKE should go. */
export type IntakeRouteKind = 'task' | 'converse' | 'teach';

/** True when the utterance is asking to be taught/explained something, independent of state. Pure,
 * rule-first, versioned in `intent-rules.v1.json`'s `intake_open_domain_v1.teach_markers`. */
export function isTeachRequestUtterance(utterance: string): boolean {
  const normalized = utterance.toLowerCase().trim().replace(/[’]/gu, "'").replace(/\s+/gu, ' ');
  if (!normalized) return false;
  return matchesAny(normalized, OPEN_DOMAIN.teach_markers);
}

/**
 * Classifies a FIRST utterance at INTAKE into `'task'` (send to atomize/clarify — the existing,
 * unchanged path), `'teach'`, or `'converse'`. Pure, deterministic, rule-first: zero LLM decisions
 * (AGENTS.md determinism invariant; `docs/BUILD-DIGEST.md` §4). This is the fix for
 * ORB-ACCEPTANCE-CONTRACT B1: the FIRST utterance's routing depended on a narrow regex
 * (`i am|i'm|i feel|i had|i was|i think|how are|what is|what's|can you|could you|do you`) that
 * "teach me about photosynthesis" — and a dozen other ordinary open-domain openers — never matched.
 *
 * ORDER IS THE REGRESSION GUARD (B3): `isLikelyTaskRequest` is checked FIRST and unconditionally
 * short-circuits to `'task'`. Every marker list below is only ever consulted for utterances that
 * already failed the task check, so broadening conversational recognition here can never re-route
 * a real task ("write the quarterly report", "clean the kitchen", "fix the login bug", "email
 * Priya about the invoice") into conversation — the one failure mode this track was warned not to
 * introduce ("do not weaken `isLikelyTaskRequest` to get B1 passing").
 */
export function classifyIntakeUtterance(
  rawUtterance: string,
  lemmatize: Lemmatizer = suffixStripLemmatizer,
): IntakeRouteKind {
  const normalized = rawUtterance.toLowerCase().trim().replace(/[’]/gu, "'").replace(/\s+/gu, ' ');
  if (!normalized) return 'task'; // caller (T0FocusSession) handles empty/whitespace before this — B5.
  // Steering the conversation outranks naming work — see `classifyConversationControl`.
  if (classifyConversationControl(normalized) !== null) return 'converse';
  if (isLikelyTaskRequest(normalized, lemmatize)) return 'task';
  if (matchesAny(normalized, OPEN_DOMAIN.teach_markers)) return 'teach';
  const ruleResult = classifyByRule(normalized);
  if (ruleResult.matched && ['question', 'chitchat', 'stuck'].includes(ruleResult.label)) {
    return 'converse';
  }
  if (normalized.includes('?')) return 'converse';
  if (
    matchesAny(normalized, OPEN_DOMAIN.opinion_markers) ||
    matchesAny(normalized, OPEN_DOMAIN.meta_orb_markers) ||
    matchesAny(normalized, OPEN_DOMAIN.feeling_markers) ||
    matchesAny(normalized, OPEN_DOMAIN.ambiguous_fragment_markers) ||
    matchesAny(normalized, OPEN_DOMAIN.hinglish_markers)
  ) {
    return 'converse';
  }
  // Short, bare reactive interjections ("wow", "hmm..."). Whole-utterance equality only (after
  // stripping trailing punctuation) so a task sentence that happens to contain "cool" among many
  // other words is never swept in — only a bare, isolated reaction is.
  const bareUtterance = normalized.replace(/[.,!?]+$/u, '');
  if (OPEN_DOMAIN.reactive_interjections.includes(bareUtterance)) return 'converse';
  // Legacy first-person framing, preserved verbatim from the original isConversationalIntake regex
  // for behavioural continuity ("I had a rough morning" already worked; it must keep working).
  if (matchesAny(normalized, OPEN_DOMAIN.conversational_framing_markers)) return 'converse';
  // Two carve-outs BEFORE the converse default, each restoring a behaviour the default flip broke.
  //
  // 1. Text these rules cannot read at all. Every marker list here is English (plus Hinglish
  //    transliterated into Latin), so a Devanagari or other non-Latin utterance matches nothing and
  //    would silently take the default. Guessing "conversation" for text we cannot parse is not
  //    better than guessing "task" — it is just a different guess — so this keeps the prior
  //    behaviour rather than quietly changing it under a script the classifier does not cover.
  //    KNOWN GAP, stated plainly: non-Latin intake is unclassified, not classified-as-task.
  if (/[^\u0000-\u024f\u2000-\u206f]/u.test(normalized)) return 'task';
  // 2. A bare short noun phrase — "taxes", "the bathroom door", "quarterly report". No verb, no
  //    pronoun, no question: naming a thing is how people name work, and this is the input CLARIFY
  //    exists for. Capped at three words and required to be pronoun-free so it cannot swallow a
  //    conversational fragment.
  const words = normalized.replace(/[.,!?]+$/u, '').split(' ');
  const hasPronoun = words.some((word) =>
    ['i', "i'm", 'im', 'me', 'my', 'we', 'you', 'your', 'it', 'that', 'this'].includes(word),
  );
  // "let's create anything" is three words with no pronoun, so this carve-out claimed it and undid
  // the `let's` fix two steps above — caught by that fix's own test. A collaborative opening was
  // already decided as an invitation; a bare NOUN phrase has no verb and no invitation in it.
  const isInvitation = ['let', 'lets', "let's"].includes(words[0] ?? '');
  if (words.length <= 3 && !hasPronoun && !isInvitation && !normalized.includes('?')) return 'task';
  // DEFAULT: converse, not task.
  //
  // This was `return 'task'`, which meant anything the marker lists did not recognise became a
  // step-atomization request. That is the wrong default for this product twice over: the owner's
  // direction is that *"offloading mental load and discussing should be the default behaviour"*,
  // and an unrecognised utterance is by definition the case where we are LEAST sure work was being
  // named. Guessing "task" there produces the reported failure — "right now its just one by one
  // messaging not a conversation completely."
  //
  // Genuine tasks are not lost: `isLikelyTaskRequest` above still short-circuits every task prefix
  // and every bare imperative with a task verb, and a user who is asked a question can always name
  // the work. The asymmetry is deliberate — mistaking a task for conversation costs one clarifying
  // exchange, while mistaking conversation for a task hands executive load back to someone who
  // came here short of it.
  return 'converse';
}

/**
 * Tier-4 extension point: given the raw utterance and the rule-first pass's candidates, resolve a
 * single `IntentLabel`. This is a strategy/callback parameter, not a call site — nothing in this
 * file invokes an LLM. Owner: the cheap-LLM crew wired in `backend/relay-py/src/orb_relay/proxy/` behind
 * `@pe/llm-gateway` (BUILD-DIGEST §8 Tier 4; C9 — no provider SDK may be imported here).
 */
export type AmbiguousIntentResolver = (
  utterance: string,
  candidates: readonly IntentLabel[],
) => IntentLabel | Promise<IntentLabel>;

/** Thrown by the default resolver — mirrors `validateAtomizerOutput`'s not-implemented marker. */
export class AmbiguousIntentNotWiredError extends Error {
  constructor(utterance: string, candidates: readonly IntentLabel[]) {
    super(
      `classifyIntent: ambiguous utterance ${JSON.stringify(utterance)} (candidates: ` +
        `${candidates.length ? candidates.join(', ') : 'none'}) — not implemented — owned by the ` +
        'Tier-4 cheap-LLM crew (backend/relay-py/src/orb_relay/proxy/atomizer.py et al., via @pe/llm-gateway)',
    );
    this.name = 'AmbiguousIntentNotWiredError';
  }
}

/** Default resolver: throws the not-implemented marker rather than silently guessing a label. */
export const notYetWiredAmbiguousResolver: AmbiguousIntentResolver = (utterance, candidates) => {
  throw new AmbiguousIntentNotWiredError(utterance, candidates);
};

/**
 * `classifyIntent(utterance, resolveAmbiguous?)` — rule-first, falling to `resolveAmbiguous` only
 * when the deterministic pass can't pick a single label. Defaults to
 * `notYetWiredAmbiguousResolver`, so calling this with an ambiguous utterance and no resolver
 * supplied throws the clearly-labeled not-implemented marker rather than silently misclassifying.
 */
export function classifyIntent(
  utterance: string,
  resolveAmbiguous: AmbiguousIntentResolver = notYetWiredAmbiguousResolver,
): IntentLabel | Promise<IntentLabel> {
  const result = classifyByRule(utterance);
  if (result.matched) return result.label;
  return resolveAmbiguous(utterance, result.candidates);
}
