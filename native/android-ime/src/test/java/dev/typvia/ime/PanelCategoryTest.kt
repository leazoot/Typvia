// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.typvia_mobile_ffi.SnapshotEntry
import uniffi.typvia_mobile_ffi.SnapshotFolder

/** Factory keeps tests independent of the FFI field order. */
private fun entry(
    id: String = "s1",
    title: String = "Title",
    snippetType: String = "text",
    trigger: String? = null,
    folderId: String? = null,
    isFavorite: Boolean = false,
    isRecent: Boolean = false,
    isSensitive: Boolean = false,
) = SnapshotEntry(id, title, snippetType, trigger, folderId, isFavorite, isRecent, isSensitive)

class PanelCategoryTest {
    @Test
    fun availableListsOnlyPopulatedCategories() {
        val chips =
            PanelCategory.available(
                listOf(entry(isRecent = true), entry(id = "s2", snippetType = "template")),
                listOf(SnapshotFolder("f1", "Email")),
            )
        assertEquals(
            listOf(
                PanelCategory.Recent,
                PanelCategory.Folder("f1", "Email"),
                PanelCategory.Templates,
            ),
            chips,
        )
    }

    @Test
    fun emptySnapshotYieldsNoChips() {
        assertEquals(emptyList<PanelCategory>(), PanelCategory.available(emptyList(), emptyList()))
    }

    @Test
    fun vaultAndStarredAppearWhenPresent() {
        val chips =
            PanelCategory.available(
                listOf(
                    entry(isFavorite = true),
                    entry(id = "s9", title = "", snippetType = "", isSensitive = true),
                ),
                emptyList(),
            )
        assertEquals(listOf(PanelCategory.Starred, PanelCategory.Vault), chips)
    }

    @Test
    fun matchesFiltersByCategory() {
        val inFolder = entry(folderId = "f1")
        val sensitive = entry(id = "s9", isSensitive = true)
        assertTrue(PanelCategory.Folder("f1", "Email").matches(inFolder))
        assertFalse(PanelCategory.Folder("f2", "Shell").matches(inFolder))
        assertTrue(PanelCategory.Vault.matches(sensitive))
        assertFalse(PanelCategory.Vault.matches(inFolder))
        assertTrue(PanelCategory.Recent.matches(entry(isRecent = true)))
        assertFalse(PanelCategory.Recent.matches(inFolder))
        assertTrue(PanelCategory.Templates.matches(entry(snippetType = "template")))
        assertTrue(PanelCategory.Starred.matches(entry(isFavorite = true)))
    }

    @Test
    fun labelsMatchTheDesignVocabulary() {
        assertEquals("Recent", PanelCategory.Recent.label)
        assertEquals("Favorites", PanelCategory.Starred.label)
        assertEquals("Email", PanelCategory.Folder("f1", "Email").label)
        assertEquals("Templates", PanelCategory.Templates.label)
        assertEquals("Vault", PanelCategory.Vault.label)
    }
}
