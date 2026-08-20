// Runs the real FFI on the host JVM (JNA loads the host dylib built by
// crates/mobile-ffi/build-android.sh — the same loading path as the mobile-ffi
// smoke test), so the sensitive red line is asserted through the exact code the
// IME ships. The envelope bytes spell a deliberately fake marker so the search
// assertion can prove nothing echoes it.

package dev.typvia.ime

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

// "AKIA_FAKE" as bytes: 65,75,73,65,95,70,65,75,69
private const val FIXTURE =
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

/** FIXTURE plus one fillable template entry (kept separate so the default
 * ordering assertions stay untouched). */
private const val TEMPLATE_FIXTURE =
    """{"snapshot_version":1,"generated_at":1700000000000,"device_id":"device-1",""" +
        """"snippets":[""" +
        """{"id":"t1","title":"PR review","snippet_type":"template","trigger":null,""" +
        """"trigger_mode":null,"folder_id":null,"is_favorite":false,""" +
        """"body":"Hi {{team}}, the {{project}} PR is ready for {{team}}."},""" +
        """{"id":"s9","encrypted_metadata":[65,75,73,65,95,70,65,75,69]}""" +
        """],""" +
        """"recent_ids":[],"favorite_ids":[],"folder_metadata":[]}"""

class SnapshotStoreTest {
    @get:Rule
    val temp = TemporaryFolder()

    private fun write(content: String): File {
        val file = temp.newFile("snapshot.json")
        file.writeText(content)
        return file
    }

    @Test
    fun loadsEntriesRecentFirstAndPopulatedFolders() {
        val store = SnapshotStore.load(write(FIXTURE))
        assertNotNull(store)
        assertEquals(listOf("s2", "s1", "s9"), store!!.entries.map { it.id })
        // Only folders holding at least one entry become chips.
        assertEquals(listOf("Email"), store.folders.map { it.name })
    }

    @Test
    fun sensitiveEntryCarriesNoPlaintextFields() {
        val store = SnapshotStore.load(write(FIXTURE))!!
        val locked = store.entries.first { it.id == "s9" }
        assertTrue(locked.isSensitive)
        assertEquals("", locked.title)
        assertEquals("", locked.snippetType)
        assertNull(locked.trigger)
        assertNull(locked.folderId)
    }

    @Test
    fun bodyReturnsNormalTextAndNullForSensitiveOrUnknown() {
        val store = SnapshotStore.load(write(FIXTURE))!!
        assertEquals("docker logs -f app", store.body("s1"))
        assertNull(store.body("s9"))
        assertNull(store.body("ghost"))
    }

    @Test
    fun searchMatchesNormalEntriesAndNeverTheEnvelope() {
        val store = SnapshotStore.load(write(FIXTURE))!!
        assertEquals(listOf("s1"), store.search("DOCKER").map { it.id })
        // Red line: the ciphertext envelope is not searchable at any layer.
        assertEquals(emptyList<String>(), store.search("AKIA_FAKE").map { it.id })
    }

    @Test
    fun templateVariablesListDistinctNamesFirstSeenAndRefuseSensitive() {
        val store = SnapshotStore.load(write(TEMPLATE_FIXTURE))!!
        assertEquals(listOf("team", "project"), store.templateVariables("t1"))
        // Sensitive and unknown ids degrade to "nothing to fill".
        assertEquals(emptyList<String>(), store.templateVariables("s9"))
        assertEquals(emptyList<String>(), store.templateVariables("ghost"))
    }

    @Test
    fun templatePreviewMarksUnfilledAndRenderBlanksThem() {
        val store = SnapshotStore.load(write(TEMPLATE_FIXTURE))!!
        val values = mapOf("team" to "Core")
        assertEquals(
            "Hi Core, the ‹project› PR is ready for Core.",
            store.templatePreview("t1", values),
        )
        assertEquals(
            "Hi Core, the  PR is ready for Core.",
            store.templateRender("t1", values),
        )
    }

    @Test
    fun templateFillRefusesSensitiveIdsWithNull() {
        val store = SnapshotStore.load(write(TEMPLATE_FIXTURE))!!
        // Red line: the fill path can never surface vault content.
        assertNull(store.templatePreview("s9", emptyMap()))
        assertNull(store.templateRender("s9", emptyMap()))
    }

    @Test
    fun malformedDocumentDegradesToNull() {
        assertNull(SnapshotStore.load(write("not a snapshot")))
    }

    @Test
    fun missingFileDegradesToNull() {
        assertNull(SnapshotStore.load(File(temp.root, "absent.json")))
    }
}
