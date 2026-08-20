// Host Kotlin smoke for the UniFFI surface: the same typed calls and
// assertions as smoke/main.swift, run through the generated Kotlin
// bindings so the Android chain proves the identical contract. Run by
// build-android.sh on the host JVM (JNA loads the host dylib); exits
// non-zero on any mismatch. The sensitive envelope bytes spell a
// deliberately fake marker so the red-line checks can assert nothing
// echoes it.

import uniffi.typvia_mobile_ffi.SnapshotException
import uniffi.typvia_mobile_ffi.entryBody
import uniffi.typvia_mobile_ffi.filterEntries
import uniffi.typvia_mobile_ffi.filterFolderTitles
import uniffi.typvia_mobile_ffi.parseSnapshot
import uniffi.typvia_mobile_ffi.snapshotEntries
import uniffi.typvia_mobile_ffi.snapshotFolders
import kotlin.system.exitProcess

// "AKIA_FAKE" as bytes: 65,75,73,65,95,70,65,75,69
private val json =
    """{"snapshot_version":1,"generated_at":1700000000000,"device_id":"device-1",""" +
        """"snippets":[""" +
        """{"id":"s1","title":"Docker logs","snippet_type":"command","trigger":":dlog",""" +
        """"trigger_mode":"delimiter","folder_id":"f1","is_favorite":false,""" +
        """"body":"docker logs -f app"},""" +
        """{"id":"s2","title":"Standup","snippet_type":"text","trigger":null,""" +
        """"trigger_mode":null,"folder_id":null,"is_favorite":true,""" +
        """"body":"Yesterday I shipped"},""" +
        """{"id":"s9","encrypted_metadata":[65,75,73,65,95,70,65,75,69]}""" +
        """],""" +
        """"recent_ids":["s2","s1"],"favorite_ids":["s2"],""" +
        """"folder_metadata":[{"id":"f2","name":"Shell","sort_order":1},""" +
        """{"id":"f1","name":"Email","sort_order":0}]}"""

fun main() {
    val failures = mutableListOf<String>()

    try {
        val overview = parseSnapshot(json)
        if (overview.snapshotVersion != 1u) failures.add("version=${overview.snapshotVersion}")
        if (overview.deviceId != "device-1") failures.add("device=${overview.deviceId}")
        if (overview.recentTotal != 2u || overview.favoriteTotal != 1u) {
            failures.add("counts=${overview.recentTotal}/${overview.favoriteTotal}")
        }
        if (overview.folderTitles != listOf("Email", "Shell")) {
            failures.add("titles=${overview.folderTitles}")
        }
    } catch (error: SnapshotException) {
        failures.add("parse threw $error")
    }

    try {
        val hits = filterFolderTitles(json, "sHe")
        if (hits != listOf("Shell")) failures.add("filter=$hits")
    } catch (error: SnapshotException) {
        failures.add("filter threw $error")
    }

    try {
        val entries = snapshotEntries(json)
        if (entries.map { it.id } != listOf("s2", "s1", "s9")) {
            failures.add("order=${entries.map { it.id }}")
        }
        val s2 = entries.find { it.id == "s2" }
        if (s2 == null) {
            failures.add("s2 missing")
        } else if (s2.title != "Standup" || !s2.isFavorite || !s2.isRecent || s2.isSensitive) {
            failures.add("s2 fields wrong")
        }
        // Red line: the sensitive entry carries no plaintext through the FFI.
        val s9 = entries.find { it.id == "s9" }
        if (s9 == null) {
            failures.add("sensitive entry missing from list")
        } else if (!s9.isSensitive || s9.title != "" || s9.snippetType != "" ||
            s9.trigger != null || s9.folderId != null
        ) {
            failures.add("sensitive entry leaked fields")
        }
    } catch (error: SnapshotException) {
        failures.add("entries threw $error")
    }

    try {
        if (entryBody(json, "s1") != "docker logs -f app") failures.add("body(s1) wrong")
        if (entryBody(json, "s9") != null) failures.add("sensitive body was not null")
        if (entryBody(json, "ghost") != null) failures.add("unknown id yielded a body")
    } catch (error: SnapshotException) {
        failures.add("body threw $error")
    }

    try {
        val hits = filterEntries(json, "DOCKER")
        if (hits.map { it.id } != listOf("s1")) failures.add("search=${hits.map { it.id }}")
        val marker = filterEntries(json, "AKIA_FAKE")
        if (marker.isNotEmpty()) failures.add("envelope marker matched a search")
    } catch (error: SnapshotException) {
        failures.add("search threw $error")
    }

    try {
        val folders = snapshotFolders(json)
        if (folders.map { it.name } != listOf("Email")) {
            failures.add("folders=${folders.map { it.name }}")
        }
    } catch (error: SnapshotException) {
        failures.add("folders threw $error")
    }

    try {
        parseSnapshot("not a snapshot")
        failures.add("malformed document was accepted")
    } catch (error: SnapshotException.Malformed) {
        // Expected: unreadable documents fail without echoing content.
    } catch (error: SnapshotException) {
        failures.add("malformed document raised the wrong error: $error")
    }

    if (failures.isEmpty()) {
        println("TYPVIA_FFI_SMOKE_OK")
    } else {
        println("TYPVIA_FFI_SMOKE_FAIL: ${failures.joinToString("; ")}")
        exitProcess(1)
    }
}
