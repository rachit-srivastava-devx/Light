/**
 * Remote TTS audio boundary.
 *
 * The relay carries bytes, not a provider-specific decoder contract. This module makes the
 * boundary explicit: WAV headers are self-describing; raw PCM requires an explicit format. Every
 * accepted input is normalized to the one playback format used by the mobile voice port.
 */

/** Remote TTS playback target. Mic capture remains a separate 16 kHz STT contract. */
export const SPEECH_SAMPLE_RATE_HZ = 24_000 as const;
export const SPEECH_CHANNELS = 1 as const;
export const SPEECH_BITS_PER_SAMPLE = 16 as const;
export const MAX_SPEECH_DURATION_MS = 120_000;

export type SpeechAudioErrorCode =
  | 'empty_audio'
  | 'format_required'
  | 'invalid_wav'
  | 'unsupported_wav'
  | 'invalid_pcm'
  | 'non_speech_audio'
  | 'audio_too_long';

export class SpeechAudioError extends Error {
  readonly name = 'SpeechAudioError';

  constructor(
    readonly code: SpeechAudioErrorCode,
    message: string,
  ) {
    super(message);
  }
}

export interface RawPcmS16LeFormat {
  readonly kind: 'pcm_s16le';
  readonly sample_rate_hz: number;
  readonly channels: number;
}

export type RemoteTtsFormat = 'wav' | RawPcmS16LeFormat;

export interface NormalizedSpeechAudio {
  readonly samples: Int16Array;
  readonly sample_rate_hz: typeof SPEECH_SAMPLE_RATE_HZ;
  readonly channels: typeof SPEECH_CHANNELS;
  readonly bits_per_sample: typeof SPEECH_BITS_PER_SAMPLE;
  readonly duration_ms: number;
}

interface WavFormat {
  readonly audio_format: number;
  readonly channels: number;
  readonly sample_rate_hz: number;
  readonly byte_rate: number;
  readonly block_align: number;
  readonly bits_per_sample: number;
}

interface WavData {
  readonly format: WavFormat;
  readonly bytes: Uint8Array;
}

function bytesOf(input: ArrayBuffer | Uint8Array): Uint8Array {
  return input instanceof Uint8Array ? input : new Uint8Array(input);
}

function ascii(bytes: Uint8Array, start: number): string {
  return String.fromCharCode(bytes[start] ?? 0, bytes[start + 1] ?? 0, bytes[start + 2] ?? 0, bytes[start + 3] ?? 0);
}

function hasRiffWaveHeader(bytes: Uint8Array): boolean {
  return bytes.length >= 12 && ascii(bytes, 0) === 'RIFF' && ascii(bytes, 8) === 'WAVE';
}

function readUint16(view: DataView, offset: number): number {
  return view.getUint16(offset, true);
}

function readUint32(view: DataView, offset: number): number {
  return view.getUint32(offset, true);
}

