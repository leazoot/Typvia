// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ui.TypeSort
import dev.typvia.mobile.ui.UiPreference
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What the two rooms decide, without a screen.
 *
 * The rules under test are the ones a screenshot cannot check: that a shut
 * chapter publishes nothing, that a list of trigger words contains only
 * snippets that have one, and that the counts on screen are the core's rather
 * than the length of whatever page was fetched.
 */
class RoomsTest {
    @Test
    fun `a shut chapter publishes no count at all`() {
        val counts = TypeSort.entries.associateWith { 7u }
        val chapters = Chapter.assemble(counts = counts, vaultOpen = false)

        val secret = chapters.first { it.sort == TypeSort.Secret }
        assertTrue(secret.isShut)
        // Not zero, and not a dash: absent. A shut chapter that prints 0 is
        // telling the reader the vault is empty.
        assertNull(secret.count)
        assertTrue(chapters.filter { it.sort != TypeSort.Secret }.all { it.count == 7u })
    }

    @Test
    fun `an open vault makes the secret chapter an ordinary one`() {
        val chapters = Chapter.assemble(
            counts = mapOf(TypeSort.Secret to 3u),
            vaultOpen = true,
        )
        val secret = chapters.first { it.sort == TypeSort.Secret }
        assertEquals(3u, secret.count)
        assertTrue(!secret.isShut)
    }

    @Test
    fun `every kind gets a chapter, in the shared order`() {
        val chapters = Chapter.assemble(counts = emptyMap(), vaultOpen = true)
        assertEquals(TypeSort.entries, chapters.map { it.sort })
        // A kind the core reported nothing for has none, not "unknown".
        assertTrue(chapters.all { it.count == 0u })
    }

    @Test
    fun `the count on the shelf is the core's, not the page's length`() {
        val shelf = HomeShelf.assemble(total = 132u, recent = emptyList(), mostUsed = emptyList())
        assertEquals(132u, shelf.total)
        assertTrue(shelf.tiles.isEmpty())
    }

    @Test
    fun `the trigger list holds only snippets that have a trigger`() {
        val withTrigger = snippet(id = "a", trigger = ";deploy")
        val withoutOne = snippet(id = "b", trigger = null)
        val blankOne = snippet(id = "c", trigger = "   ")

        val shelf = HomeShelf.assemble(
            total = 3u,
            recent = listOf(withTrigger, withoutOne, blankOne),
            mostUsed = listOf(withoutOne, blankOne, withTrigger),
        )

        assertEquals(listOf("a"), shelf.triggers.map { it.id })
        // The cards are a different list and keep everything, in the order the
        // core gave them.
        assertEquals(listOf("a", "b", "c"), shelf.tiles.map { it.id })
    }

    @Test
    fun `the trigger list is capped at what the frames print`() {
        val many = (1..10).map { snippet(id = "s$it", trigger = ";t$it") }
        val shelf = HomeShelf.assemble(total = 10u, recent = many, mostUsed = many)
        assertEquals(HomeShelf.TRIGGER_LIMIT, shelf.triggers.size)
    }

    /**
     * The page asked of the usage view is deliberately larger than the number
     * of lines printed: the most-used snippets need not all have a trigger,
     * and asking for exactly three would silently shorten the list.
     */
    @Test
    fun `more rows are asked for than are printed`() {
        assertTrue(HomeShelf.TRIGGER_PAGE_LIMIT > HomeShelf.TRIGGER_LIMIT.toUInt())
    }

    private fun snippet(id: String, trigger: String?) = uniffi.typvia_mobile_ffi.Snippet(
        id = id,
        title = "Title $id",
        body = "body",
        snippetType = "text",
        description = null,
        folderId = null,
        trigger = trigger,
        triggerMode = null,
        language = null,
        securityLevel = "normal",
        isFavorite = false,
        isPinned = false,
        isEnabled = true,
        createdAt = 0,
        updatedAt = 0,
        lastUsedAt = null,
        usageCount = 0u,
        version = 1u,
        deletedAt = null,
    )
}

