package dev.typvia.mobile

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.TileService

/**
 * Quick Settings tile: a pure entry point. The tile carries no
 * snippet content, count, or state — tapping it opens the app, whose first
 * screen is the search surface ("open" is "search"). A locked device goes
 * through the system's unlock discipline before anything opens.
 */
class TypviaTileService : TileService() {

  override fun onClick() {
    if (isLocked) {
      unlockAndRun { open() }
    } else {
      open()
    }
  }

  private fun open() {
    val intent =
      Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
      // API 34 removed the Intent overload's effect; the PendingIntent
      // variant is the supported path.
      startActivityAndCollapse(
        PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE)
      )
    } else {
      @Suppress("DEPRECATION") startActivityAndCollapse(intent)
    }
  }
}
