package com.orbmobile

import android.app.Application
import android.os.SystemClock
import com.facebook.react.PackageList
import com.facebook.react.ReactApplication
import com.facebook.react.ReactHost
import com.facebook.react.ReactNativeApplicationEntryPoint.loadReactNative
import com.facebook.react.defaults.DefaultReactHost.getDefaultReactHost

class MainApplication : Application(), ReactApplication {

  override val reactHost: ReactHost by lazy {
    getDefaultReactHost(
      context = applicationContext,
      packageList =
        PackageList(this).packages.apply {
          // Packages that cannot be autolinked yet can be added manually here, for example:
          add(OrbSpeechPackage())
          add(OrbMicPackage())
        },
    )
  }

  override fun onCreate() {
    val appStartedAtMs = SystemClock.elapsedRealtime()
    super.onCreate()
    OrbPresence.start(appStartedAtMs)
    OrbGreeting.prewarm(this)
    loadReactNative(this)
  }
}
