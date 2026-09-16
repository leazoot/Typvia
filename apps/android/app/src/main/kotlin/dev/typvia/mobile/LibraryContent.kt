// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ui.TypeSort
import uniffi.typvia_mobile_ffi.Folder
import uniffi.typvia_mobile_ffi.Snippet

/**
 * One chapter of the type specimen: a kind, how many of it there are, and
 * whether it is readable at all.
 *
 * The secret chapter is the one that can be shut. A shut chapter publishes
 * **no count** — not zero, not a dash — because how much is in the vault is a
 * fact about the vault, and a shut vault states none.
 */
data class Chapter(
    val sort: TypeSort,
    /** Absent only when the chapter is shut. */
    val count: UInt?,
    val isShut: Boolean,
) {
    companion object {
        /**
         * Every kind gets a chapter, in the design system's own order, whether
         * or not the core reported anything for it: a kind with nothing in it
         * has none, which is different from a kind nobody has heard of.
         */
        fun assemble(counts: Map<TypeSort, UInt>, vaultOpen: Boolean): List<Chapter> =
            TypeSort.entries.map { sort ->
                val shut = sort == TypeSort.Secret && !vaultOpen
                Chapter(
                    sort = sort,
                    count = if (shut) null else counts[sort] ?: 0u,
                    isShut = shut,
                )
            }
    }
}

/**
 * A run of rows under one of the reader's folders.
 *
 * Only the text chapter uses these — it is the largest one, and the delivery
 * files it under the folders the reader made. A group is a heading and its
 * rows, not a second kind of row, so a grouped chapter and a plain one are
 * still the same list.
 *
 * The rule is the other platform's, to the letter: a folder the reader has
 * emptied does not appear, rows in no folder come last and carry no heading,
 * and a folder this page did not fetch leaves its rows visible rather than
 * dropping them off the chapter.
 */
data class ChapterGroup(
    /** Absent for the rows in no folder: "unfiled" is not a folder anybody made. */
    val title: String?,
    val rows: List<Snippet>,
) {
    companion object {
        fun group(rows: List<Snippet>, folders: List<Folder>): List<ChapterGroup> {
            val groups = mutableListOf<ChapterGroup>()
            for (folder in folders) {
                val inFolder = rows.filter { it.folderId == folder.id }
                if (inFolder.isEmpty()) continue
                groups.add(ChapterGroup(title = folder.name, rows = inFolder))
            }
            val known = folders.map { it.id }.toSet()
            val loose = rows.filter { it.folderId == null || it.folderId !in known }
            if (loose.isNotEmpty()) groups.add(ChapterGroup(title = null, rows = loose))
            return groups
        }
    }
}
