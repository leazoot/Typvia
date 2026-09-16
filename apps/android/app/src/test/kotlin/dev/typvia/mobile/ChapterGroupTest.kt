// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Filing the largest chapter under the reader's own folders.
//
// The rule is the other platform's; these pin the three parts of it that are
// easy to get subtly wrong, and that a screenshot of a tidy library would not
// reveal: an empty folder heading, a lost row, and the order.

package dev.typvia.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.typvia_mobile_ffi.Folder
import uniffi.typvia_mobile_ffi.Snippet

class ChapterGroupTest {
    private fun folder(id: String, name: String) = Folder(
        id = id, parentId = null, name = name, sortOrder = 0, createdAt = 0, updatedAt = 0,
    )

    private fun row(id: String, folderId: String?) = Snippet(
        id = id, title = id, body = "body", snippetType = "text", securityLevel = "normal",
        description = null, folderId = folderId, trigger = null, triggerMode = null,
        language = null, isFavorite = false, isPinned = false, isEnabled = true,
        usageCount = 0u, lastUsedAt = null, createdAt = 0, updatedAt = 0, deletedAt = null,
        version = 1u,
    )

    @Test
    fun `rows are filed under their folders, in the folders' own order`() {
        val groups = ChapterGroup.group(
            rows = listOf(row("a", "f2"), row("b", "f1"), row("c", "f1")),
            folders = listOf(folder("f1", "Work"), folder("f2", "Home")),
        )

        assertEquals(listOf("Work", "Home"), groups.map { it.title })
        assertEquals(listOf("b", "c"), groups[0].rows.map { it.id })
    }

    @Test
    fun `an emptied folder gets no heading`() {
        val groups = ChapterGroup.group(
            rows = listOf(row("a", "f1")),
            folders = listOf(folder("f1", "Work"), folder("f2", "Empty")),
        )

        assertEquals(listOf("Work"), groups.map { it.title })
    }

    @Test
    fun `rows in no folder come last and carry no heading`() {
        val groups = ChapterGroup.group(
            rows = listOf(row("loose", null), row("a", "f1")),
            folders = listOf(folder("f1", "Work")),
        )

        assertEquals(2, groups.size)
        assertNull("unfiled is not a folder anybody made", groups.last().title)
        assertEquals(listOf("loose"), groups.last().rows.map { it.id })
    }

    @Test
    fun `a row in a folder this page did not fetch is still shown`() {
        // Dropping it would be the chapter quietly holding fewer snippets than
        // it says it does.
        val groups = ChapterGroup.group(
            rows = listOf(row("nested", "f9")),
            folders = listOf(folder("f1", "Work")),
        )

        assertEquals(listOf(null), groups.map { it.title })
        assertEquals(listOf("nested"), groups.single().rows.map { it.id })
    }

    @Test
    fun `with no folders at all the chapter is one plain run`() {
        val groups = ChapterGroup.group(
            rows = listOf(row("a", null), row("b", null)),
            folders = emptyList(),
        )

        assertEquals(1, groups.size)
        assertNull(groups.single().title)
    }
}
