package dev.typvia.mobile

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Handler
import android.os.Looper
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * Ordinary (non-sensitive) clipboard copy for the mobile host.
 *
 * The WebView's `navigator.clipboard` is unavailable on the Android system
 * WebView the app is served through (no secure origin), so the copy of a
 * normal snippet goes through the host like every other write: the Rust side
 * reads the body from the core, calls this method, and records usage only
 * after a confirmed write.
 *
 * This is deliberately NOT the sensitive path. A secret must never reach the
 * clipboard without its auto-clear, and that guarantee lives structurally in
 * TypviaSensitiveClipboard.copy; this method carries no clear and no
 * sensitive marker, so the Rust caller (pasteboard.rs) only ever hands it a
 * plaintext, non-sensitive snippet body.
 *
 * Threading: called over JNI from a Rust blocking thread; the
 * ClipboardManager write is posted to the main looper and the calling thread
 * parks on its completion so usage is recorded strictly after the write.
 *
 * Nothing here logs — exception text can quote clip content.
 */
object TypviaClipboard {
  // Status protocol shared with src/pasteboard.rs.
  private const val STATUS_OK = 0
  private const val STATUS_BACKEND = 5

  /** Label on our own clips; never read back, never shown to the user. */
  private const val CLIP_LABEL = "dev.typvia.mobile.snippet"

  /** How long the JNI thread waits for the main looper to run the write. */
  private const val WRITE_TIMEOUT_SECONDS = 5L

  private val mainHandler = Handler(Looper.getMainLooper())

  /** Writes `text` as the primary clip. Must not be called on the main
   * thread (the latch would deadlock behind the posted write). */
  fun copy(context: Context, text: String): Int {
    if (Looper.myLooper() == Looper.getMainLooper()) return STATUS_BACKEND
    val status = AtomicInteger(STATUS_BACKEND)
    val done = CountDownLatch(1)
    val appContext = context.applicationContext
    mainHandler.post {
      try {
        status.set(writeOnMain(appContext, text))
      } finally {
        done.countDown()
      }
    }
    return try {
      if (done.await(WRITE_TIMEOUT_SECONDS, TimeUnit.SECONDS)) status.get() else STATUS_BACKEND
    } catch (e: InterruptedException) {
      Thread.currentThread().interrupt()
      STATUS_BACKEND
    }
  }

  private fun writeOnMain(context: Context, text: String): Int {
    return try {
      val manager = context.getSystemService(ClipboardManager::class.java) ?: return STATUS_BACKEND
      manager.setPrimaryClip(ClipData.newPlainText(CLIP_LABEL, text))
      STATUS_OK
    } catch (e: Exception) {
      STATUS_BACKEND
    }
  }
}
