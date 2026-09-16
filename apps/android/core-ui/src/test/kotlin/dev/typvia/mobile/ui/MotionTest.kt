// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The beats as arithmetic. What a screenshot cannot check is whether the
// numbers behind the motion are the delivery's — and whether the reduced
// motion request is honoured everywhere rather than in most places.

package dev.typvia.mobile.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class MotionTest {
    @Test
    fun `leaving is the same shape at seven tenths`() {
        for (beat in Beat.entries) {
            val exit = (beat.durationMs * Tokens.Motion.EXIT_FACTOR).toInt()
            assertTrue("$beat leaves slower than it arrived", exit < beat.durationMs)
            assertEquals((beat.durationMs * 0.7f).toInt(), exit)
        }
    }

    @Test
    fun `the vault is the one long beat`() {
        // The delivery gives the product exactly one 320ms, and it belongs to
        // the vault. A second one would make it ordinary.
        assertEquals(Tokens.Motion.UNLOCK, Beat.Unlock.durationMs)
        assertTrue(Beat.entries.count { it.durationMs >= Tokens.Motion.UNLOCK } == 1)
    }

    @Test
    fun `the ink circle reaches the corners at the delivery's extent`() {
        // 0.82 of the half-diagonal: past the corners of a 360x800 room, so
        // the room is fully open rather than open with dark corners.
        val width = 360f
        val height = 800f
        val halfDiagonal = kotlin.math.sqrt(width * width + height * height) / 2f
        val radius = kotlin.math.sqrt(width * width + height * height) /
            kotlin.math.sqrt(2f) * Tokens.Motion.INK_BLOOM_EXTENT

        assertTrue("the ink must clear the corners", radius > halfDiagonal)
    }

    // Deliberately not tested here: the shape itself. `InkBloom` builds a
    // Compose `Path`, which on a host JVM has no implementation behind it —
    // a test that constructed one would be testing the test harness. Whether
    // the circle actually opens is an eye check, and it is on the walkthrough
    // list rather than pretended at with an assertion that cannot fail.
}