/**
 * The vault room and the settings contents page.
 *
 * The rule with teeth is the same one the other platform holds: a shut vault
 * publishes nothing about what is in it, and a page that has not read a value
 * yet says nothing rather than guessing at one.
 */
class VaultAndSettingsTest {
    /** The language choice the cases below are indifferent to. */
    private val following = UiPreference.Language.System

    @Test
    fun `a vault that was never made is absent, not shut`() {
        // "Locked" would send the reader looking for a key that does not exist.
        assertEquals(VaultPhase.Absent, VaultPhase.of(null))
        assertEquals(VaultPhase.Absent, VaultPhase.of(status(initialized = false)))
        assertEquals(VaultPhase.Shut, VaultPhase.of(status(initialized = true)))
        assertEquals(
            VaultPhase.Open,
            VaultPhase.of(status(initialized = true, unlocked = true)),
        )
    }

    @Test
    fun `secrets are ordered by when they were last taken out`() {
        val sorted = VaultOrder.sort(
            listOf(
                VaultEntry("never", "Recovery code", null),
                VaultEntry("old", "Cluster login", 1_000),
                VaultEntry("recent", "Deploy key", 9_000),
            ),
        )
        assertEquals(listOf("recent", "old", "never"), sorted.map { it.id })
    }

    @Test
    fun `the settings page says what is true and nothing when it has not read`() {
        val tr = dev.typvia.mobile.ui.Translator(dev.typvia.mobile.ui.UiLanguage.En)
        val summary = SettingsSummary(
            total = 12u,
            isSyncConfigured = false,
            deviceCount = 1,
            vaultPhase = VaultPhase.Shut,
        )
        assertEquals("not set up", SettingsChapter.Sync.value(summary, following, tr))
        assertEquals("shut", SettingsChapter.Vault.value(summary, following, tr))
        assertEquals("12 pieces", SettingsChapter.Data.value(summary, following, tr))
    }

    @Test
    fun `a configured account with one device says so rather than counting to one`() {
        val tr = dev.typvia.mobile.ui.Translator(dev.typvia.mobile.ui.UiLanguage.En)
        val alone = SettingsSummary(0u, isSyncConfigured = true, deviceCount = 1, vaultPhase = VaultPhase.Absent)
        assertEquals("this device only", SettingsChapter.Sync.value(alone, following, tr))
        val paired = alone.copy(deviceCount = 3)
        assertEquals("3 devices", SettingsChapter.Sync.value(paired, following, tr))
    }

    /**
     * "Not read" and "off" are different sentences, and only one of them is a
     * claim about the reader's phone. A contents page that prints "not on yet"
     * because it failed to ask is a page telling them to go and switch on
     * something that may already be on.
     */
    @Test
    fun `a value that could not be read is blank rather than a guess`() {
        val tr = dev.typvia.mobile.ui.Translator(dev.typvia.mobile.ui.UiLanguage.En)
        val unread = SettingsSummary(
            total = 0u,
            isSyncConfigured = false,
            deviceCount = 0,
            vaultPhase = VaultPhase.Absent,
            isKeyboardOn = null,
            aiEngines = null,
        )

        assertEquals("", SettingsChapter.Keyboard.value(unread, following, tr))
        assertEquals("", SettingsChapter.Ai.value(unread, following, tr))

        val read = unread.copy(isKeyboardOn = false, aiEngines = 0)
        assertEquals("not on yet", SettingsChapter.Keyboard.value(read, following, tr))
        assertEquals("not set up", SettingsChapter.Ai.value(read, following, tr))
    }

    @Test
    fun `the contents page prints every chapter the delivery gives it`() {
        // Six, in the delivery's order. A chapter left off the contents page
        // says the product does not do that thing.
        assertEquals(
            listOf(
                SettingsChapter.Sync,
                SettingsChapter.Keyboard,
                SettingsChapter.Ai,
                SettingsChapter.Vault,
                SettingsChapter.Appearance,
                SettingsChapter.Data,
            ),
            SettingsChapter.entries.toList(),
        )
    }