function parseWav(bytes: Uint8Array): WavData {
  if (!hasRiffWaveHeader(bytes)) {
    throw new SpeechAudioError('invalid_wav', 'remote TTS audio is not a RIFF/WAVE file');
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const riffSize = readUint32(view, 4);
  // Fish Audio emits streaming WAVs with 0xffffffff placeholders because the final payload size
  // is not known when its header is written. The actual byte buffer is still bounded by the
  // received WebSocket frame, so treat the sentinel as "to end of buffer" rather than rejecting
  // an otherwise valid PCM stream.
  if (riffSize !== 0xffffffff && riffSize + 8 > bytes.length) {
    throw new SpeechAudioError('invalid_wav', 'WAV RIFF size exceeds the received bytes');
  }

  let format: WavFormat | undefined;
  let data: Uint8Array | undefined;
  let offset = 12;
  while (offset + 8 <= bytes.length) {
    const chunkId = ascii(bytes, offset);
    const chunkSize = readUint32(view, offset + 4);
    const chunkStart = offset + 8;
    const chunkEnd = chunkSize === 0xffffffff ? bytes.length : chunkStart + chunkSize;
    if (chunkEnd > bytes.length) {
      throw new SpeechAudioError('invalid_wav', `WAV ${chunkId} chunk exceeds the received bytes`);
    }

    if (chunkId === 'fmt ') {
      if (chunkSize < 16) {
        throw new SpeechAudioError('invalid_wav', 'WAV fmt chunk is shorter than PCM format metadata');
      }
      format = {
        audio_format: readUint16(view, chunkStart),
        channels: readUint16(view, chunkStart + 2),
        sample_rate_hz: readUint32(view, chunkStart + 4),
        byte_rate: readUint32(view, chunkStart + 8),
        block_align: readUint16(view, chunkStart + 12),
        bits_per_sample: readUint16(view, chunkStart + 14),
      };
    } else if (chunkId === 'data') {
      data = bytes.subarray(chunkStart, chunkEnd);
    }

    // RIFF chunks are word aligned; the pad byte is not audio data.
    offset = chunkEnd + (chunkSize % 2);
  }

  if (!format || !data) {
    throw new SpeechAudioError('invalid_wav', 'WAV must contain both fmt and data chunks');
  }
  return { format, bytes: data };
}

function validatePcmFormat(format: RawPcmS16LeFormat, byteLength: number): void {
  if (
    !Number.isInteger(format.sample_rate_hz) ||
    format.sample_rate_hz < 8_000 ||
    format.sample_rate_hz > 96_000 ||
    !Number.isInteger(format.channels) ||
    format.channels < 1 ||
    format.channels > 8
  ) {
    throw new SpeechAudioError('invalid_pcm', 'raw PCM format has invalid sample rate or channel count');
  }
  const frameBytes = format.channels * 2;
  if (byteLength === 0 || byteLength % frameBytes !== 0) {
    throw new SpeechAudioError('invalid_pcm', 'raw PCM byte length is not aligned to a 16-bit frame');
  }
}

function validateWavFormat(format: WavFormat, byteLength: number): RawPcmS16LeFormat {
  if (
    format.audio_format !== 1 ||
    format.bits_per_sample !== SPEECH_BITS_PER_SAMPLE ||
    format.channels < 1 ||
    format.channels > 8 ||
    format.sample_rate_hz < 8_000 ||
    format.sample_rate_hz > 96_000 ||
    format.block_align !== format.channels * 2 ||
    format.byte_rate !== format.sample_rate_hz * format.block_align
  ) {
    throw new SpeechAudioError('unsupported_wav', 'WAV must be little-endian PCM with 16-bit samples');
  }
  const rawFormat: RawPcmS16LeFormat = {
    kind: 'pcm_s16le',
    sample_rate_hz: format.sample_rate_hz,
    channels: format.channels,
  };
  validatePcmFormat(rawFormat, byteLength);
  return rawFormat;
}

function toMonoSamples(bytes: Uint8Array, format: RawPcmS16LeFormat): Int16Array {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const sourceFrames = bytes.length / (format.channels * 2);
  const mono = new Int16Array(sourceFrames);
  for (let frame = 0; frame < sourceFrames; frame += 1) {
    let total = 0;
    for (let channel = 0; channel < format.channels; channel += 1) {
      total += view.getInt16((frame * format.channels + channel) * 2, true);
    }
    mono[frame] = Math.max(-32768, Math.min(32767, Math.round(total / format.channels)));
  }
  return mono;
}

function resampleTo16k(source: Int16Array, sourceRateHz: number): Int16Array {
  if (sourceRateHz === SPEECH_SAMPLE_RATE_HZ) return source;

  const targetLength = Math.max(1, Math.round((source.length * SPEECH_SAMPLE_RATE_HZ) / sourceRateHz));
  const target = new Int16Array(targetLength);
  for (let index = 0; index < target.length; index += 1) {
    const sourcePosition = (index * sourceRateHz) / SPEECH_SAMPLE_RATE_HZ;
    const leftIndex = Math.min(source.length - 1, Math.floor(sourcePosition));
    const rightIndex = Math.min(source.length - 1, leftIndex + 1);
    const fraction = sourcePosition - leftIndex;
    target[index] = Math.max(
      -32768,
      Math.min(32767, Math.round((source[leftIndex] ?? 0) * (1 - fraction) + (source[rightIndex] ?? 0) * fraction)),
    );
  }
  return target;
}

function rejectNonSpeech(samples: Int16Array): void {
  let minimum = 32767;
  let maximum = -32768;
  let maximumAbs = 0;
  for (const sample of samples) {
    minimum = Math.min(minimum, sample);
    maximum = Math.max(maximum, sample);
    maximumAbs = Math.max(maximumAbs, Math.abs(sample));
  }

  // A zero/one-filled fake chunk is not a voice signal. Reject constant or near-silent payloads
  // at the production boundary instead of allowing them to become an audible "successful" TTS.
  if (samples.length === 0 || maximumAbs < 512 || maximum - minimum < 256) {
    throw new SpeechAudioError('non_speech_audio', 'remote TTS audio contains no plausible speech signal');
  }
}

function normalize(bytes: Uint8Array, format: RawPcmS16LeFormat): NormalizedSpeechAudio {
  validatePcmFormat(format, bytes.length);
  const samples = resampleTo16k(toMonoSamples(bytes, format), format.sample_rate_hz);
  const durationMs = (samples.length * 1_000) / SPEECH_SAMPLE_RATE_HZ;
  if (durationMs > MAX_SPEECH_DURATION_MS) {
    throw new SpeechAudioError('audio_too_long', 'remote TTS audio exceeds the 120 second safety limit');
  }
  rejectNonSpeech(samples);
  return {
    samples,
    sample_rate_hz: SPEECH_SAMPLE_RATE_HZ,
    channels: SPEECH_CHANNELS,
    bits_per_sample: SPEECH_BITS_PER_SAMPLE,
    duration_ms: durationMs,
  };
}

/** Decode WAV, or explicitly described raw PCM, into the mobile playback format. */
export function decodeRemoteTtsAudio(
  input: ArrayBuffer | Uint8Array,
  format?: RemoteTtsFormat,
): NormalizedSpeechAudio {
  const bytes = bytesOf(input);
  if (bytes.length === 0) {
    throw new SpeechAudioError('empty_audio', 'remote TTS returned an empty audio payload');
  }

  if (format === undefined) {
    if (!hasRiffWaveHeader(bytes)) {
      throw new SpeechAudioError(
        'format_required',
        'raw TTS PCM requires an explicit pcm_s16le format; refusing to guess from arbitrary bytes',
      );
    }
    const wav = parseWav(bytes);
    return normalize(wav.bytes, validateWavFormat(wav.format, wav.bytes.length));
  }

  if (format === 'wav') {
    const wav = parseWav(bytes);
    return normalize(wav.bytes, validateWavFormat(wav.format, wav.bytes.length));
  }
  return normalize(bytes, format);
}
