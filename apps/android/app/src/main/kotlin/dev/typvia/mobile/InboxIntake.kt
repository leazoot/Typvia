// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import java.io.File

/**
 * Taking in what the share sheet left behind.
 *
 * The share target is write-only by contract: it lands one document per share
 * and never opens the database. This is the other half — the app reads those
 * documents and files them — and without it the sheet accepts text that never
 * arrives anywhere, which is worse than not offering the sheet at all.
 *
 * Two rules carry over from the platform that already has this:
 *  - **only what actually landed is cleared.** Clearing the lot after a partial
 *    failure is the path where the reader watched the sheet accept something
 *    and then lost it.
 *  - **a trigger clash costs the trigger, not the words** — which has nothing
 *    to apply to yet: schema v1 carries a title and a text and no trigger, so
 *    a shared snippet cannot collide with one. The rule is written down here
 *    rather than implemented as a retry that repeats an identical call; when
 *    the schema grows a trigger, this is where it goes.
 */
object InboxIntake {
    /** Matches the share target's own directory name; both halves must agree. */
    const val DIRECTORY = "inbox"

    /**
     * @return how many documents were filed. Zero covers an empty inbox and a
     *   failed read the same way — there is nothing for a screen to do about
     *   either, and neither is news.
     */
    suspend fun run(store: TypviaStore, dataDir: File): Int {
        val inbox = File(dataDir, DIRECTORY)
        val documents = inbox.listFiles { file -> file.name.endsWith(".json") } ?: return 0
        var filed = 0
        for (document in documents.sortedBy { it.name }) {
            val item = runCatching { SharedItem.parse(document.readText()) }.getOrNull() ?: continue
            if (file(store, item)) {
                // Deleted one at a time, and only after it is in: a document
                // that could not be filed stays for the next attempt.
                document.delete()
                filed += 1
            }
        }
        return filed
    }

    private suspend fun file(store: TypviaStore, item: SharedItem): Boolean =
        SnippetWriter.write(store, item.title ?: "", item.text).isSuccess
}

/**
 * One shared document, schema v1.
 *
 * Parsed by hand for the same reason the writer encodes by hand: `org.json` is
 * a stub in host-JVM unit tests, and one text field does not justify a JSON
 * dependency in a shipped app.
 */
data class SharedItem(val sharedAt: Long, val title: String?, val text: String) {
    companion object {
        fun parse(json: String): SharedItem? {
            val version = number(json, "schema_version")?.toLong() ?: return null
            if (version != 1L) return null
            val text = string(json, "text") ?: return null
            if (text.isBlank()) return null
            return SharedItem(
                sharedAt = number(json, "shared_at")?.toLong() ?: 0L,
                title = string(json, "title"),
                text = text,
            )
        }

        private fun number(json: String, key: String): Double? {
            val at = json.indexOf("\"$key\":").takeIf { it >= 0 } ?: return null
            val rest = json.substring(at + key.length + 3)
            return rest.takeWhile { it.isDigit() || it == '-' }.toDoubleOrNull()
        }

        /** Reads one JSON string value, honouring the two escape classes the
         * writer emits. */
        private fun string(json: String, key: String): String? {
            val marker = "\"$key\":\""
            val at = json.indexOf(marker).takeIf { it >= 0 } ?: return null
            val builder = StringBuilder()
            var index = at + marker.length
            while (index < json.length) {
                val char = json[index]
                when {
                    char == '"' -> return builder.toString()
                    char == '\\' && index + 1 < json.length -> {
                        when (val escaped = json[index + 1]) {
                            'n' -> builder.append('\n')
                            'r' -> builder.append('\r')
                            't' -> builder.append('\t')
                            'u' -> {
                                val code = json.substring(index + 2, index + 6).toIntOrNull(16)
                                    ?: return null
                                builder.append(code.toChar())
                                index += 4
                            }
                            else -> builder.append(escaped)
                        }
                        index += 1
                    }
                    else -> builder.append(char)
                }
                index += 1
            }
            return null
        }
    }
}