    /**
     * This line used to offer a third verb — "delete everything" — that the
     * data page does not have. A reader scans the contents page to decide what
     * to open, so a verb printed here is a reader sent looking for it.
     */
    @Test
    fun `the data chapter offers only what its page holds`() {
        val en = dev.typvia.mobile.ui.Translator(dev.typvia.mobile.ui.UiLanguage.En)
        val zh = dev.typvia.mobile.ui.Translator(dev.typvia.mobile.ui.UiLanguage.Zh)
        assertEquals("Export · import · the bin", SettingsChapter.Data.blurb(en))
        assertEquals("导出 · 导入 · 回收站", SettingsChapter.Data.blurb(zh))
    }

    @Test
    fun `every chapter is named in both languages and the halves differ`() {
        val en = dev.typvia.mobile.ui.Translator(dev.typvia.mobile.ui.UiLanguage.En)
        val zh = dev.typvia.mobile.ui.Translator(dev.typvia.mobile.ui.UiLanguage.Zh)
        for (chapter in SettingsChapter.entries) {
            assertTrue(chapter.title(en).isNotBlank())
            assertTrue(chapter.title(zh).isNotBlank())
            assertTrue(chapter.title(en) != chapter.title(zh))
        }
    }

    private fun status(
        initialized: Boolean,
        unlocked: Boolean = false,
    ) = uniffi.typvia_mobile_ffi.VaultStatus(
        initialized = initialized,
        unlocked = unlocked,
        unlockedAt = null,
        lastActivityAt = null,
        idleTimeoutMs = 300_000,
    )
}

/**
 * Underlining what the reader already typed.
 *
 * The split is presentation, and it is deliberately literal: it re-finds the
 * head of a trigger the reader typed themselves. It does not decide what
 * matched — that answer is the core's, and a screen that formed its own opinion
 * would underline things the search did not actually match on.
 */
class TriggerMatchTest {
    @Test
    fun `the head the reader typed is the part that gets underlined`() {
        val hit = TriggerMatch.of(";deploy", ";de")
        assertEquals(";de", hit.matched)
        assertEquals("ploy", hit.rest)
    }

    @Test
    fun `a query that is not the head of this trigger underlines nothing`() {
        // The core may well have matched this row on its title or its body.
        // Underlining a coincidence inside the trigger would be the screen
        // claiming to know why the row is here.
        val hit = TriggerMatch.of(";deploy", "rollback")
        assertEquals("", hit.matched)
        assertEquals(";deploy", hit.rest)
    }

    @Test
    fun `case is not what makes a trigger different`() {
        val hit = TriggerMatch.of(";Deploy", ";de")
        // The trigger keeps the case it was stored with; only the length of
        // what was typed decides where the split falls.
        assertEquals(";De", hit.matched)
        assertEquals("ploy", hit.rest)
    }

    @Test
    fun `an empty query underlines nothing at all`() {
        val hit = TriggerMatch.of(";deploy", "   ")
        assertEquals("", hit.matched)
        assertEquals(";deploy", hit.rest)
    }
}

/**
 * How much a chapter asks for at a time.
 *
 * The product is sized for fifty thousand snippets, which is never one query.
 * What is pinned here is the arithmetic the screen leans on — that it asks
 * ahead of the reader rather than when they have already run out.
 */
class ChapterPagingTest {
    @Test
    fun `the next page is asked for before the reader reaches the end`() {
        assertTrue(ChapterPaging.LOOKAHEAD > 0)
        // And the lookahead has to fit inside a page, or the request fires on
        // a row that does not exist yet.
        assertTrue(ChapterPaging.LOOKAHEAD.toUInt() < ChapterPaging.PAGE)
    }

    @Test
    fun `a page is small enough to draw and large enough to be worth a query`() {
        assertTrue(ChapterPaging.PAGE >= 20u)
        assertTrue(ChapterPaging.PAGE <= 200u)
    }
}
