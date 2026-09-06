import type { CompletionResponse, ContentBlock } from '@pe/llm-gateway';

export const SPEECH_MARKUP_TAGS = [
  'warm',
  'happy',
  'sad',
  'angry',
  'scared',
  'tired',
  'chuckle',
  'chuckling',
  'laugh',
  'laughing',
  'giggle',
  'excited',
  'friendly',
  'playful',
  'sigh',
  'gasp',
  'breathing',
  'inhale',
  'exhale',
  'clears throat',
  'groan',
  'panting',
  'mumbling',
  'hmm',
  'thinking',
  'empathetic',
  'confident',
  'emphasis',
  'pause',
  'short pause',
  'long pause',
  'whispering',
  'soft voice',
  'loud voice',
  'shouting',
  'low voice',
  'high pitch',
  'speaking slowly',
  'speaking fast',
  'curious',
  'gentle',
  'celebratory',
] as const;

export type SpeechMarkupTag = (typeof SPEECH_MARKUP_TAGS)[number];
export type SpeechIntent =
  | 'presence'
  | 'capture'
  | 'clarify'
  | 'shrink'
  | 'suggest'
  | 'celebrate'
  | 'repair'
  | 'pause';

export interface NormalizedSpeechResponse {
  readonly intent: SpeechIntent;
  readonly spoken_text: string;
  readonly plain_text: string;
  readonly tags: readonly SpeechMarkupTag[];
  readonly emotion: string;
  readonly interruptible: boolean;
  readonly max_duration_ms: number;
  readonly follow_up: {
    readonly kind: 'wait_for_user' | 'schedule_check_in' | 'none';
    readonly delay_ms: number;
  };
}

export interface NormalizedCompletionResponse extends CompletionResponse {
  readonly speech: NormalizedSpeechResponse;
}

/**
 * System contract for model-generated user-facing speech.
 *
 * This is intentionally opt-in at the HTTP boundary. The same gateway also serves the
 * atomizer, whose JSON schema must never be contaminated with speech tags. Callers that need a
 * spoken LLM answer send `response_mode: "speech"`; the sidecar injects this contract before the
 * model call and normalizes the returned envelope on the way out.
 */
export const SPEECH_MARKUP_SYSTEM_PROMPT = `
You are generating one short, human, ADHD-friendly spoken response for a focus body-double.
Return ONLY one JSON object with exactly these fields:
{"intent":"presence|capture|clarify|shrink|suggest|celebrate|repair|pause","spoken_text":"string","emotion":"warm|curious|gentle|celebratory|upbeat|calm","interruptible":true,"max_duration_ms":4200,"follow_up":{"kind":"wait_for_user|schedule_check_in|none","delay_ms":0}}

The spoken_text is sent directly to Fish Audio S2 TTS. S2 accepts natural-language bracket tags, so
use these when they improve the delivery: [friendly] [warm] [happy] [excited] [curious] [playful]
[empathetic] [confident] [thinking] [breathing] [inhale] [exhale] [sigh] [gasp] [clears throat]
[chuckle] or [chuckling] [laughing] [giggle] [hmm] [emphasis] [pause] [short pause] [long pause]
[whispering] [soft voice] [loud voice] [shouting] [low voice] [high pitch] [speaking slowly]
[speaking fast].
Use one primary mood per sentence and space emotional changes out. Use at most two physical cues
([breathing], [inhale], [exhale], [sigh], [gasp], [clears throat], [chuckling], [laughing]) per
response, two [emphasis] tags, and three [short pause] tags. Put an emotion tag at the start of a
sentence or beat, and put [emphasis] immediately before the important word or short phrase.
For a thinking/clarifying response, one spoken “Hmm,” is welcome; never repeat fillers. Use [long
pause] only for reflective space, never for an error, safety, consent, or repair message. Do not
describe the tags or put markup in any JSON field other than spoken_text.
Keep the response under 480 characters and ask at most one question. Never expose raw backend errors,
secrets, or internal identifiers. The emotion field is a bounded prosody hint, not permission to change
state or safety policy.

Good example:
{"intent":"presence","spoken_text":"[inhale] [friendly] Hey, I’m here with you. [short pause] We can take [emphasis]one small move together.","emotion":"warm","interruptible":true,"max_duration_ms":4200,"follow_up":{"kind":"wait_for_user","delay_ms":0}}

Reflective example:
{"intent":"clarify","spoken_text":"[thinking] Hmm, let’s make this smaller. [short pause] What is the next visible action?","emotion":"curious","interruptible":true,"max_duration_ms":4200,"follow_up":{"kind":"wait_for_user","delay_ms":0}}
`.trim();

