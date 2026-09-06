/**
 * Plays relay-synthesized TTS audio chunks over the presence bed's voice branch.
 *
 * This is the seam `App.tsx`'s `RelayClient.onSpeechAudio`/`onSpeechComplete` handlers feed:
 * raw PCM bytes arrive over the socket, get turned into a real `AudioBuffer` via the same
 * `AudioContextLike`/`AudioBufferSourceNodeLike` surface `AudioGraph.ts` already defines, and get
 * scheduled onto the persistent `voiceGain` node via `connectVoiceSource` (§2, "mixed voice
 * branch") — the same duck/restore lifecycle used by `relay.speak()` while Fish audio is in
 * flight; the audio itself always comes from the relay, never the on-device TTS engine.
 *
 * The TTS relay contract is 16-bit signed PCM, mono, at 24kHz. Mic capture remains a separate
 * 16kHz contract owned by `voice/NativeMicPort.ts`. This player still validates every
 * chunk, accepts a directly delivered WAV, and normalizes any explicitly contracted PCM before
 * it reaches the native audio graph; a provider/container mismatch therefore fails closed here.
 *
 * Scheduling is a pure function of an injected `RelayAudioContext.currentTime` clock (no
 * `Date.now()`/timers read here), so `RelayAudioPlayer`'s ordering logic is unit-testable against
 * a fake context, matching `AudioGraph.test.ts` / `PresenceBoot.test.ts`'s style.
 *
 * `handleClosing('user_pause')` (barge-in) actually stops audio: every source scheduled since the
 * last stop/`completeTurn()` is retained in `activeSources` so there is something to call
 * `.stop()` on (`connectVoiceSource` itself never retains its `source` argument), and a turn-epoch
 * counter (`turnEpoch`/`suppressedEpoch`) drops any chunk that arrives afterward but before the
 * stopped turn's epoch actually ends — relay-rs delivers a whole reply as one unpaced burst, so
 * the remainder of a just-interrupted reply keeps arriving here for a while after the stop. An
 * epoch, rather than a plain boolean, is what it takes to make a stranded-forever suppression
 * unrepresentable: a turn can end two ways (`completeTurn()`, the normal path, or
 * `handleClosing('provider_failure')`, for a turn that fails instead), and only an epoch that
 * advances on *either* boundary guarantees a later turn is never blocked by a stop whose own turn
 * never reached `completeTurn()`. See `handleClosing`'s and `turnEpoch`'s docstrings for the full
 * reasoning, and `BargeInPlaybackLatency.test.ts` for the measurement this is graded against
 * (100ms budget) and the stranding scenario this exists to prevent.
 */

import {
  connectVoiceSource,
  type AudioBufferSourceNodeLike,
  type AudioContextLike,
  type GainNodeLike,
} from '../presence/AudioGraph';
import type { AudioBufferLike, PresenceAudioContext } from '../presence/PresenceBoot';
import { noopDevLogger, type DevLogger } from '../shared/DevLogger';
import {
  decodeRemoteTtsAudio,
  SPEECH_SAMPLE_RATE_HZ,
  SpeechAudioError,
  type NormalizedSpeechAudio,
} from './SpeechAudio';
import { speakFailure } from './SpeechFailure';
import type { SpeechPort } from './SpeechPort';

/** TTS playback format — 16-bit PCM, mono, 24kHz. Mic frames use a separate 16kHz contract. */
export const RELAY_AUDIO_SAMPLE_RATE_HZ = SPEECH_SAMPLE_RATE_HZ;
const PCM16_FULL_SCALE = 32_768;
const BYTES_PER_SAMPLE = 2;

/**
 * The context surface this player needs: `AudioContextLike`'s node factories plus `currentTime`
 * (to schedule chunks back-to-back) and `createBuffer` (to build a real `AudioBuffer` from PCM
 * bytes) — the same pair `PresenceBoot.ts` already requires of its context.
 */
export type RelayAudioContext = PresenceAudioContext;

/**
 * Converts one PCM chunk into a mono `AudioBuffer`-shaped object via `ctx.createBuffer`, matching
 * `PresenceBoot.bootPresenceBed`'s existing float-conversion pattern (`sample / INT16_MAX`).
 * Pure given `ctx`: no clock, no randomness, no side effects beyond the one `createBuffer` call.
 */
export function pcmToAudioBuffer(
  ctx: RelayAudioContext,
  pcm: ArrayBuffer,
  sampleRateHz: number = RELAY_AUDIO_SAMPLE_RATE_HZ,
): AudioBufferLike {
  const samples = new Int16Array(pcm);
  const buffer = ctx.createBuffer(1, samples.length, sampleRateHz);
  const channel = buffer.getChannelData(0);
  for (let i = 0; i < samples.length; i++) {
    channel[i] = (samples[i] ?? 0) / PCM16_FULL_SCALE;
  }
  return buffer;
}

