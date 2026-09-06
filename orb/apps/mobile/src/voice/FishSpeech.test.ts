import { describe, expect, it, vi } from 'vitest';

import { sendFishSpeech } from './FishSpeech';

describe('Fish speech routing', () => {
  it('sends the selected voice and emotion through the open Fish transport', () => {
    const speak = vi.fn();

    expect(sendFishSpeech({ isClosed: false, speak }, 'Good morning, Rachit', 'voice-123', 'upbeat')).toBe(true);
    expect(speak).toHaveBeenCalledWith('Good morning, Rachit', 'voice-123', 'upbeat');
  });

  it('does not invoke any speech implementation when Fish is unavailable', () => {
    const speak = vi.fn();

    expect(sendFishSpeech({ isClosed: true, speak }, 'fallback text', 'native-voice', 'calm')).toBe(false);
    expect(speak).not.toHaveBeenCalled();
    expect(sendFishSpeech(null, 'fallback text', 'native-voice', 'calm')).toBe(false);
  });
});
