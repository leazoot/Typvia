// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// The one rule of the key store that can be checked without a device: an
// entry name is not a path, and the name it resolves to must never change.
//
// The rest — that the AndroidKeyStore key exists, that what lands on disk is
// sealed — needs the platform's own key store and is verified on a device.

package dev.typvia.mobile.ffi

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Test

class AndroidKeyKeeperTest {
    /**
     * Entry names come from the layer above. A name that reached the
     * filesystem as written could carry `../` and put key material outside
     * the directory this class controls.
     */
    @Test
    fun `an entry name cannot escape the directory it is stored in`() {
        for (entry in listOf("../../etc/passwd", "a/b/c", "..", "with space", "")) {
            val name = AndroidKeyKeeper.fileName(entry)

            assertFalse("`$entry` produced a separator", name.contains('/'))
            assertFalse("`$entry` produced a dot", name.contains('.'))
            assertEquals("a digest is fixed width", 64, name.length)
        }
    }

    /**
     * Stable across launches. If this ever changed, a device would fail to
     * find the identity it wrote yesterday and would quietly become a new
     * one — which reads, on the other devices, as a stranger joining.
     */
    @Test
    fun `the same entry always resolves to the same file`() {
        assertEquals(
            AndroidKeyKeeper.fileName("sync.device"),
            AndroidKeyKeeper.fileName("sync.device"),
        )
        assertNotEquals(
            AndroidKeyKeeper.fileName("sync.device"),
            AndroidKeyKeeper.fileName("sync.device.2"),
        )
    }
}