function normalizedToAudioBuffer(ctx: RelayAudioContext, audio: NormalizedSpeechAudio): AudioBufferLike {
  // react-native-audio-api's native mixer expects buffers at the context's output rate. Passing
  // 16 kHz samples to a 24/48 kHz context without changing the sample count changes the pitch and
  // makes Fish TTS sound slow, mumbled, or otherwise unintelligible on-device. Resample while
  // preserving duration, then label the buffer with the actual native rate.
  const targetRateHz = ctx.sampleRate;
  const samples = resampleSamples(audio.samples, audio.sample_rate_hz, targetRateHz);
  const buffer = ctx.createBuffer(audio.channels, samples.length, targetRateHz);
  const channel = buffer.getChannelData(0);
  for (let i = 0; i < samples.length; i += 1) {
    channel[i] = (samples[i] ?? 0) / PCM16_FULL_SCALE;
  }
  return buffer;
}

/** Linear PCM resampler used only at the native playback boundary. */
export function resampleSamples(samples: Int16Array, sourceRateHz: number, targetRateHz: number): Int16Array {
  if (samples.length === 0 || sourceRateHz === targetRateHz) return samples;
  if (!Number.isFinite(sourceRateHz) || sourceRateHz <= 0 || !Number.isFinite(targetRateHz) || targetRateHz <= 0) {
    throw new Error('Audio sample rates must be positive finite numbers');
  }

  const targetLength = Math.max(1, Math.round((samples.length * targetRateHz) / sourceRateHz));
  const target = new Int16Array(targetLength);
  for (let index = 0; index < target.length; index += 1) {
    const sourcePosition = (index * sourceRateHz) / targetRateHz;
    const leftIndex = Math.min(samples.length - 1, Math.floor(sourcePosition));
    const rightIndex = Math.min(samples.length - 1, leftIndex + 1);
    const fraction = sourcePosition - leftIndex;
    target[index] = Math.max(
      -32768,
      Math.min(32767, Math.round((samples[leftIndex] ?? 0) * (1 - fraction) + (samples[rightIndex] ?? 0) * fraction)),
    );
  }
  return target;
}

/** How many seconds of audio a raw 16-bit mono PCM chunk of `byteLength` bytes represents. */
export function pcmChunkDurationSeconds(
  byteLength: number,
  sampleRateHz: number = RELAY_AUDIO_SAMPLE_RATE_HZ,
): number {
  return byteLength / BYTES_PER_SAMPLE / sampleRateHz;
}

/**
 * Queues and schedules relay-delivered TTS PCM chunks onto the presence bed's voice branch,
 * back-to-back, so a multi-chunk synthesis turn plays as continuous speech instead of overlapping
 * or gapping. One instance covers one voice branch (`voiceGain`); the caller (`App.tsx`) owns its
 * lifetime alongside the presence bed.
 */
export class RelayAudioPlayer {
  private nextStartAtSeconds: number | null = null;
  private playbackEndAtSeconds: number | null = null;
  private turnHasAudio = false;
  private turnSawSilentChunk = false;
  private turnChunkCount = 0;
  private turnByteCount = 0;
  private turnSilentChunkCount = 0;
  private turnDurationMs = 0;
  /** Every source scheduled since the last stop/completeTurn — retained only so a barge-in stop
   *  has something to call `.stop()` on. Cleared (not stopped) in `completeTurn()` too, so a long
   *  session without a single barge-in never accumulates references across turns. */
  private activeSources: AudioBufferSourceNodeLike[] = [];
  /**
   * Increments on every boundary that conclusively ends a turn: `completeTurn()` (the normal
   * path) and `handleClosing('provider_failure')` (a turn/connection that ends abnormally and
   * will never get a `completeTurn()` call — see `handleClosing`'s docstring). A `user_pause` stop
   * does NOT advance this on its own — it only records which epoch it stopped, in
   * `suppressedEpoch` — because advancing it immediately would let through the exact leftover
   * bytes of the just-stopped turn that this mechanism exists to drop.
   */
  private turnEpoch = 0;
  /** The epoch `enqueueChunk` should keep dropping chunks for, or `null` once nothing is
   *  suppressed. Suppression is in effect exactly while `suppressedEpoch === turnEpoch`; once
   *  either `completeTurn()` or a `provider_failure` advances `turnEpoch`, the comparison stops
   *  matching on its own — a genuinely new turn is never blocked by a stale stop. */
  private suppressedEpoch: number | null = null;
  private readonly announcedFailures = new Set<'provider_failure' | 'invalid_tts_audio' | 'tts_playback_failure'>();

