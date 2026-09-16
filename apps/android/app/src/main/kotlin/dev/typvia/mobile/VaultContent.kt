// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import uniffi.typvia_mobile_ffi.VaultStatus

/**
 * The vault, as values.
 *
 * The shut vault publishes nothing: not the contents, not the count, not how
 * many attempts are left. What it says is that the things in it are on this
 * device, and that is the whole of it.
 */
enum class VaultPhase {
    /** Shut. Seven dots where a count would be. */
    Shut,
    Open,
    /** No vault has been made on this device yet. */
    Absent,
    ;

    companion object {
        fun of(status: VaultStatus?): VaultPhase = when {
            status == null -> Absent
            !status.initialized -> Absent
            status.unlocked -> Open
            else -> Shut
        }
    }
}

/** One secret, listed: a name and when it was last taken out — never a body. */
data class VaultEntry(val id: String, val title: String, val lastUsedAt: Long?)

/**
 * How the vault list is ordered: by when each secret was last taken out, most
 * recent first, with the never-taken ones after them.
 */
object VaultOrder {
    fun sort(entries: List<VaultEntry>): List<VaultEntry> =
        entries.sortedWith(
            compareByDescending<VaultEntry> { it.lastUsedAt ?: Long.MIN_VALUE }
                .thenBy { it.title },
        )
}
