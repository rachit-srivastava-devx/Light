package com.orbmobile

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.SystemClock
import android.util.Log
import kotlin.math.max
import kotlin.math.min

object OrbPresence {
  private const val TAG = "OrbPresence"
  private const val SAMPLE_RATE_HZ = 48000
  private const val BUFFER_SECONDS = 1
  private const val GAIN = 0.035
  private var track: AudioTrack? = null
  private var startedAtMs: Long? = null

  @Synchronized
  fun start(appStartedAtMs: Long) {
    if (track != null) return
    try {
      val frameCount = SAMPLE_RATE_HZ * BUFFER_SECONDS
      val pcm = buildBrownNoise(frameCount)
      val audioTrack =
          AudioTrack.Builder()
              .setAudioAttributes(
                  AudioAttributes.Builder()
                      .setUsage(AudioAttributes.USAGE_MEDIA)
                      .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                      .build())
              .setAudioFormat(
                  AudioFormat.Builder()
                      .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                      .setSampleRate(SAMPLE_RATE_HZ)
                      .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                      .build())
              .setTransferMode(AudioTrack.MODE_STATIC)
              .setBufferSizeInBytes(pcm.size * Short.SIZE_BYTES)
              .build()

      audioTrack.write(pcm, 0, pcm.size)
      audioTrack.setLoopPoints(0, frameCount, -1)
      audioTrack.play()
      track = audioTrack
      startedAtMs = SystemClock.elapsedRealtime()
      Log.i(TAG, "started elapsed_ms=${SystemClock.elapsedRealtime() - appStartedAtMs}")
    } catch (error: RuntimeException) {
      Log.e(TAG, "start_failed ${error.message}", error)
    }
  }

  fun logActivityReady(activityStartedAtMs: Long) {
    val started = startedAtMs
    val state = if (track?.playState == AudioTrack.PLAYSTATE_PLAYING) "playing" else "not_playing"
    val elapsedMs = if (started != null && started <= activityStartedAtMs) 0 else SystemClock.elapsedRealtime() - activityStartedAtMs
    Log.i(TAG, "activity_ready state=$state elapsed_ms=$elapsedMs")
  }

  private fun buildBrownNoise(frameCount: Int): ShortArray {
    val out = ShortArray(frameCount)
    var randomState = 42u
    var y = 0.0
    for (i in 0 until frameCount) {
      randomState += 0x6D2B79F5u
      var t = randomState
      t = (t xor (t shr 15)) * (t or 1u)
      t = t xor (t + ((t xor (t shr 7)) * (t or 61u)))
      val white = (((t xor (t shr 14)).toDouble() / UInt.MAX_VALUE.toDouble()) * 2.0) - 1.0
      y = max(-1.0, min(1.0, y * 0.998 + white * 0.02))
      out[i] = (y * Short.MAX_VALUE * GAIN).toInt().toShort()
    }
    return out
  }
}