const TAG_PATTERN = /\[([^\]]+)\]/g;
const ALLOWED_TAGS = new Set<string>(SPEECH_MARKUP_TAGS);
const INTENTS = new Set<SpeechIntent>([
  'presence',
  'capture',
  'clarify',
  'shrink',
  'suggest',
  'celebrate',
  'repair',
  'pause',
]);
const FOLLOW_UP_KINDS = new Set(['wait_for_user', 'schedule_check_in', 'none']);
const DEFAULT_SPEECH: Omit<NormalizedSpeechResponse, 'spoken_text' | 'plain_text' | 'tags'> = {
  intent: 'presence',
  emotion: 'warm',
  interruptible: true,
  max_duration_ms: 4_200,
  follow_up: { kind: 'none', delay_ms: 0 },
};

const PHYSICAL_TAGS = new Set<SpeechMarkupTag>([
  'breathing', 'inhale', 'exhale', 'sigh', 'gasp', 'clears throat', 'chuckle', 'chuckling',
  'laugh', 'laughing', 'giggle', 'groan', 'panting',
]);
const MOOD_TAGS = new Set<SpeechMarkupTag>([
  'warm', 'happy', 'sad', 'angry', 'scared', 'tired', 'excited', 'friendly', 'playful',
  'curious', 'gentle', 'celebratory', 'empathetic', 'confident', 'thinking',
]);
const PAUSE_TAGS = new Set<SpeechMarkupTag>(['pause', 'short pause', 'long pause']);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function clampDuration(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return DEFAULT_SPEECH.max_duration_ms;
  return Math.max(800, Math.min(Math.round(value), 6_000));
}

function normalizeTag(raw: string): SpeechMarkupTag | null {
  const tag = raw.trim().toLowerCase().replace(/\s+/g, ' ');
  return ALLOWED_TAGS.has(tag) ? (tag as SpeechMarkupTag) : null;
}

function sanitizeMarkup(input: string, intent: SpeechIntent): {
  spoken_text: string;
  plain_text: string;
  tags: readonly SpeechMarkupTag[];
} {
  const tags: SpeechMarkupTag[] = [];
  const spoken_text = input
    // Models sometimes emit Markdown emphasis despite the Fish markup contract. Convert it to
    // Fish's explicit cue so the user hears emphasis instead of literal asterisks.
    .replace(/\*{2}([^*]+)\*{2}/g, (_match, text: string) => {
      return `[emphasis] ${text}`;
    })
    .replace(/\*([^*]+)\*/g, (_match, text: string) => {
      return `[emphasis] ${text}`;
    })
    .replace(TAG_PATTERN, (_match, raw: string) => {
      const tag = normalizeTag(raw);
      // Long pauses are useful for reflection, but a repair must stay concise and actionable.
      if (tag === 'long pause' && intent === 'repair') return '';
      if (tag === null) return '';
      tags.push(tag);
      return `[${tag}]`;
    })
    .replace(/\s+/g, ' ')
    .trim();
  const plain_text = spoken_text
    .replace(TAG_PATTERN, '')
    .replace(/\s+/g, ' ')
    .trim();
  return { spoken_text, plain_text, tags };
}

function limitQuestions(input: string): string {
  let questionSeen = false;
  return input.replace(/\?/g, () => {
    if (questionSeen) return '.';
    questionSeen = true;
    return '?';
  });
}

/** Adds a small amount of deterministic human prosody when the model returns a bare sentence. */
function addNaturalProsody(
  text: string,
  intent: SpeechIntent,
  emotion: string,
  existingTags: readonly SpeechMarkupTag[],
): string {
  // A model that already supplied Fish cues made an intentional delivery choice. Preserve its
  // exact timing/emotion plan instead of stacking server-generated tags on top of it.
  if (existingTags.length > 0) return text;
  let output = text;
  const hasPhysical = existingTags.some((tag) => PHYSICAL_TAGS.has(tag));
  const hasMood = existingTags.some((tag) => MOOD_TAGS.has(tag));
  const hasPause = existingTags.some((tag) => PAUSE_TAGS.has(tag));

  if (intent === 'clarify' && !/\b(hmm|um|uh)\b/i.test(output)) {
    output = `[thinking] Hmm, ${output}`;
  } else if (!hasPhysical && intent !== 'repair' && intent !== 'pause') {
    output = `[breathing] ${output}`;
  }

  if (!hasMood) {
    const requestedMood = emotion.trim().toLowerCase() as SpeechMarkupTag;
    const mood: SpeechMarkupTag = intent === 'celebrate' || emotion === 'celebratory' || emotion === 'upbeat'
      ? 'excited'
      : intent === 'repair'
        ? 'empathetic'
        : MOOD_TAGS.has(requestedMood)
          ? requestedMood
          : intent === 'clarify'
            ? 'curious'
            : 'friendly';
    output = `[${mood}] ${output}`;
  }

  if (!hasPause && (output.match(/[.!?](?=\s|$)/g) ?? []).length >= 2) {
    output = output.replace(/([.!?])\s+/, '$1 [short pause] ');
  }
  return output;
}

