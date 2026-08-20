// Write-only inbox channel into the app-private data dir: one JSON document
// per share, landed tmp+rename so the main app never observes a half-written
// file. Byte-compatible with the iOS extension's writes (schema v1, same file
// naming) — the host's share_inbox.rs reads both unchanged. The Activity
// never reads the database, the keyboard snapshot, or anything else in the
// data dir — write-only by contract, because extensions never touch the main
// store. Pure JVM code (no Android types) so the host-JVM unit tests exercise
// the real writer.

package dev.typvia.share

import java.io.File
import java.util.UUID

object ShareInbox {
    /** Relative inbox directory under the app data dir; must match the
     * host's `share_inbox::INBOX_DIR_NAME` joined onto its Android
     * share-inbox root (the same data dir). */
    const val INBOX_DIR_NAME = "inbox"

    /** Mirror of the host's SHARE_TEXT_MAX_BYTES (UTF-8 bytes); Save is
     * disabled beyond it so the host never has to quarantine our writes. */
    const val TEXT_MAX_BYTES = 128 * 1024

    /** Mirror of the host/editor derived-title bound. */
    const val TITLE_MAX_CHARS = 60

    /**
     * The derived-title preview shown as the Title placeholder: first
     * non-empty line, bounded. The host applies the same rule when no title
     * is sent, so leaving the field empty and this preview agree (single
     * source of truth stays in share_inbox.rs).
     */
    fun derivedTitle(text: String): String {
        val line = text.split("\n").map { it.trim() }.firstOrNull { it.isNotEmpty() } ?: ""
        return if (line.length <= TITLE_MAX_CHARS) line else line.substring(0, TITLE_MAX_CHARS)
    }

    /**
     * Atomically lands one schema-v1 document as `inbox/share-<uuid>.json`
     * under [root]: encode, write `<name>.tmp` in the same directory,
     * rename. Returns false on any failure, leaving no readable partial
     * file (`*.json.tmp` is outside the host's scan set; the tmp file is
     * cleaned up best-effort). Nothing is logged in either outcome — a
     * document holds user content (log red line).
     */
    fun write(root: File, sharedAt: Long, title: String?, text: String): Boolean {
        val inbox = File(root, INBOX_DIR_NAME)
        val name = "share-${UUID.randomUUID()}.json"
        val tmp = File(inbox, "$name.tmp")
        return try {
            inbox.mkdirs()
            tmp.writeBytes(encode(sharedAt, title, text).toByteArray(Charsets.UTF_8))
            if (tmp.renameTo(File(inbox, name))) true else throw java.io.IOException()
        } catch (error: Exception) {
            tmp.delete()
            false
        }
    }

    /** Schema v1 (pinned; share_inbox.rs is the reading counterpart):
     * `{ "schema_version": 1, "shared_at": <unix ms>, "title"?: string,
     * "text": string }`. `title` is omitted when the sender typed none —
     * the host owns the first-line derivation. `source_hint` stays
     * reserved (parity with iOS: never written). */
    internal fun encode(sharedAt: Long, title: String?, text: String): String {
        val builder = StringBuilder()
        builder.append("{\"schema_version\":1,\"shared_at\":").append(sharedAt)
        if (title != null) {
            builder.append(",\"title\":")
            appendJsonString(builder, title)
        }
        builder.append(",\"text\":")
        appendJsonString(builder, text)
        return builder.append('}').toString()
    }

    /** Minimal RFC 8259 string encoder — the two mandatory escape classes
     * (quote/backslash and control characters); everything else, including
     * CJK and emoji, passes through as UTF-8. Hand-rolled because
     * org.json is absent from host-JVM unit tests and a JSON library
     * dependency is not justified for one string field. */
    private fun appendJsonString(builder: StringBuilder, value: String) {
        builder.append('"')
        for (ch in value) {
            when {
                ch == '"' -> builder.append("\\\"")
                ch == '\\' -> builder.append("\\\\")
                ch == '\n' -> builder.append("\\n")
                ch == '\r' -> builder.append("\\r")
                ch == '\t' -> builder.append("\\t")
                ch < ' ' -> builder.append("\\u%04x".format(ch.code))
                else -> builder.append(ch)
            }
        }
        builder.append('"')
    }
}
