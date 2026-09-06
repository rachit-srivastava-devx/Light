import { describe, expect, it } from 'vitest';
import { normalizeCompletionResponse, normalizeSpeechText, SPEECH_MARKUP_SYSTEM_PROMPT } from '../src/speechMarkup';

describe('speech markup contract', () => {
  it('publishes a strict opt-in LLM contract for Fish markup', () => {
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('Return ONLY one JSON object');
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('[chuckle]');
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('[excited]');
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('[playful]');
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('[emphasis]');
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('[long pause]');
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('[breathing]');
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('Hmm');
    expect(SPEECH_MARKUP_SYSTEM_PROMPT).toContain('Never expose raw backend errors');
  });

  it('preserves the full Fish example while deriving clean text for non-TTS consumers', () => {
    const result = normalizeSpeechText(JSON.stringify({
      intent: 'suggest',
      spoken_text: "[chuckle] When you're creating something new, there's this [emphasis] beautiful mix of wonder and fear. [long pause] And it's overwhelming sometimes, and scary, but also incredibly magical.",
      emotion: 'curious',
      interruptible: true,
      max_duration_ms: 4200,
      follow_up: { kind: 'none', delay_ms: 0 },
    }));
    expect(result.spoken_text).toContain('[chuckle]');
    expect(result.spoken_text).toContain('[emphasis]');
    expect(result.spoken_text).toContain('[long pause]');
    expect(result.plain_text).not.toMatch(/\[[^\]]+\]/);
    expect(result.tags).toEqual(['chuckle', 'emphasis', 'long pause']);
  });

  it('preserves supported Fish tags and produces plain text for logs/accessibility', () => {
    const result = normalizeSpeechText('[chuckle] You did it. [emphasis] One step. [long pause]');
    expect(result.spoken_text).toBe('[chuckle] You did it. [emphasis] One step. [long pause]');
    expect(result.plain_text).toBe('You did it. One step.');
    expect(result.tags).toEqual(['chuckle', 'emphasis', 'long pause']);
  });

  it('converts Markdown emphasis to a Fish emphasis tag instead of speaking asterisks', () => {
    const result = normalizeSpeechText('[warm] So, what *are* we focusing on?');

    expect(result.spoken_text).toBe('[warm] So, what [emphasis] are we focusing on?');
    expect(result.plain_text).toBe('So, what are we focusing on?');
    expect(result.tags).toEqual(['warm', 'emphasis']);
  });

  it('accepts expressive Fish cues without leaking them into plain text', () => {
    const result = normalizeSpeechText('[friendly] Hey! [excited] We got started. [pause] Nice work.');
    expect(result.spoken_text).toBe('[friendly] Hey! [excited] We got started. [pause] Nice work.');
    expect(result.plain_text).toBe('Hey! We got started. Nice work.');
    expect(result.tags).toEqual(['friendly', 'excited', 'pause']);
  });

  it('adds a single natural breath, mood, beat, and thinking filler to bare structured speech', () => {
    const result = normalizeSpeechText(JSON.stringify({
      intent: 'clarify',
      spoken_text: 'We can make this smaller. What is the next visible action?',
      emotion: 'curious',
      follow_up: { kind: 'wait_for_user', delay_ms: 0 },
    }));

    expect(result.spoken_text).toBe('[curious] [thinking] Hmm, We can make this smaller. [short pause] What is the next visible action?');
    expect(result.plain_text).toBe('Hmm, We can make this smaller. What is the next visible action?');
    expect(result.tags).toEqual(['curious', 'thinking', 'short pause']);
  });

  it('strips unsupported tags without losing the spoken words', () => {
    const result = normalizeSpeechText('[scream] I am [mystery] still here.');
    expect(result.spoken_text).toBe('I am still here.');
    expect(result.plain_text).toBe('I am still here.');
    expect(result.tags).toEqual([]);
  });

  it('accepts the structured LLM response and applies safety bounds', () => {
    const result = normalizeSpeechText(JSON.stringify({
      intent: 'repair',
      spoken_text: '[warm] I missed that. [long pause] Please try again?',
      emotion: 'curious',
      interruptible: false,
      max_duration_ms: 99_999,
      follow_up: { kind: 'schedule_check_in', delay_ms: 999_999 },
    }));
    expect(result.intent).toBe('repair');
    expect(result.spoken_text).toBe('[warm] I missed that. Please try again?');
    expect(result.max_duration_ms).toBe(6_000);
    expect(result.follow_up).toEqual({ kind: 'schedule_check_in', delay_ms: 300_000 });
    expect(result.interruptible).toBe(false);
  });

  it('limits spoken responses to one question', () => {
    const result = normalizeSpeechText('Are you ready? Want to start? We can do one step.');
    expect(result.spoken_text).toBe('Are you ready? Want to start. We can do one step.');
    expect(result.plain_text).toBe('Are you ready? Want to start. We can do one step.');
  });

  it('does not reinterpret atomizer JSON as a speech envelope', () => {
    const raw = '{"steps":[{"step_text":"Open the file"}],"steps_total":1}';
    const result = normalizeSpeechText(raw);
    expect(result.plain_text).toBe(raw);
    expect(result.tags).toEqual([]);
  });

  it('adds normalized speech metadata while keeping the response contract', () => {
    const response = normalizeCompletionResponse({
      id: 'r1',
      content: [
        { type: 'text', text: '[celebratory] Done!' },
        { type: 'tool_use', id: 't1', name: 'noop', input: {} },
      ],
      stop_reason: 'end_turn',
      usage: { input_tokens: 1, output_tokens: 1, cache_read_tokens: 0, cache_creation_tokens: 0 },
      cost: { usd: 0, inr: 0, model: 'test', tier: 'small', cached: false },
      model: 'test',
      latency_ms: 1,
    });
    expect(response.content[0]).toEqual({ type: 'text', text: '[celebratory] Done!' });
    expect(response.content[1]?.type).toBe('tool_use');
    expect(response.speech.plain_text).toBe('Done!');
  });
});
