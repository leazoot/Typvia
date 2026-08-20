package dev.typvia.mobile

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit

/**
 * System-scheduled background sync round. May run in a process the system
 * started without any activity — only Application.onCreate has run, Tauri
 * has not — so the native entry assembles its own minimal host state. This
 * class only loads the library, yields to a foregrounded app, and maps the
 * native result codes onto WorkManager semantics.
 */
class SyncWorker(context: Context, params: WorkerParameters) : Worker(context, params) {

  override fun doWork(): Result {
    // The foreground app runs its own sync triggers; a concurrent worker
    // round would only compete for the single-writer database.
    if (appIsInForeground()) return Result.success()
    return try {
      System.loadLibrary(LIB)
      when (nativeBackgroundRound(applicationContext)) {
        ROUND_OK -> Result.success()
        else -> Result.retry()
      }
    } catch (e: UnsatisfiedLinkError) {
      // A partial install/update shape; the next attempt retries with a
      // healthy library.
      Result.retry()
    }
  }

  // Process-importance checks are unusable here: executing a job boosts the
  // process to foreground importance, so runningAppProcesses reports
  // "foreground" for the worker's own run and the round would always yield
  // (observed on the emulator). ProcessLifecycleOwner tracks actual
  // activity visibility instead: STARTED only when a screen is showing,
  // CREATED in a headless spawn. Its state is main-thread-owned, so the
  // read hops there; an unresponsive main thread yields conservatively.
  private fun appIsInForeground(): Boolean {
    val answer = ArrayBlockingQueue<Boolean>(1)
    Handler(Looper.getMainLooper()).post {
      answer.offer(
        ProcessLifecycleOwner.get().lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)
      )
    }
    return answer.poll(5, TimeUnit.SECONDS) ?: true
  }

  // Result protocol shared with the Rust side (background.rs): 0 = the
  // round ran (or there was nothing to do), anything else = retry.
  private external fun nativeBackgroundRound(context: Context): Int

  companion object {
    private const val LIB = "typvia_mobile_lib"
    private const val ROUND_OK = 0
  }
}
