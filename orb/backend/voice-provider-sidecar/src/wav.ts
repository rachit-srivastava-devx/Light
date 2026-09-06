/**
 * Wraps raw bytes in a minimal 16-bit PCM WAV container so file-upload-style STT APIs (Sarvam)
 * get a self-describing audio file instead of a bare byte stream.
 *
 * ASSUMPTION (flagged, not verified against a real device capture): the contract's
 * `audio_bytes: number[]` is treated as 16-bit little-endian PCM, mono, 16kHz — the sample rate
 * `docs/BUILD-DIGEST.md`'s voice plane targets elsewhere in this repo. If the real mobile client
 * ships a different sample rate/format, this header is wrong and must be corrected together with
 * whatever sets that format on the client side; there is no live audio capture available in this
 * environment to confirm it byte-for-byte.
 */
/** Mic/STT upload format. TTS uses the higher-fidelity relay target below. */
const STT_SAMPLE_RATE = 16_000;
/** Fish TTS output format carried over relay and normalized by mobile for playback. */
const TTS_RELAY_SAMPLE_RATE = 24_000;
const CHANNELS = 1;
const BITS_PER_SAMPLE = 16;

export function pcm16ToWav(pcm: Uint8Array): Uint8Array {
  const blockAlign = (CHANNELS * BITS_PER_SAMPLE) / 8;
  const byteRate = STT_SAMPLE_RATE * blockAlign;
  const buffer = new ArrayBuffer(44 + pcm.length);
  const view = new DataView(buffer);

  writeAscii(view, 0, 'RIFF');
  view.setUint32(4, 36 + pcm.length, true);
  writeAscii(view, 8, 'WAVE');
  writeAscii(view, 12, 'fmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true); // PCM
  view.setUint16(22, CHANNELS, true);
  view.setUint32(24, STT_SAMPLE_RATE, true);
  view.setUint32(28, byteRate, true);
  view.setUint16(32, blockAlign, true);
  view.setUint16(34, BITS_PER_SAMPLE, true);
  writeAscii(view, 36, 'data');
  view.setUint32(40, pcm.length, true);

  const out = new Uint8Array(buffer);
  out.set(pcm, 44);
  return out;
}

function writeAscii(view: DataView, offset: number, text: string): void {
  for (let i = 0; i < text.length; i += 1) {
    view.setUint8(offset + i, text.charCodeAt(i));
  }
}

function readAscii(view: DataView, offset: number, length: number): string {
  let out = '';
  for (let i = 0; i < length; i += 1) out += String.fromCharCode(view.getUint8(offset + i));
  return out;
}

/**
 * Every TTS backend in this sidecar (Fish, Sarvam, Cartesia) requests or returns a WAV container,
 * not headerless raw PCM — `RelayAudioPlayer.ts` on the client assumes the bytes it receives ARE
 * raw 16-bit PCM samples (see its own confidence note). Without this, the 44+ header bytes get
 * played as if they were the first ~22 audio samples: an audible click/glitch at minimum, and if
 * the response isn't actually WAV at all (e.g. Fish silently defaulting to MP3 despite the
 * requested format), corrupted noise for the whole clip — that second case cannot be fixed here,
 * only detected and logged, since MP3 decoding is a real codec, not a header to skip.
 *
 * Walks the RIFF chunk list properly (rather than assuming a fixed 44-byte offset) because some
 * encoders insert extra chunks — e.g. a `LIST`/`INFO` metadata chunk — before `data`.
 *
 * Returns the original bytes unchanged (not a partial/corrupted slice) if this doesn't parse as a
 * well-formed RIFF/WAVE container, so a non-WAV response is not guessed as PCM.
 */
export function extractPcmFromWav(bytes: Uint8Array): { pcm: Uint8Array; wasWav: boolean } {
  if (bytes.length < 12) return { pcm: bytes, wasWav: false };
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (readAscii(view, 0, 4) !== 'RIFF' || readAscii(view, 8, 4) !== 'WAVE') {
    return { pcm: bytes, wasWav: false };
  }
  let format:
    | { audioFormat: number; channels: number; sampleRateHz: number; blockAlign: number; bitsPerSample: number }
    | undefined;
  let data: Uint8Array | undefined;
  let offset = 12;
  while (offset + 8 <= bytes.length) {
    const chunkId = readAscii(view, offset, 4);
    const chunkSize = view.getUint32(offset + 4, true);
    const dataStart = offset + 8;
    const dataEnd = Math.min(dataStart + chunkSize, bytes.length);
    if (dataStart > bytes.length || dataEnd < dataStart) return { pcm: bytes, wasWav: false };
    if (chunkId === 'fmt ' && chunkSize >= 16) {
      format = {
        audioFormat: view.getUint16(dataStart, true),
        channels: view.getUint16(dataStart + 2, true),
        sampleRateHz: view.getUint32(dataStart + 4, true),
        blockAlign: view.getUint16(dataStart + 12, true),
        bitsPerSample: view.getUint16(dataStart + 14, true),
      };
    }
    if (chunkId === 'data') {
      data = bytes.subarray(dataStart, dataEnd);
      break;
    }
    // Chunks are padded to even byte boundaries per the RIFF spec.
    offset = dataStart + chunkSize + (chunkSize % 2);
  }
  if (data && format) {
    if (
      format.audioFormat !== 1 ||
      format.channels !== CHANNELS ||
      format.bitsPerSample !== BITS_PER_SAMPLE ||
      format.blockAlign !== CHANNELS * (BITS_PER_SAMPLE / 8) ||
      format.sampleRateHz <= 0 ||
      data.length % (BITS_PER_SAMPLE / 8) !== 0
    ) {
      throw new Error(
        `Fish TTS returned unsupported WAV format: ${format.channels}ch/${format.sampleRateHz}Hz/${format.bitsPerSample}bit`,
      );
    }
    return { pcm: resamplePcm16(data, format.sampleRateHz, TTS_RELAY_SAMPLE_RATE), wasWav: true };
  }
  // Keep the legacy fmt-less fixture/pass-through behavior for the T0 fake path. Real provider
  // WAVs are validated above; a real WAV without fmt is still surfaced as `wasWav` so callers do
  // not accidentally prepend its RIFF header to playback.
  if (data) return { pcm: data, wasWav: true };
  // Well-formed RIFF/WAVE header but no `data` subchunk found — malformed, not a real WAV payload
  // worth trusting; return unchanged rather than guessing.
  return { pcm: bytes, wasWav: false };
}

/** Normalize a valid mono PCM16 WAV to the headerless 24 kHz TTS relay contract. */
function resamplePcm16(bytes: Uint8Array, sourceRateHz: number, targetRateHz: number): Uint8Array {
  if (sourceRateHz === targetRateHz) return bytes;
  const source = new Int16Array(bytes.length / 2);
  const sourceView = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let index = 0; index < source.length; index += 1) {
    source[index] = sourceView.getInt16(index * 2, true);
  }

  const targetLength = Math.max(1, Math.round((source.length * targetRateHz) / sourceRateHz));
  const target = new Uint8Array(targetLength * 2);
  const targetView = new DataView(target.buffer);
  for (let index = 0; index < targetLength; index += 1) {
    const position = (index * sourceRateHz) / targetRateHz;
    const left = Math.min(source.length - 1, Math.floor(position));
    const right = Math.min(source.length - 1, left + 1);
    const fraction = position - left;
    const sample = Math.max(
      -32768,
      Math.min(32767, Math.round((source[left] ?? 0) * (1 - fraction) + (source[right] ?? 0) * fraction)),
    );
    targetView.setInt16(index * 2, sample, true);
  }
  return target;
}
