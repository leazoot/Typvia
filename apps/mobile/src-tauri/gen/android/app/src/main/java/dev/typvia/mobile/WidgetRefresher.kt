package dev.typvia.mobile

import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context

/**
 * Event-driven widget refresh: the Rust host calls this over
 * JNI right after a successful KeyboardSnapshot write, so widgets follow
 * data changes without any polling timeline. Called best-effort — a failed
 * poke must never fail the snapshot write behind it.
 */
object WidgetRefresher {

  @JvmStatic
  fun refresh(context: Context) {
    val manager = AppWidgetManager.getInstance(context) ?: return
    val ids =
      manager.getAppWidgetIds(ComponentName(context, TypviaWidgetProvider::class.java))
    if (ids.isEmpty()) return
    manager.notifyAppWidgetViewDataChanged(ids, R.id.widget_list)
  }
}
