// SPIKE (TASK-010): Kotlin calling Rust over both FFI paths.

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import uniffi.ffi_spike.firstTitle
import uniffi.ffi_spike.snapshotCount

// Path B: hand-written C ABI via a plain JNA interface.
interface CAbi : Library {
    fun typvia_snapshot_count(json: String): Long
    fun typvia_first_title(json: String): Pointer?
    fun typvia_string_free(s: Pointer?)
}

fun main() {
    val json = """{"snippets":[{"title":"Standup notes"},{"title":"SQL header"}]}"""

    // Path A: UniFFI generated bindings (typed Kotlin API).
    val uniffiCount = snapshotCount(json)
    val uniffiTitle = firstTitle(json) ?: "<none>"
    println("uniffi: count=$uniffiCount first=$uniffiTitle")

    val cabi = Native.load("ffi_spike", CAbi::class.java)
    val cCount = cabi.typvia_snapshot_count(json)
    val raw = cabi.typvia_first_title(json)
    val cTitle = raw?.getString(0) ?: "<none>"
    cabi.typvia_string_free(raw)
    println("cabi: count=$cCount first=$cTitle")

    val ok = uniffiCount == 2L && cCount == 2L &&
        uniffiTitle == "Standup notes" && cTitle == "Standup notes"
    println(if (ok) "KOTLIN_FFI_OK" else "KOTLIN_FFI_MISMATCH")
}
