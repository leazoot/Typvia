// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ui.TypeSort
import uniffi.typvia_mobile_ffi.Snippet

/**
 * What the home screen shows, as values.
 *
 * Everything here is derived from what the core returned, and nothing here
 * decides anything the core decides: no ranking, no sensitivity test of its
 * own, no trigger parsing. The one rule these types hold is a display rule
 * with teeth — a secret has no previewable body, and that is expressed by
 * leaving no way to build a row that carries one.
 */

/**
 * A trigger word split at what the reader has already typed.
 *
 * Which snippets matched is the core's answer; this only re-finds the head of a
 * trigger the reader typed themselves, so the head can be underlined and the
 * tail cannot. Underlined, never highlighted: a coloured block behind text is
 * the search engine's idea of emphasis, not this product's.
 */
data class TriggerMatch(val matched: String, val rest: String) {
    companion object {
        fun of(trigger: String, query: String): TriggerMatch {
            val typed = query.trim()
            if (typed.isEmpty() || !trigger.startsWith(typed, ignoreCase = true)) {
                return TriggerMatch(matched = "", rest = trigger)
            }
            return TriggerMatch(
                matched = trigger.take(typed.length),
                rest = trigger.drop(typed.length),
            )
        }
    }
}

/** A recall card: one of the sheets in the horizontal run under the search position. */
data class RecallTile(
    val id: String,
    val title: String,
    val sort: TypeSort?,
    val trigger: String?,
) {
    companion object {
        fun of(snippet: Snippet): RecallTile = RecallTile(
            id = snippet.id,
            title = snippet.title,
            sort = TypeSort.ofCoreType(snippet.snippetType),
            trigger = snippet.trigger,
        )
    }
}

/** A line in the trigger list under the cards. */
data class TriggerLine(
    val id: String,
    val trigger: String,
    val title: String,
) {
    companion object {
        /**
         * Only snippets that actually have a trigger word can be lines in a
         * list *of* trigger words. A row without one would be a promise the
         * keyboard cannot keep.
         */
        fun of(snippet: Snippet): TriggerLine? {
            val trigger = snippet.trigger?.takeIf { it.isNotBlank() } ?: return null
            return TriggerLine(id = snippet.id, trigger = trigger, title = snippet.title)
        }
    }
}

/**
 * Everything the shelf shows, assembled in one pass.
 *
 * The assembly happens here rather than in a screen so that it runs inside the
 * one call that already holds the database, and so that it can be tested
 * without a screen.
 */
data class HomeShelf(
    val total: UInt,
    val tiles: List<RecallTile>,
    val triggers: List<TriggerLine>,
) {
    companion object {
        /** How many rows the cards ask for. */
        const val RECENT_LIMIT: UInt = 12u

        /** How many trigger lines the frames print. */
        const val TRIGGER_LIMIT = 3

        /**
         * Asked of the usage-ordered view. A margin over [TRIGGER_LIMIT],
         * because the most-used snippets need not all have a trigger word —
         * without it a heavily used snippet with no trigger would silently
         * shorten the list.
         */
        const val TRIGGER_PAGE_LIMIT: UInt = 12u

        /**
         * @param recent the core's recall order, used as given.
         * @param mostUsed the core's usage order. The trigger lines are headed
         *   *the ones you type most*, so they come from the list that answers
         *   that question rather than from the recall order.
         */
        fun assemble(total: UInt, recent: List<Snippet>, mostUsed: List<Snippet>): HomeShelf =
            HomeShelf(
                total = total,
                tiles = recent.map(RecallTile::of),
                triggers = mostUsed.mapNotNull(TriggerLine::of).take(TRIGGER_LIMIT),
            )
    }
}
