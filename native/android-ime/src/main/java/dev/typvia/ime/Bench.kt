// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// What the bench is holding, as values. The views draw this and decide
// nothing; every question about *which* snippets a word matches is put to the
// shared engine through the snapshot rather than answered here.

package dev.typvia.ime

import dev.typvia.mobile.ui.KeyboardPhase
import uniffi.typvia_mobile_ffi.SnapshotEntry

/** One sheet in the run. */
data class BenchTile(
    val id: String,
    val title: String,
    val trigger: String?,
    val mark: String,
    val isSecret: Boolean,
    /**
     * The first hit of a live query: wider, and carrying a preview. The
     * compositor hands over the one most likely wanted rather than making the
     * reader pick it out of a row of equals.
     */
    val isLead: Boolean,
    /** Just used. It steps back rather than jumping out from under the thumb. */
    val isSpent: Boolean,
)

/**
 * One lead in the sorts band.
 *
 * Membership is fixed when the bench is built, and a lead with nothing behind
 * it right now dims instead of leaving: a band that re-flows under the thumb
 * is a band nobody trusts to still be what it was a moment ago.
 */
data class BenchSort(
    /** Null is the "all kinds" lead. */
    val mark: String?,
    val isActive: Boolean,
    val hasHits: Boolean,
)

/**
 * The bench.
 *
 * It owns the query, the two filters and the phase, and it is deliberately
 * free of Android types so the rules can be tested without a device.
 */
class Bench(private val store: SnapshotStore?) {
    var query: String = ""
        private set

    /** The chosen kind, or null for all of them. */
    var sort: String? = null
        private set

    /** The chosen scope, or null for the whole bench. */
    var scope: PanelCategory? = null
        private set

    var phase: KeyboardPhase = KeyboardPhase.Browsing
        private set

    var tiles: List<BenchTile> = emptyList()
        private set

    /**
     * The scopes the function row offers.
     *
     * Only the two the frames give the keyboard. The other three the
     * vocabulary knows about are answered elsewhere on this panel — kind by
     * the sorts band, and folders by the app, which is where a library is
     * read rather than reached into.
     */
    val scopes: List<PanelCategory> =
        store?.let { PanelCategory.available(it.entries, it.folders) }
            .orEmpty()
            .filter { it is PanelCategory.Recent || it is PanelCategory.Starred }

    /** The kinds this snapshot actually holds, in the delivery's order. */
    private val marks: List<String> =
        MARK_ORDER.filter { mark -> store?.entries.orEmpty().any { markOf(it) == mark } }

    /** What the bench can reach. Not the library's total, which it cannot see. */
    val total: Int = store?.entries?.size ?: 0

    private var spentId: String? = null

    init {
        rebuild()
    }

    /** The sorts band, as it should be drawn right now. */
    fun sorts(): List<BenchSort> {
        val reachable = matching(base())
        return listOf(BenchSort(mark = null, isActive = sort == null, hasHits = reachable.isNotEmpty())) +
            marks.map { mark ->
                BenchSort(
                    mark = mark,
                    isActive = sort == mark,
                    hasHits = reachable.any { markOf(it) == mark },
                )
            }
    }

    // MARK: What the reader does

    fun type(character: Char) {
        query += character
        spentId = null
        rebuild()
    }

    fun backspace(): Boolean {
        if (query.isEmpty()) return false
        query = query.dropLast(1)
        rebuild()
        return true
    }

    fun clearQuery() {
        query = ""
        rebuild()
    }

    /** Tapping the active lead clears the filter rather than doing nothing. */
    fun select(mark: String?) {
        sort = if (sort == mark) null else mark
        rebuild()
    }

    fun select(category: PanelCategory?) {
        scope = if (scope == category) null else category
        rebuild()
    }

    /**
     * Body text for insertion. A secret never comes back through here — the
     * snapshot holds no plaintext for it, and the panel sends those to the ink
     * room instead.
     */
    fun body(id: String): String? {
        val tile = tiles.firstOrNull { it.id == id } ?: return null
        if (tile.isSecret) return null
        return store?.body(id)
    }

    /** Records what was just typed into the host, and shows the undo line. */
    fun markUsed(id: String, title: String) {
        spentId = id
        rebuild()
        phase = KeyboardPhase.Inserted(title)
    }

    fun openSecret(title: String) {
        phase = KeyboardPhase.Secret(title)
    }

    /** Back from the undo line or the ink room to whatever was on the bench. */
    fun backToBench() {
        rebuild()
    }

    // MARK: Assembly

    private fun base(): List<SnapshotEntry> {
        val entries = store ?: return emptyList()
        return if (query.isBlank()) entries.entries else entries.search(query.trim())
    }

    private fun matching(entries: List<SnapshotEntry>): List<SnapshotEntry> =
        entries
            .filter { sort == null || markOf(it) == sort }
            .filter { scope?.matches(it) ?: true }

    private fun rebuild() {
        val filtered = matching(base())
        val filtering = query.isNotBlank()
        tiles = filtered.mapIndexed { index, entry ->
            BenchTile(
                id = entry.id,
                // A secret arrives as an id and an envelope: no title, no
                // kind, nothing that could be drawn. The dot run says there is
                // something there without saying anything about it.
                title = if (entry.isSensitive) LOCKED_TITLE else entry.title,
                trigger = entry.trigger?.takeIf { it.isNotBlank() },
                mark = markOf(entry),
                isSecret = entry.isSensitive,
                isLead = index == 0 && filtering,
                isSpent = entry.id == spentId,
            )
        }
        phase = when {
            // No snapshot, unreadable bytes, or a library with nothing in it
            // yet: one honest answer covers all three, because the reader can
            // do the same one thing about each of them.
            store == null || store.entries.isEmpty() -> KeyboardPhase.Unreachable
            filtered.isEmpty() -> KeyboardPhase.NoMatch(query.trim())
            filtering -> KeyboardPhase.Filtering(query.trim())
            else -> KeyboardPhase.Browsing
        }
    }

    private companion object {
        /** The delivery's order for the sort marks, and the whole of it. */
        val MARK_ORDER = listOf("TX", "CD", "CM", "PR", "TP", "SC", "AI", "LK")

        /** What a locked sheet is called: shape, and nothing about content. */
        val LOCKED_TITLE = dev.typvia.mobile.ui.BodyMask.SHORT

        /**
         * A secret carries no kind of its own — the snapshot holds an id and
         * an envelope — so the mark is what it *is* rather than what the empty
         * string falls back to, which is the ordinary-text mark.
         */
        fun markOf(entry: SnapshotEntry): String =
            if (entry.isSensitive) "SC" else TypeMark.mark(entry.snippetType)
    }
}
