package com.orbmobile

import android.speech.tts.TextToSpeech
import android.util.Log
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import java.util.ArrayDeque
import java.util.Locale
import java.util.UUID

class OrbSpeechModule(private val reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext), TextToSpeech.OnInitListener {

  private data class PendingSpeech(val text: String, val promise: Promise)

  private var tts: TextToSpeech? = null
  private var ready = false
  private var initializing = false
  private val pending = ArrayDeque<PendingSpeech>()

  init {
    Log.i("OrbSpeech", "module_registered")
  }

  override fun getName(): String = "OrbSpeech"

  @ReactMethod
  fun speak(text: String, promise: Promise) {
    val trimmed = text.trim()
    if (trimmed.isEmpty()) {
      promise.resolve(null)
      return
    }
    Log.i("OrbSpeech", "speak_requested")
    if (ready) {
      speakNow(trimmed, promise)
      return
    }
    pending.add(PendingSpeech(trimmed, promise))
    if (!initializing) {
      initializing = true
      tts = TextToSpeech(reactContext.applicationContext, this)
    }
  }

  @ReactMethod
  fun stop(promise: Promise) {
    tts?.stop()
    promise.resolve(null)
  }

  override fun onInit(status: Int) {
    initializing = false
    if (status != TextToSpeech.SUCCESS) {
      val error = IllegalStateException("Android TextToSpeech init failed with status $status")
      while (pending.isNotEmpty()) {
        pending.removeFirst().promise.reject("tts_init_failed", error)
      }
      return
    }
    ready = true
    tts?.language = Locale.getDefault()
    while (pending.isNotEmpty()) {
      val next = pending.removeFirst()
      speakNow(next.text, next.promise)
    }
  }

  private fun speakNow(text: String, promise: Promise) {
    val engine = tts
    if (engine == null) {
      promise.reject("tts_unavailable", "Android TextToSpeech is not available")
      return
    }
    engine.stop()
    val result = engine.speak(text, TextToSpeech.QUEUE_FLUSH, null, UUID.randomUUID().toString())
    if (result == TextToSpeech.SUCCESS) {
      Log.i("OrbSpeech", "queued")
      promise.resolve(null)
    } else {
      promise.reject("tts_speak_failed", "Android TextToSpeech speak returned $result")
    }
  }

  override fun invalidate() {
    tts?.shutdown()
    tts = null
    ready = false
    initializing = false
    pending.clear()
    super.invalidate()
  }
}
