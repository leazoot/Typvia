// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile.ui

import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The design system's value layer, which is where its rules actually live.
 *
 * What is checked here is what the delivery says must be true on Android
 * specifically — the four places the two platforms part — and the shared
 * vocabulary that both of them are held to.
 */
class DesignSystemTest {
    @Test
    fun `chinese headings drop one weight and body does not`() {
        // Source Han Sans at the same nominal weight is heavier than PingFang;
        // a screen set the iOS way comes out black.
        assertEquals(FontWeight.SemiBold, TypviaType.Title2.weight(UiLanguage.En))
        assertEquals(FontWeight.Medium, TypviaType.Title2.weight(UiLanguage.Zh))
        assertEquals(
            TypviaType.Body.weight(UiLanguage.En),
            TypviaType.Body.weight(UiLanguage.Zh),
        )
    }

    @Test
    fun `the margin is the only thing the narrower screen takes it out of`() {
        // 360 has thirty fewer points than 390. The delivery spends them all
        // on the margin, and nothing on the type ladder.
        assertEquals(20.dp, Tokens.Space.screenPadding)
        assertEquals(30f, Tokens.Type.title2.value)
        assertEquals(17f, Tokens.Type.body.value)
    }

    @Test
    fun `the keyboard gives the system bar its eight points`() {
        assertEquals(252.dp, Tokens.Viewport.imeHeight)
        assertTrue(Tokens.Viewport.tileWidth < Tokens.Viewport.tileLeadWidth)
    }

    @Test
    fun `the platform's own hit floor is honoured rather than iOS's`() {
        assertEquals(48.dp, Tokens.Hit.minimum)
    }

    @Test
    fun `every kind has a two-letter mark and only the secret one is filled`() {
        assertEquals(8, TypeSort.entries.size)
        assertEquals(8, TypeSort.entries.map { it.code }.toSet().size)
        assertTrue(TypeSort.entries.all { it.code.length == 2 })
        assertEquals(listOf(TypeSort.Secret), TypeSort.entries.filter { it.isReversed })
    }

    @Test
    fun `a kind is read back by the core's own word for it`() {
        assertEquals(TypeSort.Secret, TypeSort.ofCoreType("sensitive"))
        assertEquals(TypeSort.AiAction, TypeSort.ofCoreType("ai_action"))
        // A kind this build does not know is not a kind; it is not guessed at.
        assertNull(TypeSort.ofCoreType("quantum"))
    }

    @Test
    fun `every kind is named in both languages and the two are different`() {
        val en = Translator(UiLanguage.En)
        val zh = Translator(UiLanguage.Zh)
        for (sort in TypeSort.entries) {
            assertTrue(sort.name(en).isNotBlank())
            assertTrue(sort.name(zh).isNotBlank())
            // A missing half is how a single-language screen turns into a
            // mixed one without anybody noticing.
            assertNotEquals(sort.name(en), sort.name(zh))
        }
    }

    /**
     * The plural rule lives in one place because written out at each call site
     * it came out as "1 pieces", "1 minutes", "1 devices".
     */
    @Test
    fun `a count of one takes the singular noun, and nothing else does`() {
        val en = Translator(UiLanguage.En)
        assertEquals("1 device", en.counted(1uL, "device", "devices", "1 台设备"))
        assertEquals("0 devices", en.counted(0uL, "device", "devices", "0 台设备"))
        assertEquals("2 devices", en.counted(2uL, "device", "devices", "2 台设备"))
        assertEquals("1 piece", en.pieces(1))
        assertEquals("3 pieces", en.pieces(3))
    }

    /** Chinese has no plural, so its half is passed whole and never branches. */
    @Test
    fun `the chinese half is rendered exactly as it was written`() {
        val zh = Translator(UiLanguage.Zh)
        assertEquals("1 台设备", zh.counted(1uL, "device", "devices", "1 台设备"))
        assertEquals("1 枚", zh.pieces(1))
    }

    @Test
    fun `one language is rendered, and an unknown locale renders english`() {
        assertEquals(UiLanguage.Zh, UiLanguage.resolve(listOf("zh-Hans-CN", "en-US")))
        assertEquals(UiLanguage.En, UiLanguage.resolve(listOf("de-DE")))
        assertEquals(UiLanguage.En, UiLanguage.resolve(emptyList()))
    }

    @Test
    fun `a caret's width follows from its height alone`() {
        assertEquals(2.dp, CaretShape.width(16.dp))
        assertEquals(3.dp, CaretShape.width(28.dp))
        assertEquals(4.dp, CaretShape.width(52.dp))
        assertEquals(5.dp, CaretShape.width(72.dp))
    }

    @Test
    fun `a breathing caret never goes all the way out`() {
        // A caret that vanishes reads as a bug rather than as waiting.
        assertTrue(CaretShape.BREATHE_FLOOR > 0f)
    }

    @Test
    fun `the curve is the same four numbers as the other platform`() {
        assertArrayEquals(floatArrayOf(0.23f, 1f, 0.32f, 1f), Tokens.Motion.enterCurve)
    }

    // The room bar no longer wraps, so `doorLineBreaks` and the four cases
    // that pinned it are gone with it. What they protected against — the
    // fourth door cut off mid-word at the largest system text size — is now
    // structural: the doors take an equal quarter each and the word sits under
    // its mark, so a long word wraps inside its own column instead of pushing
    // the one after it off the bar. Deleting a test whose subject no longer
    // exists is not the same as deleting a test to go green; the behaviour it
    // guarded is stated here so nobody re-adds a single-line row by accident.

    private fun assertArrayEquals(expected: FloatArray, actual: FloatArray) {
        assertEquals(expected.toList(), actual.toList())
    }
}

/**
 * The composing bench's arithmetic.
 *
 * The keyboard is the one surface where a number being wrong is felt rather
 * than seen: a band that does not add up puts the function row under the
 * system's navigation bar, and a typing rate that ignores its cap holds the
 * host app's field hostage for four seconds.
 */
class KeyboardBenchTest {
    @Test
    fun `the bands fit inside the height the service asks for`() {
        assertTrue(KeyboardBand.bands <= KeyboardBand.total)
        // What is left is the system's, and it is left alone: a function row
        // under the navigation bar is a function row people mis-tap.
        assertTrue(KeyboardBand.gutter.value >= 0f)
        assertEquals(252.dp, KeyboardBand.total)
    }

    @Test
    fun `a short body is typed at the delivery's rate`() {
        val plan = TypeIn.plan(4)
        assertTrue(plan is TypeIn.Plan.Character)
        assertEquals(
            Tokens.Motion.TYPE_IN_PER_CHARACTER.toDouble(),
            (plan as TypeIn.Plan.Character).intervalMs,
            0.0001,
        )
    }

    @Test
    fun `the cap is on the whole insertion, so a longer body types faster`() {
        val count = 60
        val plan = TypeIn.plan(count) as TypeIn.Plan.Character
        assertTrue(plan.intervalMs < Tokens.Motion.TYPE_IN_PER_CHARACTER)
        assertEquals(
            Tokens.Motion.TYPE_IN_CAP.toDouble(),
            plan.intervalMs * count,
            0.0001,
        )
    }

    @Test
    fun `past the point where the motion could be seen it simply lands`() {
        assertEquals(TypeIn.Plan.AtOnce, TypeIn.plan(5_000))
        assertEquals(TypeIn.Plan.AtOnce, TypeIn.plan(1))
        assertEquals(TypeIn.Plan.AtOnce, TypeIn.plan(0))
    }
}
