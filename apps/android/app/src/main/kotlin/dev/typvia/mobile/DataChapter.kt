// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.counted
import uniffi.typvia_mobile_ffi.BackupRestored
import uniffi.typvia_mobile_ffi.CoreException
import uniffi.typvia_mobile_ffi.ImportReport
import uniffi.typvia_mobile_ffi.backupFormatMarker

/**
 * What kind of file the reader picked.
 *
 * The product does not guess where it cannot know. A `.md` or `.csv` file says
 * what it is; a `.json` file might be a Typvia backup, this product's own
 * export, a massCode dump or a CopyQ tab — so the backup is recognised by the
 * marker it actually carries, and the remaining three are **asked about**
 * rather than guessed at. A wrong guess here imports somebody's library as
 * gibberish.
 */
sealed interface PickedFile {
    /** A sealed Typvia backup. It needs a passphrase and an empty library. */
    data object Backup : PickedFile

    /** A file whose format is known from its name. [format] is the core's own
     * vocabulary, never a word invented here. */
    data class Known(val format: String) : PickedFile

    /** A `.json` that is not a backup: this product's export, massCode and
     * CopyQ all use the extension, and only the reader knows which. */
    data object AskWhichJson : PickedFile

    /** Nothing here can read that. */
    data object Unreadable : PickedFile

    companion object {
        /** The three a `.json` file could be, in the core's vocabulary. */
        val JSON_KINDS = listOf("json", "masscode", "copyq")

        /**
         * @param name the file's own name, as the picker reported it.
         * @param head the first part of the text — enough to carry the outer
         *   marker of a sealed backup, which is written in the clear.
         */
        fun of(name: String, head: String): PickedFile {
            if (head.contains("\"${backupFormatMarker()}\"")) return Backup
            return when (name.substringAfterLast('.', "").lowercase()) {
                "md", "markdown", "txt" -> Known("markdown")
                "csv" -> Known("csv")
                "json" -> AskWhichJson
                else -> Unreadable
            }
        }
    }
}

/**
 * What the data chapter says, as values.
 *
 * The rule this file holds: **a report says what happened to the library, and
 * a failure says what did not**. Import and restore are both all-or-nothing in
 * the core, so every sentence here can promise that much without hedging.
 */
object DataChapterCopy {
    /** What one import did, including what it would not take. */
    fun imported(report: ImportReport, tr: Translator): String {
        val lines = mutableListOf(
            if (report.imported == 1u) {
                tr("1 snippet came in.", "进来 1 枚片段。")
            } else {
                tr("${report.imported} snippets came in.", "进来 ${report.imported} 枚片段。")
            },
        )
        // Named, not counted: a reader who is told "3 conflicts" cannot go and
        // look at the three they already have.
        if (report.conflicts.isNotEmpty()) {
            lines += tr(
                "These triggers are already yours, so those entries stayed out: ${report.conflicts.joinToString(", ")}.",
                "这些触发词你已经用了,对应的条目没有进来:${report.conflicts.joinToString("、")}。",
            )
        }
        if (report.skipped.isNotEmpty()) {
            lines += if (report.skipped.size == 1) {
                tr("1 entry could not be read.", "有 1 条读不了。")
            } else {
                tr("${report.skipped.size} entries could not be read.", "有 ${report.skipped.size} 条读不了。")
            }
        }
        return lines.joinToString(" ")
    }

    /** What a restore brought back. Counts only — this page never opens a body. */
    fun restored(report: BackupRestored, tr: Translator): String {
        // Each noun decides its own plural; the sentence between them differs
        // by language, which is why the pieces are assembled and not formatted.
        val snippets = tr.counted(report.snippets.toULong(), "snippet", "snippets", "${report.snippets} 枚片段")
        val folders = tr.counted(report.folders.toULong(), "folder", "folders", "${report.folders} 个文件夹")
        val tags = tr.counted(report.tags.toULong(), "tag", "tags", "${report.tags} 个标签")
        val head = tr("Back: $snippets, $folders, $tags.", "回来了:$snippets、$folders、$tags。")
        val vault = if (report.vaultRestored) {
            tr(
                "The vault came with it, and it opens with the master password it had.",
                "保险库也回来了,用它原来的主密码打开。",
            )
        } else {
            tr("There was no vault in that backup.", "那份备份里没有保险库。")
        }
        return "$head $vault"
    }

    /** Why it did not happen — and, first, what is still true. */
    fun sentence(refusal: DataRefusal, tr: Translator): String {
        val said = when (refusal) {
            DataRefusal.LibraryNotEmpty -> tr(
                "A backup can only be restored into an empty library, and this one already has snippets in it.",
                "备份只能恢复到一个空库里,而这个库里已经有片段了。",
            )
            DataRefusal.DidNotOpen -> tr(
                "That backup did not open with that passphrase.",
                "这个口令打不开那份备份。",
            )
            DataRefusal.NotAFileThisReads -> tr(
                "Nothing here can read that file.",
                "这里读不了那种文件。",
            )
            DataRefusal.TooBig -> tr(
                "That file is larger than this can take.",
                "那个文件超过了能处理的大小。",
            )
            DataRefusal.TypedDifferently -> tr(
                "The two typings differ, so nothing was written.",
                "两次输入不一样,所以什么都没写。",
            )
            DataRefusal.Storage -> tr(
                "Something under the page failed.",
                "底下出了点问题。",
            )
        }
        return "$said ${nothingChanged(tr)}"
    }

    /** Said after every refusal on this page, because this is the one page
     * where the reader's whole library is at stake. */
    fun nothingChanged(tr: Translator): String =
        tr("Your library is exactly as it was.", "你的库和刚才一模一样。")
}

/** Why an import, export or restore did not happen. Never the raw error. */
enum class DataRefusal {
    LibraryNotEmpty,
    DidNotOpen,
    NotAFileThisReads,
    TooBig,
    TypedDifferently,
    Storage,
    ;

    companion object {
        fun of(error: Throwable): DataRefusal = when (error) {
            is CoreException.Conflict -> LibraryNotEmpty
            is CoreException.PermissionDenied -> DidNotOpen
            is CoreException.Validation -> NotAFileThisReads
            else -> Storage
        }
    }
}

/**
 * The acts behind the data chapter.
 *
 * What a format is, what may be imported, what a backup holds and what
 * restoring one requires are all the core's; this carries the question across
 * and hands back what came, failures included.
 */
object DataKeeper {
    suspend fun export(store: TypviaStore, passphrase: String): Result<String> =
        runCatching { store.perform { it.backupExport(passphrase) } }

    suspend fun import(store: TypviaStore, format: String, text: String): Result<ImportReport> =
        runCatching { store.perform { it.snippetsImport(format, text) } }

    suspend fun restore(
        store: TypviaStore,
        passphrase: String,
        text: String,
    ): Result<BackupRestored> =
        runCatching { store.perform { it.backupRestore(passphrase, text) } }
}
