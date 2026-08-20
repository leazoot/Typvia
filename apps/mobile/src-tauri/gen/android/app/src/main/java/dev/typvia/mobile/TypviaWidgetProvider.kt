package dev.typvia.mobile

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.RemoteViews

/**
 * Home-screen widget: a navigation surface, not a calling
 * surface. Rows come from the KeyboardSnapshot only (normal snippets,
 * title + type mark, never body), every tap leaves for the app through
 * the two controlled typvia:// deep links, and the widget itself never
 * touches the network or records usage.
 */
class TypviaWidgetProvider : AppWidgetProvider() {

  override fun onUpdate(
    context: Context,
    manager: AppWidgetManager,
    appWidgetIds: IntArray,
  ) {
    for (widgetId in appWidgetIds) {
      manager.updateAppWidget(widgetId, buildViews(context, widgetId))
    }
  }

  private fun buildViews(context: Context, widgetId: Int): RemoteViews {
    val views = RemoteViews(context.packageName, R.layout.widget_typvia)

    val adapter =
      Intent(context, TypviaWidgetService::class.java).apply {
        putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId)
        // Distinct data per widget id so the system does not share one
        // factory across widgets with equal-looking intents.
        data = Uri.parse(toUri(Intent.URI_INTENT_SCHEME))
      }
    views.setRemoteAdapter(R.id.widget_list, adapter)
    views.setEmptyView(R.id.widget_list, R.id.widget_empty)

    // Row-tap template: action and package are fixed here, so a fill-in
    // intent can only contribute the data Uri and the intent can never
    // leave this app. MUTABLE is required for fill-in to apply at all.
    val rowTemplate = Intent(Intent.ACTION_VIEW).setPackage(context.packageName)
    views.setPendingIntentTemplate(
      R.id.widget_list,
      PendingIntent.getActivity(
        context,
        0,
        rowTemplate,
        PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
      ),
    )

    // Wordmark and empty state both land on the search-first Home.
    val search =
      Intent(Intent.ACTION_VIEW, Uri.parse("typvia://search")).setPackage(context.packageName)
    val openSearch =
      PendingIntent.getActivity(context, 1, search, PendingIntent.FLAG_IMMUTABLE)
    views.setOnClickPendingIntent(R.id.widget_header, openSearch)
    views.setOnClickPendingIntent(R.id.widget_empty, openSearch)
    return views
  }
}
