package com.orbmobile

import android.content.Context
import android.os.SystemClock
import android.speech.tts.TextToSpeech
import android.util.Log
import java.util.Locale
import java.util.UUID

object OrbGreeting : TextToSpeech.OnInitListener {
  private const val TAG = "OrbGreeting"
  private const val FIRST_STEP = "Open the first small action"
  private var tts: TextToSpeech? = null
  private var pending = false
  private var ready = false

  @Synchronized
  fun prewarm(context: Context) {
    if (tts != null) return
    tts = TextToSpeech(context.applicationContext, this)
  }

  @Synchronized
  fun request(context: Context, activityStartedAtMs: Long) {
    if (pending) return
    Log.i(TAG, "speech_request elapsed_ms=${SystemClock.elapsedRealtime() - activityStartedAtMs}")
    pending = true
    if (tts == null) {
      tts = TextToSpeech(context.applicationContext, this)
      return
    }
    if (ready) speakLocked()
  }

  @Synchronized
  override fun onInit(status: Int) {
    if (status != TextToSpeech.SUCCESS) {
      Log.e(TAG, "init_failed status=$status")
      pending = false
      ready = false
      return
    }
    ready = true
    if (pending) speakLocked()
  }

  private fun speakLocked() {
    val engine = tts ?: return
    engine.language = Locale.getDefault()
    val result = engine.speak(FIRST_STEP, TextToSpeech.QUEUE_FLUSH, null, UUID.randomUUID().toString())
    if (result == TextToSpeech.SUCCESS) {
      Log.i(TAG, "queued")
    } else {
      Log.e(TAG, "speak_failed result=$result")
    }
    pending = false
  }
}
