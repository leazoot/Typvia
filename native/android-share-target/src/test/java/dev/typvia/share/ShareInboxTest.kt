// Host-JVM tests for the write-only inbox layer: file landing protocol
// (tmp+rename, uuid .json names), schema-v1 byte shape the host's
// share_inbox.rs reader pins, and the derived-title rule mirrored from the
// host. The Activity itself is exercised on the emulator, not here.

package dev.typvia.share

import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ShareInboxTest {
    private fun tempRoot(): File = Files.createTempDirectory("share-inbox-test").toFile()

    private fun soleInboxFile(root: File): File {
        val files = File(root, ShareInbox.INBOX_DIR_NAME).listFiles()!!.toList()
        assertEquals(1, files.size)
        return files[0]
    }

    @Test
    fun writeLandsOneFinalJsonFileAndNoTmpResidue() {
        val root = tempRoot()

        assertTrue(ShareInbox.write(root, 1_700_000_000_000L, null, "Ship it"))

        val file = soleInboxFile(root)
        assertTrue(file.name.startsWith("share-"))
        assertTrue(file.name.endsWith(".json"))
        assertFalse(file.name.endsWith(".tmp"))
    }

    @Test
    fun documentCarriesSchemaV1FieldsInHostOrder() {
        val root = tempRoot()

        assertTrue(ShareInbox.write(root, 1_700_000_000_000L, "Release notes", "body text"))

        val json = soleInboxFile(root).readText(Charsets.UTF_8)
        assertEquals(
            "{\"schema_version\":1,\"shared_at\":1700000000000," +
                "\"title\":\"Release notes\",\"text\":\"body text\"}",
            json,
        )
    }

    @Test
    fun titleIsOmittedWhenNoneWasTyped() {
        val root = tempRoot()

        assertTrue(ShareInbox.write(root, 1_000L, null, "first line\nrest"))

        val json = soleInboxFile(root).readText(Charsets.UTF_8)
        assertEquals("{\"schema_version\":1,\"shared_at\":1000,\"text\":\"first line\\nrest\"}", json)
    }

    @Test
    fun stringsAreJsonEscapedAndCjkPassesThrough() {
        assertEquals(
            "{\"schema_version\":1,\"shared_at\":1," +
                "\"text\":\"quote \\\" slash \\\\ tab \\t\\r\\n中文 emoji 🎉 bell \\u0007\"}",
            ShareInbox.encode(1L, null, "quote \" slash \\ tab \t\r\n中文 emoji 🎉 bell \u0007"),
        )
    }

    @Test
    fun derivedTitleIsFirstNonEmptyLineBoundedToSixtyChars() {
        assertEquals("Ship it behind a flag", ShareInbox.derivedTitle("\n  \nShip it behind a flag\nrest"))
        assertEquals("x".repeat(60), ShareInbox.derivedTitle("x".repeat(90)))
        assertEquals("", ShareInbox.derivedTitle("  \n \t "))
    }

    @Test
    fun failedWriteLeavesNoPartialFile() {
        val root = tempRoot()
        // A plain file where the inbox directory must be created makes both
        // mkdirs and the tmp write fail — the honest-failure path.
        assertTrue(File(root, ShareInbox.INBOX_DIR_NAME).createNewFile())

        assertFalse(ShareInbox.write(root, 1_000L, null, "text"))

        assertTrue(File(root, ShareInbox.INBOX_DIR_NAME).isFile)
    }

    @Test
    fun twoWritesLandTwoDistinctFiles() {
        val root = tempRoot()

        assertTrue(ShareInbox.write(root, 1_000L, null, "one"))
        assertTrue(ShareInbox.write(root, 2_000L, null, "two"))

        assertEquals(2, File(root, ShareInbox.INBOX_DIR_NAME).listFiles()!!.size)
    }
}
