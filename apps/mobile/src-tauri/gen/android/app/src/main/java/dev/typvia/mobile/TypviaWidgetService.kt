package dev.typvia.mobile

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.RemoteViews
import android.widget.RemoteViewsService
import dev.typvia.ime.SnapshotStore
import dev.typvia.ime.TypeMark
import java.io.File
import uniffi.typvia_mobile_ffi.SnapshotEntry
import uniffi.typvia_mobile_ffi.SnapshotException
import uniffi.typvia_mobile_ffi.widgetEntries

/**
 * List factory behind the widget's rows. Reads the KeyboardSnapshot the
 * host already writes (same file the IME reads — zero new data surface)
 * and takes its rows from the Rust-side `widgetEntries` selection, so the
 * ordering/content rules (favorites first, sensitive excluded entirely)
 * have exactly one implementation. Every degraded case — no file yet,
 * unreadable bytes, failed validation — renders as the same empty state,
 * and errors are deliberately not logged (snapshot-adjacent messages could
 * reach logcat; security.md extension rules).
 */
class TypviaWidgetService : RemoteViewsService() {
  override fun onGetViewFactory(intent: Intent): RemoteViewsFactory =
    WidgetRowsFactory(applicationContext)
}

private class WidgetRowsFactory(
  private val context: Context,
) : RemoteViewsService.RemoteViewsFactory {

  private var rows: List<SnapshotEntry> = emptyList()

  override fun onCreate() {}

  override fun onDataSetChanged() {
    rows =
      try {
        val json = File(context.dataDir, SnapshotStore.RELATIVE_PATH).readText()
        widgetEntries(json, ROW_LIMIT)
      } catch (error: java.io.IOException) {
        emptyList()
      } catch (error: SnapshotException) {
        emptyList()
      }
  }

  override fun getViewAt(position: Int): RemoteViews {
    val views = RemoteViews(context.packageName, R.layout.widget_row)
    val row = rows.getOrNull(position) ?: return views
    val mark = TypeMark.mark(row.snippetType)
    views.setTextViewText(R.id.widget_row_mark, mark)
    views.setTextViewText(R.id.widget_row_title, row.title)
    // Screen readers get the type as a word, never the two-letter mark.
    views.setContentDescription(
      R.id.widget_row_root,
      "${TypeMark.word(mark)}, ${row.title}",
    )
    views.setOnClickFillInIntent(
      R.id.widget_row_root,
      Intent().setData(Uri.parse("typvia://snippet/${row.id}")),
    )
    return views
  }

  override fun getCount(): Int = rows.size

  override fun getLoadingView(): RemoteViews? = null

  override fun getViewTypeCount(): Int = 1

  override fun getItemId(position: Int): Long = position.toLong()

  override fun hasStableIds(): Boolean = false

  override fun onDestroy() {
    rows = emptyList()
  }

  private companion object {
    /** Largest size tier shows 8 rows. */
    val ROW_LIMIT = 8u
  }
}
