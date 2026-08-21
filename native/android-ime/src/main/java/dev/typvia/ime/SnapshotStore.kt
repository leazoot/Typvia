// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Read-only access to the KeyboardSnapshot document the host app writes into
// its private data dir. The IME runs in the same APK/UID, so a plain file
// read suffices — it never writes, and parsing/validation live in Rust
// (typvia-mobile-ffi) so a malformed, stale, or newer-version file degrades
// to the empty state instead of crashing.

package dev.typvia.ime

import java.io.File
import uniffi.typvia_mobile_ffi.SnapshotEntry
import uniffi.typvia_mobile_ffi.SnapshotException
import uniffi.typvia_mobile_ffi.SnapshotFolder
import uniffi.typvia_mobile_ffi.entryBody
import uniffi.typvia_mobile_ffi.filterEntries
import uniffi.typvia_mobile_ffi.snapshotEntries
import uniffi.typvia_mobile_ffi.snapshotFolders
import uniffi.typvia_mobile_ffi.templateFillPreview
import uniffi.typvia_mobile_ffi.templateFillRender
import uniffi.typvia_mobile_ffi.templateVariables

class SnapshotStore private constructor(
    /** The raw document, kept for follow-up FFI calls (search, body lookup). */
    private val json: String,
    /** Entries in default display order (recent first, then file order). */
    val entries: List<SnapshotEntry>,
    /** Folders holding at least one entry, in display order. */
    val folders: List<SnapshotFolder>,
) {
    /**
     * Entries matching a search query (Rust-side, case-insensitive over
     * title/trigger/body of normal entries). Empty query returns the default
     * list. A call that fails mid-session degrades to no rows.
     */
    fun search(query: String): List<SnapshotEntry> =
        try {
            filterEntries(json, query)
        } catch (error: SnapshotException) {
            emptyList()
        }

    /**
     * Body text for insertion; null for sensitive or unknown ids, so a
     * locked row can never insert plaintext.
     */
    fun body(id: String): String? =
        try {
            entryBody(json, id)
        } catch (error: SnapshotException) {
            null
        }

    /**
     * Distinct `{{variable}}` names of one normal entry's body, first-seen
     * order (the fill view's field list). Empty for entries without
     * variables and for sensitive or unknown ids (the FFI refuses those
     * with `Invalid`, which degrades to "nothing to fill" here).
     */
    fun templateVariables(id: String): List<String> =
        try {
            templateVariables(json, id)
        } catch (error: SnapshotException) {
            emptyList()
        }

    /** Lenient fill preview: filled values win, an unfilled variable shows
     * a `‹name›` placeholder. Null for sensitive or unknown ids. */
    fun templatePreview(id: String, values: Map<String, String>): String? =
        try {
            templateFillPreview(json, id, values)
        } catch (error: SnapshotException) {
            null
        }

    /** Final fill for insertion: unfilled variables become empty text (no
     * placeholder marks leave the keyboard). Null for sensitive or unknown
     * ids, so the fill path can never surface vault content. */
    fun templateRender(id: String, values: Map<String, String>): String? =
        try {
            templateFillRender(json, id, values)
        } catch (error: SnapshotException) {
            null
        }

    companion object {
        /** Written by the host app under Context.dataDir (Tauri's
         * app_data_dir on Android is the package root, not files/). */
        const val RELATIVE_PATH = "keyboard-snapshot/snapshot.json"

        /**
         * Loads and validates the current snapshot. Null covers every
         * degraded case the same way — no file yet, unreadable bytes, failed
         * validation — because the keyboard's answer to all of them is the
         * same honest empty state. Errors are deliberately not logged:
         * snapshot-adjacent messages could reach logcat.
         */
        fun load(file: File): SnapshotStore? {
            val json =
                try {
                    file.readText()
                } catch (error: java.io.IOException) {
                    return null
                }
            return try {
                SnapshotStore(json, snapshotEntries(json), snapshotFolders(json))
            } catch (error: SnapshotException) {
                null
            }
        }
    }
}
