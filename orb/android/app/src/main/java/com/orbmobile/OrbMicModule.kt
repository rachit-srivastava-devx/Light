package com.orbmobile

import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.SystemClock
import android.util.Base64
import android.util.Log
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.modules.core.DeviceEventManagerModule.RCTDeviceEventEmitter
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Native mic capture: 16kHz mono 16-bit PCM via `AudioRecord`, streamed to JS as base64-encoded
 * chunks over `DeviceEventEmitter`. Mirrors `OrbSpeechModule`'s shape (a small
 * `ReactContextBaseJavaModule`, no view manager, registered via its own `ReactPackage`).
 *
 * This module owns capture only — VAD scoring, base64 decode, and relay wiring live in JS
 * (`apps/mobile/src/voice/NativeMicPort.ts`) so the framing contract (`VadFrame`) stays a single
 * source of truth.
 */
class OrbMicModule(private val reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

  companion object {
    private const val SAMPLE_RATE_HZ = 16_000
    private const val CHANNEL_CONFIG = AudioFormat.CHANNEL_IN_MONO
    private const val AUDIO_FORMAT = AudioFormat.ENCODING_PCM_16BIT
    // ~20ms at 16kHz/16-bit/mono = 16_000 * 0.02 * 2 bytes = 640 bytes.
    private const val CHUNK_BYTES = 640
    private const val LOG_EVERY_N_FRAMES = 50
    private const val EVENT_NAME = "OrbMicFrame"
  }

  private var audioRecord: AudioRecord? = null
  private var captureThread: Thread? = null
  private val recording = AtomicBoolean(false)

  init {
    Log.i("OrbMic", "module_registered")
  }

  override fun getName(): String = "OrbMic"

  private fun openAudioRecord(): AudioRecord {
    val minBufferBytes =
        AudioRecord.getMinBufferSize(SAMPLE_RATE_HZ, CHANNEL_CONFIG, AUDIO_FORMAT)
    require(minBufferBytes > 0) { "AudioRecord.getMinBufferSize returned $minBufferBytes" }
    val bufferBytes = maxOf(minBufferBytes, CHUNK_BYTES * 4)

    // VOICE_RECOGNITION applies AGC/noise-suppression tuned for speech, but is not guaranteed
    // present on every emulator image; MIC always is. Probe by attempting to initialize each
    // source in turn rather than assuming.
    val sources = intArrayOf(MediaRecorder.AudioSource.VOICE_RECOGNITION, MediaRecorder.AudioSource.MIC)
    var lastError: Exception? = null
    for (source in sources) {
      try {
        val record =
            AudioRecord(source, SAMPLE_RATE_HZ, CHANNEL_CONFIG, AUDIO_FORMAT, bufferBytes)
        if (record.state == AudioRecord.STATE_INITIALIZED) {
          Log.i("OrbMic", "audio_source_selected source=$source")
          return record
        }
        record.release()
      } catch (error: Exception) {
        lastError = error
      }
    }
    throw IllegalStateException("Unable to initialize AudioRecord with any known source", lastError)
  }

  @ReactMethod
  fun start(promise: Promise) {
    if (recording.get()) {
      promise.resolve(null)
      return
    }
    val record: AudioRecord
    try {
      record = openAudioRecord()
    } catch (error: Exception) {
      Log.e("OrbMic", "start_failed ${error.message}")
      promise.reject("mic_start_failed", error)
      return
    }

    audioRecord = record
    recording.set(true)
    record.startRecording()
    Log.i("OrbMic", "start_recording")

    val thread =
        Thread {
          val buffer = ByteArray(CHUNK_BYTES)
          var frameCount = 0L
          while (recording.get()) {
            val bytesRead = record.read(buffer, 0, buffer.size)
            if (bytesRead <= 0) continue

            val payload = if (bytesRead == buffer.size) buffer else buffer.copyOf(bytesRead)
            val base64Audio = Base64.encodeToString(payload, Base64.NO_WRAP)
            val atMs = SystemClock.elapsedRealtime()

            val event = Arguments.createMap()
            event.putString("audioBase64", base64Audio)
            event.putDouble("atMs", atMs.toDouble())
            event.putInt("byteLength", bytesRead)

            try {
              reactContext.getJSModule(RCTDeviceEventEmitter::class.java).emit(EVENT_NAME, event)
            } catch (error: Exception) {
              Log.e("OrbMic", "emit_failed ${error.message}")
            }

            frameCount += 1
            if (frameCount % LOG_EVERY_N_FRAMES == 0L) {
              Log.i("OrbMic", "frames_captured count=$frameCount last_bytes=$bytesRead")
            }
          }
        }
    thread.name = "OrbMicCaptureThread"
    captureThread = thread
    thread.start()
    promise.resolve(null)
  }

  @ReactMethod
  fun stop(promise: Promise) {
    if (!recording.getAndSet(false)) {
      promise.resolve(null)
      return
    }
    try {
      captureThread?.join(1_000)
    } catch (error: InterruptedException) {
      Thread.currentThread().interrupt()
    }
    captureThread = null

    val record = audioRecord
    audioRecord = null
    if (record != null) {
      try {
        record.stop()
      } catch (error: IllegalStateException) {
        // Already stopped or never started successfully — release is still safe below.
      }
      record.release()
    }
    Log.i("OrbMic", "stop_recording")
    promise.resolve(null)
  }

  override fun invalidate() {
    recording.set(false)
    try {
      captureThread?.join(1_000)
    } catch (error: InterruptedException) {
      Thread.currentThread().interrupt()
    }
    captureThread = null
    audioRecord?.let {
      try {
        it.stop()
      } catch (error: IllegalStateException) {
        // no-op
      }
      it.release()
    }
    audioRecord = null
    super.invalidate()
  }
}
