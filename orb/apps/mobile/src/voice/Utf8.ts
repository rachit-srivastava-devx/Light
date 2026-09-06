export function utf8Encode(text: string): Uint8Array {
  const bytes: number[] = [];
  for (const char of text) {
    const codePoint = char.codePointAt(0);
    if (codePoint === undefined) continue;
    if (codePoint <= 0x7f) {
      bytes.push(codePoint);
    } else if (codePoint <= 0x7ff) {
      bytes.push(0xc0 | (codePoint >> 6), 0x80 | (codePoint & 0x3f));
    } else if (codePoint <= 0xffff) {
      bytes.push(0xe0 | (codePoint >> 12), 0x80 | ((codePoint >> 6) & 0x3f), 0x80 | (codePoint & 0x3f));
    } else {
      bytes.push(
        0xf0 | (codePoint >> 18),
        0x80 | ((codePoint >> 12) & 0x3f),
        0x80 | ((codePoint >> 6) & 0x3f),
        0x80 | (codePoint & 0x3f),
      );
    }
  }
  return Uint8Array.from(bytes);
}

export function utf8Decode(bytes: Uint8Array): string {
  let text = '';
  for (let i = 0; i < bytes.length; i++) {
    const first = bytes[i];
    if (first === undefined || first === 0) continue;
    if (first <= 0x7f) {
      text += String.fromCodePoint(first);
      continue;
    }
    const second = bytes[++i];
    if (second === undefined) {
      text += '\uFFFD';
      break;
    }
    if ((first & 0xe0) === 0xc0) {
      text += String.fromCodePoint(((first & 0x1f) << 6) | (second & 0x3f));
      continue;
    }
    const third = bytes[++i];
    if (third === undefined) {
      text += '\uFFFD';
      break;
    }
    if ((first & 0xf0) === 0xe0) {
      text += String.fromCodePoint(((first & 0x0f) << 12) | ((second & 0x3f) << 6) | (third & 0x3f));
      continue;
    }
    const fourth = bytes[++i];
    if (fourth === undefined) {
      text += '\uFFFD';
      break;
    }
    text += String.fromCodePoint(
      ((first & 0x07) << 18) | ((second & 0x3f) << 12) | ((third & 0x3f) << 6) | (fourth & 0x3f),
    );
  }
  return text;
}
