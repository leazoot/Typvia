package dev.typvia.mobile

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.PersistableBundle
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * One-time sensitive clipboard copy with a guaranteed-scheduled auto-clear
 * (the iOS sibling is the OS-enforced UIPasteboard expiry in
 * src/pasteboard.rs). Android has no clipboard-expiry API, so the guarantee
 * is built structurally instead: [copy] is the only method that touches the
 * clipboard with a secret, and it schedules the guarded clear BEFORE setting
 * the clip — there is no code path that puts a secret on the clipboard
 * without its clear (security red line).
 *
 * What the clip carries:
 *  - `EXTRA_IS_SENSITIVE` in the description extras. The constant is API 33
 *    but its value is the canonical string key ("android.content.extra
 *    .IS_SENSITIVE") and `setExtras` exists since API 24, so pre-33 systems
 *    (minSdk 28) simply carry the marker for any consumer that knows it;
 *    on 33+ the system redacts the clipboard preview / editor chip.
 *  - A private label ([CLIP_LABEL]) so the delayed clear can recognize our
 *    own clip and never clobber a newer clip the user copied since.
 *
 * Honest guarantees and their limits (the focus-gating below was verified
 * empirically on the API 34 emulator — ClipboardService logs "Denying
 * clipboard access … not in focus" for both calls):
 *  - Foreground at expiry: the main-looper `postDelayed` fires after the
 *    shared 30s, the description guard is readable, and the clear lands.
 *  - Backgrounded at expiry: Android focus-gates BOTH
 *    `getPrimaryClipDescription` AND `clearPrimaryClip` (on API 34 the
 *    denial covers writes too, not just the documented Android 10+ read
 *    restriction). The scheduled clear still runs (the guard's null
 *    description falls through to a clear attempt — a no-op when denied,
 *    a wipe when this build allows it), but on a denying build the secret
 *    outlives its window while the user stays in another app. What closes
 *    the gap, honestly: [clearStaleClip] wipes the leftover clip the moment
 *    the user returns to Typvia (verified on the emulator: the denied
 *    background clear was followed by a successful resume sweep before
 *    anything could paste); `EXTRA_IS_SENSITIVE` keeps every preview
 *    redacted meanwhile; and on Android 13+ the system itself clears the
 *    clipboard after ~1h (no such backstop on 12 and below). This is weaker
 *    than the iOS OS-enforced expiry — recorded as a platform asymmetry,
 *    not papered over.
 *  - Process death / cached-process freeze inside the window kills or stalls
 *    the pending clear; the same [clearStaleClip] launch/resume sweep and
 *    the Android 13+ system timeout are the mitigations. A clear from a
 *    dead process is not schedulable — `AlarmManager`/WorkManager were
 *    considered and rejected: a fresh background process faces the same
 *    focus gate (it could neither verify nor reliably clear the clip) and
 *    would add machinery without adding guarantee.
 *  - Clearing when the app goes to the background was rejected by design
 *    intent: the flow IS "copy, switch away, paste", so the clip must
 *    survive backgrounding for its window.
 *
 * Nothing in this file logs; exception messages are dropped because they can
 * quote clip text.
 */
object TypviaSensitiveClipboard {
  // Status protocol shared with src/pasteboard.rs (Android write path).
  private const val STATUS_OK = 0
  private const val STATUS_BACKEND = 5

  /** Private label naming our sensitive clip so the delayed clear and the
   * stale-clip sweep recognize it without reading any content. */
  private const val CLIP_LABEL = "dev.typvia.mobile.sensitive"

  /** How long the JNI thread waits for the main looper to run the write; a
   * healthy main thread answers within a frame. */
  private const val WRITE_TIMEOUT_SECONDS = 5L

  private val mainHandler = Handler(Looper.getMainLooper())

  // The one live pending clear; main-thread confined. A newer copy cancels
  // and replaces it (the newer clear covers the shared label), and its
  // absence while our clip is still primary identifies a stale clip from a
  // process killed mid-window (see clearStaleClip).
  private var pendingClear: Runnable? = null

  /**
   * Writes `secret` as the primary clip, sensitive-marked, with the clear
   * scheduled `clearMs` from now. Called over JNI from a Rust blocking
   * thread; the ClipboardManager work runs on the main looper and this
   * thread parks on its completion so the caller can order record-usage
   * strictly after a confirmed write. Must not be called on the main thread
   * (the latch would deadlock behind the posted write).
   */
  fun copy(context: Context, secret: String, clearMs: Long): Int {
    if (Looper.myLooper() == Looper.getMainLooper()) return STATUS_BACKEND
    // A non-positive window would mean a write without a real auto-clear;
    // refuse rather than deliver (red line).
    if (clearMs <= 0) return STATUS_BACKEND
    val status = AtomicInteger(STATUS_BACKEND)
    val done = CountDownLatch(1)
    mainHandler.post {
      try {
        status.set(writeOnMain(context.applicationContext, secret, clearMs))
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

  /**
   * Called from MainActivity.onResume: a sensitive clip that is still ours
   * while no countdown is live is left over from an expiry the OS denied in
   * the background (focus gate, see the class comment) or from a process
   * that died mid-window — wipe it now that focus makes the clear possible.
   * A live pendingClear means the current window is simply being honored
   * (returning to Typvia mid-countdown must not cut the copy short — the
   * row is still showing "clears in Ns").
   */
  fun clearStaleClip(context: Context) {
    val appContext = context.applicationContext
    mainHandler.post {
      if (pendingClear != null) return@post
      try {
        val manager =
          appContext.getSystemService(ClipboardManager::class.java) ?: return@post
        // onResume runs focused, so the description is readable here: null
        // really means "empty" and only our own label is ever cleared.
        val label = manager.primaryClipDescription?.label?.toString() ?: return@post
        if (label == CLIP_LABEL) manager.clearPrimaryClip()
      } catch (e: Exception) {
        // Best-effort sweep; the scheduled-clear path is the real guarantee.
      }
    }
  }

  /** Main looper only: the single sensitive clipboard write. */
  private fun writeOnMain(context: Context, secret: String, clearMs: Long): Int {
    return try {
      val manager = context.getSystemService(ClipboardManager::class.java) ?: return STATUS_BACKEND
      val clip = ClipData.newPlainText(CLIP_LABEL, secret)
      clip.description.extras = PersistableBundle().apply {
        putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true)
      }
      // Structural red line: the clear is scheduled before the clip is set,
      // so a secret can never reach the clipboard without a pending clear.
      // If the set below then fails, the orphaned clear finds a foreign (or
      // no) label at fire time and does nothing — the guard makes the order
      // safe.
      pendingClear?.let { mainHandler.removeCallbacks(it) }
      val clear = Runnable {
        pendingClear = null
        clearIfOurs(manager)
      }
      pendingClear = clear
      mainHandler.postDelayed(clear, clearMs)
      manager.setPrimaryClip(clip)
      STATUS_OK
    } catch (e: Exception) {
      STATUS_BACKEND
    }
  }

  /** The guarded clear: skips only when a readable description proves a
   * newer, foreign clip took over. An unreadable (null) description means
   * "empty or focus-denied"; the clear is attempted anyway — a no-op on an
   * empty clipboard, denied silently by a focus-gating build, a wipe where
   * allowed (see the class comment). */
  private fun clearIfOurs(manager: ClipboardManager) {
    try {
      val description = manager.primaryClipDescription
      if (description != null && description.label?.toString() != CLIP_LABEL) return
      manager.clearPrimaryClip()
    } catch (e: Exception) {
      // Nothing useful can be done mid-flight; the stale-clip sweep and the
      // Android 13+ system timeout remain as backstops.
    }
  }
}