function parseModelEnvelope(raw: string): {
  text: string;
  isSpeechEnvelope: boolean;
  isStructured: boolean;
  intent: SpeechIntent;
  emotion: string;
  interruptible: boolean;
  max_duration_ms: number;
  follow_up: NormalizedSpeechResponse['follow_up'];
} {
  const fallback = {
    text: raw,
    isSpeechEnvelope: true,
    isStructured: false,
    intent: DEFAULT_SPEECH.intent,
    emotion: DEFAULT_SPEECH.emotion,
    interruptible: DEFAULT_SPEECH.interruptible,
    max_duration_ms: DEFAULT_SPEECH.max_duration_ms,
    follow_up: DEFAULT_SPEECH.follow_up,
  };
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed) || typeof parsed.spoken_text !== 'string') {
      return { ...fallback, isSpeechEnvelope: false };
    }
    const intent = typeof parsed.intent === 'string' && INTENTS.has(parsed.intent as SpeechIntent)
      ? (parsed.intent as SpeechIntent)
      : DEFAULT_SPEECH.intent;
    const followUp = isRecord(parsed.follow_up) ? parsed.follow_up : {};
    const followUpKind = typeof followUp.kind === 'string' && FOLLOW_UP_KINDS.has(followUp.kind)
      ? (followUp.kind as NormalizedSpeechResponse['follow_up']['kind'])
      : 'none';
    const followUpDelay = typeof followUp.delay_ms === 'number' && Number.isFinite(followUp.delay_ms)
      ? Math.max(0, Math.min(Math.round(followUp.delay_ms), 300_000))
      : 0;
    return {
      text: parsed.spoken_text,
      isSpeechEnvelope: true,
      isStructured: true,
      intent,
      emotion: typeof parsed.emotion === 'string' ? parsed.emotion : DEFAULT_SPEECH.emotion,
      interruptible: parsed.interruptible !== false,
      max_duration_ms: clampDuration(parsed.max_duration_ms),
      follow_up: { kind: followUpKind, delay_ms: followUpDelay },
    };
  } catch {
    return fallback;
  }
}

export function normalizeSpeechText(raw: string): NormalizedSpeechResponse {
  const parsed = parseModelEnvelope(raw.trim());
  if (!parsed.isSpeechEnvelope) {
    return {
      ...DEFAULT_SPEECH,
      spoken_text: raw,
      plain_text: raw,
      tags: [],
    };
  }
  const text = parsed.text.slice(0, 1_000);
  const sanitized = sanitizeMarkup(text, parsed.intent);
  const naturalText = parsed.isStructured
    ? addNaturalProsody(sanitized.spoken_text, parsed.intent, parsed.emotion, sanitized.tags)
    : sanitized.spoken_text;
  const finalSanitized = sanitizeMarkup(naturalText.slice(0, 480).trim(), parsed.intent);
  const boundedText = limitQuestions(finalSanitized.spoken_text);
  const boundedPlainText = boundedText
    .replace(TAG_PATTERN, '')
    .replace(/\s+/g, ' ')
    .trim();
  return {
    ...DEFAULT_SPEECH,
    intent: parsed.intent,
    emotion: parsed.emotion,
    interruptible: parsed.interruptible,
    max_duration_ms: parsed.max_duration_ms,
    follow_up: parsed.follow_up,
    spoken_text: boundedText,
    plain_text: boundedPlainText,
    tags: finalSanitized.tags,
  };
}

function textBlock(block: ContentBlock): string | null {
  return block.type === 'text' ? block.text : null;
}

export function normalizeCompletionResponse(result: CompletionResponse): NormalizedCompletionResponse {
  const text = result.content.map(textBlock).find((value): value is string => value !== null) ?? '';
  const speech = normalizeSpeechText(text);
  const content = result.content.map((block) =>
    block.type === 'text' && block.text === text ? { ...block, text: speech.spoken_text } : block,
  );
  return { ...result, content, speech };
}
