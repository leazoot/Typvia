// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.pieces
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypeSort
import dev.typvia.mobile.ui.TypeSortMark
import dev.typvia.mobile.ui.TypviaType

/**
 * The library: eight chapters, each a page of its own.
 *
 * This is the contents page. The kind is said by its two-letter mark and its
 * name — no per-kind colour, no icons — and the shut chapter says it is shut
 * instead of saying how much is behind it.
 */
@Composable
fun LibraryScreen(
    chapters: List<Chapter>?,
    isUnreadable: Boolean = false,
    onOpenChapter: ((TypeSort) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Paper.base)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        Text(
            text = tr("LIBRARY", "资料库"),
            style = TypviaType.MonoLabel.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = 14.dp, bottom = LibraryMetrics.headGap),
        )
        if (chapters == null) {
            Text(
                text = if (isUnreadable) {
                    tr("The local library could not be read.", "读不到本地库。")
                } else {
                    tr("reading…", "正在读…")
                },
                style = TypviaType.Mono.style(tr.language),
                color = Paper.ink3,
            )
            return@Column
        }
        for (chapter in chapters) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .then(
                        // A shut chapter has nothing to open, and a chapter
                        // with nothing in it opens onto nothing. Neither is
                        // offered as a way in.
                        if (onOpenChapter != null && !chapter.isShut && (chapter.count ?: 0u) > 0u) {
                            Modifier.clickable { onOpenChapter(chapter.sort) }
                        } else {
                            Modifier
                        },
                    )
                    .padding(vertical = LibraryMetrics.rowGap),
                horizontalArrangement = Arrangement.spacedBy(LibraryMetrics.markGap),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TypeSortMark(
                    sort = chapter.sort,
                    accessibilityLabel = chapter.sort.name(tr),
                )
                Text(
                    text = chapter.sort.name(tr),
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = Paper.ink,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    text = when {
                        // A shut chapter states that it is shut. It does not
                        // state a count, because a count is a fact about the
                        // vault and the vault is not open.
                        chapter.isShut -> tr("shut", "已锁")
                        // A chapter whose count could not be read says
                        // nothing about how many, rather than saying zero.
                        else -> chapter.count?.let { tr.pieces(it.toInt()) }.orEmpty()
                    },
                    style = TypviaType.Mono.style(tr.language),
                    color = Paper.ink3,
                )
            }
        }
    }
}

object LibraryMetrics {
    val headGap = 26.dp
    val rowGap = 14.dp
    val markGap = 14.dp
}

/**
 * One chapter, open: the rows it holds.
 *
 * The kind is said once, at the head, and then not repeated on every row —
 * every row here is that kind, and a mark on each of them would be saying the
 * same word eight times down the page.
 */
@Composable
fun ChapterScreen(
    sort: TypeSort,
    rows: List<uniffi.typvia_mobile_ffi.Snippet>?,
    onOpen: ((String) -> Unit)?,
    onClose: () -> Unit,
    onNeedMore: (() -> Unit)? = null,
    /**
     * The way to add one from inside the chapter a reader is browsing.
     *
     * Absent, this room is one you can only read: the home screen's verb was
     * the only way to write anything, so a reader standing in the chapter they
     * wanted to add to had to leave it first.
     */
    onCompose: (() -> Unit)? = null,
    /**
     * The reader's folders, for the one chapter that is filed under them. The
     * delivery groups only the text chapter: it is the largest, and the others
     * are short enough to read straight down.
     */
    folders: List<uniffi.typvia_mobile_ffi.Folder> = emptyList(),
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    // Lazy, because this is the one list in the product that is sized for a
    // library of fifty thousand: a column that composes every row it has been
    // handed is a column that stops being usable exactly when somebody has
    // enough snippets to need it.
    LazyColumn(
        modifier = modifier
            .fillMaxSize()
            .background(Paper.base)
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        item {
            Text(
                text = tr("Back", "返回"),
                style = TypviaType.BodyS.style(tr.language),
                color = Paper.ink2,
                modifier = Modifier
                    .padding(top = 14.dp, bottom = LibraryMetrics.headGap)
                    .clickable(onClick = onClose),
            )
            Text(
                text = sort.name(tr),
                style = TypviaType.Title2.style(tr.language),
                color = Paper.ink,
            )
        }
        if (rows == null) {
            item {
                Text(
                    text = tr("reading…", "正在读…"),
                    style = TypviaType.Mono.style(tr.language),
                    color = Paper.ink3,
                    modifier = Modifier.padding(top = LibraryMetrics.rowGap),
                )
            }
            return@LazyColumn
        }
        // Grouped only where the delivery groups: the text chapter under the
        // folders the reader made. Everywhere else the heading would be one
        // line of furniture over a list short enough to read straight down.
        val groups = if (sort == TypeSort.Text && folders.isNotEmpty()) {
            ChapterGroup.group(rows, folders)
        } else {
            listOf(ChapterGroup(title = null, rows = rows))
        }
        for (group in groups) {
            group.title?.let { name ->
                item(key = "folder:$name") {
                    Text(
                        // A label, not a row: it names what is under it and is
                        // not something to tap.
                        text = name,
                        style = TypviaType.MonoLabel.style(tr.language),
                        color = Paper.ink3,
                        modifier = Modifier.padding(top = LibraryMetrics.headGap),
                    )
                }
            }
            itemsIndexed(group.rows, key = { _, row -> row.id }) { index, row ->
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .then(if (onOpen != null) Modifier.clickable { onOpen(row.id) } else Modifier)
                    .padding(vertical = LibraryMetrics.rowGap),
            ) {
                Text(
                    text = row.title,
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = Paper.ink,
                )
                row.trigger?.let { trigger ->
                    Text(
                        text = trigger,
                        style = TypviaType.Mono.style(tr.language),
                        color = dev.typvia.mobile.ui.Room.Home.accent,
                    )
                }
            }
            // Asked for while the reader is still a screen away from the end,
            // so the next page is already there when they arrive. Counted
            // against the chapter rather than the group: the next page is the
            // chapter's, and a short last folder would otherwise never ask.
            if (row.id == rows.getOrNull(rows.lastIndex - ChapterPaging.LOOKAHEAD)?.id) {
                LaunchedEffect(rows.size) { onNeedMore?.invoke() }
            }
            }
        }
        onCompose?.let { compose ->
            item(key = "compose") {
                PaperVerb(
                    label = tr("Write one", "写一枚"),
                    isPrimary = true,
                    onTap = compose,
                )
            }
        }
    }
}

object ChapterPaging {
    /** How many rows a page holds. */
    const val PAGE: UInt = 50u

    /** How far from the end the next page is asked for. */
    const val LOOKAHEAD = 8
}
