import { describe, expect, it } from 'vitest';

import { utf8Decode, utf8Encode } from './Utf8';

describe('UTF-8 speech transport', () => {
  it('round-trips code-mixed Indic speech without Latin-1 truncation', () => {
    const transcript = 'मुझे टैक्स भरना है, then email Sam';
    expect(utf8Decode(utf8Encode(transcript))).toBe(transcript);
  });
});