  constructor(
    private readonly ctx: RelayAudioContext,
    private readonly voiceGain: GainNodeLike,
    private readonly sampleRateHz: number = RELAY_AUDIO_SAMPLE_RATE_HZ,
    private readonly speechPort?: SpeechPort,
    private readonly devLogger: DevLogger = noopDevLogger,
  ) {}

  /**
   * Schedules one chunk to start immediately after the previous chunk in this turn (or "now" if
   * this is the first chunk since boot or since the last `completeTurn`/gap). Empty chunks are
   * dropped rather than scheduling a zero-length source.
   */
  enqueueChunk(pcm: ArrayBuffer): void {
    if (pcm.byteLength === 0) return;
    // relay-rs sends a whole reply as one unpaced burst; bytes belonging to a turn that was just
    // stopped keep arriving here for a while. Drop them rather than schedule them — a stop that
    // only cancels what's already scheduled and lets the next chunk through is not a stop. Scoped
    // to the epoch that was actually stopped (see `turnEpoch`'s docstring) rather than a flat
    // boolean, so a turn that ends abnormally (never reaching `completeTurn()`) cannot strand this
    // check in the "suppress everything forever" state.
    if (this.suppressedEpoch === this.turnEpoch) {
      this.devLogger.log('relay_audio.chunk_suppressed_after_stop', { chunk_bytes: pcm.byteLength });
      return;
    }
    try {
      // The sidecar normally strips WAV containers before chunking. Auto-detecting RIFF here also
      // keeps the client boundary safe if a provider response bypasses that normalizer. Raw PCM is
      // accepted only under the explicit relay contract; arbitrary bytes are never guessed as voice.
      const bytes = new Uint8Array(pcm);
      const looksLikeWav = bytes.length >= 12 && String.fromCharCode(...bytes.subarray(0, 4)) === 'RIFF';
      const audio = decodeRemoteTtsAudio(
        pcm,
        looksLikeWav
          ? undefined
          : { kind: 'pcm_s16le', sample_rate_hz: this.sampleRateHz, channels: 1 },
      );
      const now = this.ctx.currentTime;
      const startAtSeconds =
        this.nextStartAtSeconds !== null && this.nextStartAtSeconds > now ? this.nextStartAtSeconds : now;

      const buffer = normalizedToAudioBuffer(this.ctx, audio);
      const source = this.ctx.createBufferSource();
      source.buffer = buffer;
      source.loop = false;
      connectVoiceSource(this.voiceGain, source, startAtSeconds);
      this.activeSources.push(source);

      const durationSeconds = pcmChunkDurationSeconds(audio.samples.byteLength, audio.sample_rate_hz);
      this.nextStartAtSeconds = startAtSeconds + durationSeconds;
      this.playbackEndAtSeconds = this.nextStartAtSeconds;
      this.turnHasAudio = true;
      this.turnChunkCount += 1;
      this.turnByteCount += pcm.byteLength;
      this.turnDurationMs += audio.duration_ms;
    } catch (error) {
      // Fish streams can end with a short low-energy/silent chunk. It is valid padding, not a
      // broken response; dropping it avoids a false spoken error after an otherwise clear reply.
      // A turn containing only such chunks is still rejected in completeTurn().
      if (error instanceof SpeechAudioError && error.code === 'non_speech_audio') {
        this.turnSawSilentChunk = true;
        this.turnSilentChunkCount += 1;
        this.turnByteCount += pcm.byteLength;
        return;
      }
      this.devLogger.log(
        'relay_audio.playback_failed',
        { chunk_bytes: pcm.byteLength, error: error instanceof Error ? error.message : String(error) },
        'error',
      );
      this.announceFailure(error instanceof SpeechAudioError ? 'invalid_tts_audio' : 'tts_playback_failure');
    }
  }

