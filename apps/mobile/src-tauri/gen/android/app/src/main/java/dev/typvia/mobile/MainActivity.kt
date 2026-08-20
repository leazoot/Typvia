package dev.typvia.mobile

import android.os.Bundle
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.annotation.Keep
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import java.util.concurrent.TimeUnit

/**
 * Android host glue for the shared SPA. System-API concerns only,
 * never business rules:
 *  - safe-area insets exposed to the WebView, because the Android WebView
 *    reports env(safe-area-inset-*) as 0 while the activity is edge-to-edge;
 *  - soft-keyboard resize, because edge-to-edge opts the window out of
 *    adjustResize, so the IME inset is re-applied as content padding;
 *  - system back forwarded to the SPA first, because TauriActivity registers
 *    no back callback (handleBackNavigation = false) and back would
 *    otherwise background the task even with an editor or popover open.
 */
class MainActivity : TauriActivity() {
  private var webView: WebView? = null

  // Written on the main thread by the insets listener, read from the
  // WebView's JS thread through the TypviaInsets bridge.
  @Volatile
  private var safeAreaCssPx: String = "0,0"

  inner class InsetsBridge {
    /** System-bar + cutout insets as CSS px: "top,bottom". */
    @JavascriptInterface
    fun cssPx(): String = safeAreaCssPx
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Background sync: best-effort periodic round at the platform
    // minimum interval, network-gated; KEEP so re-launches never reset the
    // schedule, and WorkManager's default exponential backoff covers the
    // retry protocol (SyncWorker).
    WorkManager.getInstance(this).enqueueUniquePeriodicWork(
      "typvia-background-sync",
      ExistingPeriodicWorkPolicy.KEEP,
      PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
        .setConstraints(
          Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()
        )
        .build()
    )

    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      val density = resources.displayMetrics.density
      safeAreaCssPx = "${bars.top / density},${bars.bottom / density}"
      // Edge-to-edge ignores windowSoftInputMode=adjustResize; padding the
      // content by the IME inset restores it, so the WebView (and with it
      // the page's layout viewport) shrinks above the keyboard. The page
      // keeps its own CSS bottom inset while the keyboard is up — a small
      // padding surplus, never occlusion.
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(0, 0, 0, ime.bottom)
      // Push-notify the page (rotation, nav-mode change); a fresh page load
      // pulls the cached value itself through the bridge instead.
      webView?.evaluateJavascript(
        "window.__TYPVIA_INSETS_CHANGED__ && window.__TYPVIA_INSETS_CHANGED__()",
        null
      )
      insets
    }

    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val view = webView
        if (view == null) {
          runDefaultBack()
          return
        }
        // The SPA installs the hook (mobile difference layer, back-stack.ts)
        // and answers true when it consumed the gesture (editor cancel,
        // popover close). Anything else falls through to the default back.
        view.evaluateJavascript(
          "typeof window.__TYPVIA_ANDROID_BACK__ === 'function' && window.__TYPVIA_ANDROID_BACK__()"
        ) { consumed -> if (consumed != "true") runDefaultBack() }
      }

      private fun runDefaultBack() {
        isEnabled = false
        onBackPressedDispatcher.onBackPressed()
        isEnabled = true
      }
    })
  }

  override fun onWebViewCreate(webView: WebView) {
    this.webView = webView
    // Pull-based so a page reload re-reads the values without waiting for an
    // inset dispatch; the bridge exposes inset numbers only.
    webView.addJavascriptInterface(InsetsBridge(), "TypviaInsets")
  }

  // --- Vault secure store -------------------------------------------------
  // Called from Rust over JNI on a Tauri blocking thread (never the main
  // thread); names and signatures are the Rust<->Kotlin contract mirrored in
  // src/secure_store.rs. @Keep guards the reflective JNI entry points from
  // release minification.

  // `gated` is the Rust classification (secure_store.rs needs_user_presence):
  // true only for the vault master-key copy, false for the sync key material
  // that must stay readable without a prompt.

  @Keep
  fun typviaSecureStoreStore(entry: String, secret: ByteArray, gated: Boolean): Int =
    TypviaSecureStore.store(this, entry, secret, gated)

  @Keep
  fun typviaSecureStoreRetrieve(entry: String, requestId: Long, gated: Boolean): Int =
    TypviaSecureStore.retrieve(this, entry, requestId, gated)

  @Keep
  fun typviaSecureStoreRemove(entry: String): Int =
    TypviaSecureStore.remove(this, entry)

  // --- Widget refresh poke ------------------------------------------------
  // Same JNI calling convention as the secure-store bridge above: called from
  // Rust right after a successful KeyboardSnapshot write. Best-effort by
  // contract — the caller drops any failure.

  @Keep
  fun typviaWidgetRefresh() = WidgetRefresher.refresh(this)

  // --- Sensitive clipboard copy -------------------------------------------
  // Same JNI calling convention as the secure-store bridge above. The single
  // Kotlin write schedules the auto-clear itself; Rust has no set-without-
  // clear entry point (clipboard red line).

  @Keep
  fun typviaSensitiveClipboardCopy(secret: String, clearMs: Long): Int =
    TypviaSensitiveClipboard.copy(this, secret, clearMs)

  // --- Ordinary clipboard copy --------------------------------------------
  // The system WebView has no navigator.clipboard on this origin, so a normal
  // snippet copy goes through the host too. Separate method from the
  // sensitive one on purpose: no auto-clear belongs on a normal body, and a
  // secret must never reach this path.

  @Keep
  fun typviaClipboardCopy(text: String): Int =
    TypviaClipboard.copy(this, text)

  override fun onResume() {
    super.onResume()
    // Sweep a sensitive clip whose expiry the OS denied while the app was
    // backgrounded (clipboard focus gate) or whose countdown died with the
    // process; a live countdown window is never cut short (clearStaleClip).
    TypviaSensitiveClipboard.clearStaleClip(this)
  }
}
