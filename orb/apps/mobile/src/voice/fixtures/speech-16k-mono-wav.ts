/** Small deterministic RIFF/WAVE fixture: 40 ms of a voiced 440 Hz test tone, 16-bit mono 16 kHz. */

const SAMPLE_RATE_HZ = 16_000;
const samples = Array.from({ length: 640 }, (_, index) =>
  Math.round(Math.sin((2 * Math.PI * 440 * index) / SAMPLE_RATE_HZ) * 8_000),
);

const bytes = new Uint8Array(44 + samples.length * 2);
const view = new DataView(bytes.buffer);
const writeAscii = (offset: number, value: string): void => {
  for (let index = 0; index < value.length; index += 1) bytes[offset + index] = value.charCodeAt(index);
};

writeAscii(0, 'RIFF');
view.setUint32(4, bytes.length - 8, true);
writeAscii(8, 'WAVE');
writeAscii(12, 'fmt ');
view.setUint32(16, 16, true);
view.setUint16(20, 1, true);
view.setUint16(22, 1, true);
view.setUint32(24, SAMPLE_RATE_HZ, true);
view.setUint32(28, SAMPLE_RATE_HZ * 2, true);
view.setUint16(32, 2, true);
view.setUint16(34, 16, true);
writeAscii(36, 'data');
view.setUint32(40, samples.length * 2, true);
samples.forEach((sample, index) => view.setInt16(44 + index * 2, sample, true));

export const SPEECH_16K_MONO_WAV = bytes;