  /**
   * `'provider_failure'` speaks a one-time apology (unchanged) AND advances `turnEpoch` — a
   * provider failure closes the relay connection (`session.rs`'s `fail()` returns the same
   * `Action::SendAndClose` a session pause does; `main.rs`'s `close_after` flag drives an
   * identical `break 'connection'` either way), so whatever turn was in flight will never reach
   * `completeTurn()` — no `speech_complete` frame is coming. Without advancing the epoch here, a
   * turn that was interrupted and then failed (rather than completing normally) would leave
   * `enqueueChunk`'s suppression permanently on, silently dropping every later chunk on this
   * player instance: "no reply, ever, until app restart." Advancing the epoch on a plain failure
   * that was never interrupted is a harmless no-op (there is nothing suppressed to lift).
   *
   * `'user_pause'` is the barge-in stop: it must actually cut audio, not merely log intent. Every
   * source retained since the last stop/completeTurn gets a `.stop()` call at "now", regardless of
   * whether its own scheduled window has already elapsed — cheaper and safer than tracking each
   * source's own end time just to skip an already-finished one (mirrors `AudioGraph.shutdown()`'s
   * unconditional `bedSource.stop?.()`). `playbackEndAtSeconds`/`nextStartAtSeconds` are cleared so
   * `isPlaying()` reflects the cut immediately, and `suppressedEpoch` is set to the CURRENT
   * `turnEpoch` — deliberately not advancing it — so whatever of this same turn's bytes still
   * arrive keep getting dropped until a later boundary (`completeTurn()`, or a subsequent
   * `provider_failure`) moves `turnEpoch` past it. See `turnEpoch`'s field docstring and
   * `BargeInPlaybackLatency.test.ts`'s "does not survive past the turn that was actually
   * interrupted" suite for the exact scenario this exists to prevent.
   *
   * Residual, narrower gap (disclosed rather than silently accepted): two separate `'user_pause'`
   * calls in a row — e.g. a barge-in stop followed later by the relay's own `closing` frame also
   * carrying `reason: 'user_pause'` (the explicit session-pause contract, a different event from
   * barge-in that happens to share this method and this exact string) — are indistinguishable from
   * an idempotent double-stop from inside this method: both look like "handleClosing('user_pause')
   * called again while already suppressed." Idempotency requires the second call NOT advance the
   * epoch; that same requirement means a genuinely-separate second `user_pause` event cannot lift
   * the suppression either. Closing that fully would need `App.tsx` to pass more information than
   * a repeated literal (e.g. a distinct reason, or routing the session-pause close through its own
   * method) — out of scope here per the brief (stay inside `apps/mobile/src/voice/`). It is also
   * unreachable today by the same verified fact used above: that path also closes the socket, and
   * this app has no reconnect logic (the socket effect's dependency array never changes after
   * mount; `onClosing` leaves `relayStatus` at `'closed'` with nothing transitioning it back), so
   * there is no future chunk for a stale suppression to wrongly drop.
   */
  handleClosing(reason: 'user_pause' | 'provider_failure'): void {
    if (reason === 'provider_failure') {
      this.announceFailure('provider_failure');
      this.turnEpoch += 1;
      this.suppressedEpoch = null;
      return;
    }
    const stopAtSeconds = this.ctx.currentTime;
    for (const source of this.activeSources) {
      source.stop?.(stopAtSeconds);
    }
    this.activeSources = [];
    this.nextStartAtSeconds = null;
    this.playbackEndAtSeconds = null;
    this.suppressedEpoch = this.turnEpoch;
  }

  /** True while a scheduled relay response can still be heard on the voice branch. */
  isPlaying(): boolean {
    return this.playbackEndAtSeconds !== null && this.ctx.currentTime < this.playbackEndAtSeconds;
  }

  private announceFailure(failure: 'provider_failure' | 'invalid_tts_audio' | 'tts_playback_failure'): void {
    if (this.announcedFailures.has(failure)) return;
    this.announcedFailures.add(failure);
    void speakFailure(this.speechPort, failure);
  }

  /**
   * Marks the end of a synthesis turn: the next `enqueueChunk` (a new turn) schedules from
   * "now" rather than chaining onto wherever the last turn's audio ended.
   *
   * Also releases `activeSources` (whether or not this turn was ever stopped — a normal,
   * uninterrupted turn must not accumulate source references forever across a whole session) and
   * advances `turnEpoch`, which lifts any post-stop suppression left by `handleClosing`: this is
   * the *normal* way a turn's epoch ends (the other is `handleClosing('provider_failure')`, for a
   * turn that ends abnormally instead — see its docstring). relay-rs's synchronous
   * synthesize-then-send means a genuinely new turn's audio cannot start arriving before the old
   * turn's `speech_complete` — and therefore this call — has already fired, so the very next
   * `enqueueChunk` after a normal completion is guaranteed to be a new turn, never a stale tail of
   * the one just closed.
   */
  completeTurn(): void {
    if (!this.turnHasAudio && this.turnSawSilentChunk) this.announceFailure('invalid_tts_audio');
    if (this.turnChunkCount > 0 || this.turnSilentChunkCount > 0) {
      this.devLogger.log('relay_audio.playback_summary', {
        chunks: this.turnChunkCount,
        bytes: this.turnByteCount,
        silent_chunks: this.turnSilentChunkCount,
        duration_ms: Math.round(this.turnDurationMs),
      });
    }
    this.nextStartAtSeconds = null;
    this.turnHasAudio = false;
    this.turnSawSilentChunk = false;
    this.turnChunkCount = 0;
    this.turnByteCount = 0;
    this.turnSilentChunkCount = 0;
    this.turnDurationMs = 0;
    this.activeSources = [];
    this.turnEpoch += 1;
    this.suppressedEpoch = null;
  }
}

// Re-exported only for call sites that need the narrower `AudioContextLike`/node types without
// pulling in the presence-bed-specific `createBuffer` surface.
export type { AudioContextLike, GainNodeLike };
