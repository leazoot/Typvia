// Category tabs of the expanded panel's footer rail (Recent · Favorites ·
// folders · Templates · Vault). Tabs render only when the snapshot actually
// holds matching data — no dead filters. Mirrors the iOS PanelCategory.

package dev.typvia.ime

import uniffi.typvia_mobile_ffi.SnapshotEntry
import uniffi.typvia_mobile_ffi.SnapshotFolder

sealed class PanelCategory {
    object Recent : PanelCategory()

    object Starred : PanelCategory()

    data class Folder(val id: String, val name: String) : PanelCategory()

    object Templates : PanelCategory()

    object Vault : PanelCategory()

    val label: String
        get() =
            when (this) {
                is Recent -> "Recent"
                is Starred -> "Favorites"
                is Folder -> name
                is Templates -> "Templates"
                is Vault -> "Vault"
            }

    fun matches(entry: SnapshotEntry): Boolean =
        when (this) {
            is Recent -> entry.isRecent
            is Starred -> entry.isFavorite
            is Folder -> entry.folderId == id
            is Templates -> entry.snippetType == "template"
            is Vault -> entry.isSensitive
        }

    companion object {
        fun available(
            entries: List<SnapshotEntry>,
            folders: List<SnapshotFolder>,
        ): List<PanelCategory> {
            val chips = mutableListOf<PanelCategory>()
            if (entries.any { it.isRecent }) chips.add(Recent)
            if (entries.any { it.isFavorite }) chips.add(Starred)
            folders.forEach { chips.add(Folder(it.id, it.name)) }
            if (entries.any { it.snippetType == "template" }) chips.add(Templates)
            if (entries.any { it.isSensitive }) chips.add(Vault)
            return chips
        }
    }
}
