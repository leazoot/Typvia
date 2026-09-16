// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.Caret
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Beat
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Room
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypeSortMark
import dev.typvia.mobile.ui.TypviaType
import dev.typvia.mobile.ui.systemReduceMotion

/**
 * Home: one subject to a screen, and the subject is the search position.
 *
 * The top of the page is given over to it — a caret, the word, and how much
 * there is to search. Under it the recall cards, and under those the trigger
 * words the reader types most. No icons, no cards inside cards.
 */
@Composable
fun HomeScreen(
    shelf: HomeShelf?,
    isUnreadable: Boolean = false,
    onCompose: (() -> Unit)? = null,
    onOpen: ((String) -> Unit)? = null,
    query: String = "",
    onQueryChange: ((String) -> Unit)? = null,
    results: List<RecallTile>? = null,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    var isFocused by remember { mutableStateOf(false) }
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Paper.base)
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        Text(
            text = "TYPVIA",
            style = TypviaType.MonoLabel.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = 14.dp),
        )
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(top = HomeMetrics.searchTop),
        ) {
            // One caret, not two. While the field has focus the system draws
            // the one that moves with the text; drawing ours beside it put two
            // bars on a row the delivery gives exactly one.
            if (!isFocused) {
                Caret(height = HomeMetrics.caretHeight, breathing = query.isEmpty())
            }
            Box(modifier = Modifier.padding(start = HomeMetrics.caretGap)) {
                if (query.isEmpty()) {
                    Text(
                        text = tr("Search snippets", "搜索片段"),
                        style = TypviaType.SearchInput.style(tr.language),
                        color = Paper.ink3,
                    )
                }
                if (onQueryChange != null) {
                    BasicTextField(
                        value = query,
                        onValueChange = onQueryChange,
                        singleLine = true,
                        textStyle = TypviaType.SearchInput.style(tr.language)
                            .copy(color = Paper.ink),
                        cursorBrush = SolidColor(Room.Home.accent),
                        modifier = Modifier
                            .fillMaxWidth()
                            .onFocusChanged { isFocused = it.isFocused },
                    )
                }
            }
        }
        // No border, only the rule under it: the most expensive thing on this
        // screen is the space the search position takes, and a box around it
        // does not earn its share.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = HomeMetrics.lineGap)
                .height(Tokens.Line.searchHeight)
                .background(
                    Paper.ink.copy(
                        alpha = if (query.isEmpty() && !isFocused) {
                            Tokens.Line.SEARCH_REST_OPACITY
                        } else {
                            Tokens.Line.SEARCH_FOCUS_OPACITY
                        },
                    ),
                ),
        )
        Text(
            // The count is what the core said, not what is on screen. A shelf
            // that prints its own length is a shelf that reports its page size
            // as the size of the library.
            text = when {
                // Three states, not two. A screen that says "reading" while it
                // has stopped reading is the most patient kind of lie.
                isUnreadable -> tr(
                    "The local library could not be read.",
                    "读不到本地库。",
                )
                shelf == null -> tr("reading…", "正在读…")
                else -> tr("${shelf.total} in reach", "${shelf.total} 枚可用")
            },
            style = TypviaType.Mono.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = HomeMetrics.countTop),
        )
        when {
            // Every keystroke replaces the results outright: no debounce, no
            // transition. A list that fades between answers is a list the
            // reader has to wait for.
            query.isNotBlank() -> Results(results, query, onOpen)
            shelf != null -> Shelf(shelf, onCompose, onOpen)
        }
    }
}

@Composable
private fun Shelf(shelf: HomeShelf, onCompose: (() -> Unit)?, onOpen: ((String) -> Unit)?) {
    val tr = LocalTranslator.current
    Column(modifier = Modifier.padding(top = Tokens.Space.group)) {
        // The invitation belongs to an empty library, not to an empty recall
        // order. `recent` holds what has been used, so a library with one
        // never-used snippet in it has no cards — and telling that reader
        // "this is still a blank sheet" while the line above says "1 in reach"
        // is the screen contradicting itself.
        if (shelf.total == 0u) {
            Text(
                text = tr(
                    "This is still a blank sheet.",
                    "这里还是一张白纸。",
                ),
                style = TypviaType.Title2.style(tr.language),
                color = Paper.ink,
            )
            Text(
                text = tr(
                    "Save the line you have already typed twice today as the first one.",
                    "把你今天已经重复输入过两次的那句话,存成第一枚片段。",
                ),
                style = TypviaType.BodyS.style(tr.language),
                color = Paper.ink2,
                modifier = Modifier.padding(top = HomeMetrics.lineGap),
            )
            // The invitation only appears where it can be taken up. An empty
            // state that asks for a first snippet on a screen with no way to
            // write one is asking for something it will not accept.
            if (onCompose != null) {
                Text(
                    text = tr("Save the first one", "存下第一枚"),
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = Paper.ink,
                    modifier = Modifier
                        .padding(top = Tokens.Space.group)
                        .clickable(onClick = onCompose),
                )
            }
            return@Column
        }
        if (shelf.tiles.isNotEmpty()) {
            Text(
                // Said out loud, because it is not "everything": these are the
                // ones that have been reached for.
                text = tr("RECENTLY USED", "最近用过"),
                style = TypviaType.MonoLabel.style(tr.language),
                color = Paper.ink3,
                modifier = Modifier.padding(bottom = HomeMetrics.lineGap),
            )
            LazyRow(horizontalArrangement = Arrangement.spacedBy(HomeMetrics.tileGap)) {
                items(shelf.tiles.size, key = { shelf.tiles[it].id }) { index ->
                    // Sheets are laid down one after another rather than all
                    // at once — the delivery's 70ms step. `animateItem` keeps
                    // a sheet that is still on the shelf *moving* to its new
                    // place when the shelf is re-ordered, instead of being
                    // torn down and drawn again somewhere else.
                    Arriving(index = index, modifier = Modifier.animateItem()) {
                        RecallCard(shelf.tiles[index], onOpen)
                    }
                }
            }
        }
        // The way to write another one.
        //
        // It used to live only in the empty state, which meant the product
        // could be used to save exactly one snippet and then never again —
        // the invitation disappeared with the blank sheet it was printed on.
        if (onCompose != null && shelf.total > 0u) {
            Text(
                text = tr("Write one", "写一枚"),
                style = TypviaType.SectionTitle.style(tr.language),
                color = Paper.ink,
                modifier = Modifier
                    .padding(top = Tokens.Space.group)
                    .clickable(onClick = onCompose),
            )
        }
        if (shelf.triggers.isEmpty()) return@Column
        Text(
            text = tr("THE ONES YOU TYPE MOST", "常用触发词"),
            style = TypviaType.MonoLabel.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = Tokens.Space.group),
        )
        for (line in shelf.triggers) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = HomeMetrics.lineGap),
                horizontalArrangement = Arrangement.spacedBy(HomeMetrics.lineGap),
            ) {
                Text(
                    text = line.trigger,
                    style = TypviaType.Mono.style(tr.language),
                    color = Room.Home.accent,
                )
                Text(
                    text = line.title,
                    style = TypviaType.BodyS.style(tr.language),
                    color = Paper.ink2,
                )
            }
        }
    }
}

@Composable
private fun Results(results: List<RecallTile>?, query: String, onOpen: ((String) -> Unit)?) {
    val tr = LocalTranslator.current
    Column(modifier = Modifier.padding(top = Tokens.Space.group)) {
        if (results == null) {
            Text(
                text = tr("reading…", "正在读…"),
                style = TypviaType.Mono.style(tr.language),
                color = Paper.ink3,
            )
            return@Column
        }
        if (results.isEmpty()) {
            Text(
                // The empty result is also a way in; what it must not be is a
                // dead end that only says no.
                text = tr("Nothing under that word yet.", "这个词还没有对应的片段。"),
                style = TypviaType.BodyS.style(tr.language),
                color = Paper.ink2,
            )
            return@Column
        }
        for (row in results) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .then(if (onOpen != null) Modifier.clickable { onOpen(row.id) } else Modifier)
                    .padding(vertical = HomeMetrics.lineGap),
                horizontalArrangement = Arrangement.spacedBy(HomeMetrics.tileGap),
            ) {
                row.sort?.let { sort ->
                    TypeSortMark(sort = sort, accessibilityLabel = sort.name(tr))
                }
                Text(
                    text = row.title,
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = Paper.ink,
                    modifier = Modifier.weight(1f),
                )
                row.trigger?.let { trigger ->
                    val hit = TriggerMatch.of(trigger, query)
                    Text(
                        text = buildAnnotatedString {
                            withStyle(SpanStyle(textDecoration = TextDecoration.Underline)) {
                                append(hit.matched)
                            }
                            append(hit.rest)
                        },
                        style = TypviaType.Mono.style(tr.language),
                        color = Room.Home.accent,
                    )
                }
            }
        }
    }
}

@Composable
private fun RecallCard(tile: RecallTile, onOpen: ((String) -> Unit)?) {
    val tr = LocalTranslator.current
    Column(
        modifier = Modifier
            .width(HomeMetrics.cardWidth)
            .background(Paper.carrier)
            .then(
                // A card that cannot be opened is not offered as one.
                if (onOpen != null) Modifier.clickable { onOpen(tile.id) } else Modifier,
            )
            .padding(HomeMetrics.cardPadding),
        verticalArrangement = Arrangement.spacedBy(HomeMetrics.lineGap),
    ) {
        tile.sort?.let { sort ->
            TypeSortMark(sort = sort, accessibilityLabel = sort.name(tr))
        }
        Text(
            text = tile.title,
            style = TypviaType.SectionTitle.style(tr.language),
            color = Paper.ink,
        )
        tile.trigger?.let { trigger ->
            Text(
                text = trigger,
                style = TypviaType.Mono.style(tr.language),
                color = Room.Home.accent,
            )
        }
    }
}

object HomeMetrics {
    /** The delivery's step between one sheet landing and the next. */
    const val STAGGER_STEP_MS = 70L

    /**
     * How many sheets are staggered before the rest simply land. Past this a
     * reader is waiting rather than watching.
     */
    const val STAGGER_CAP = 5

    /** How far a sheet rises into place. Six points, as the frames give it. */
    const val ARRIVAL_RISE = 6f

    val searchTop = 40.dp
    val caretHeight = 30.dp
    val caretGap = 10.dp
    val countTop = 12.dp
    val lineGap = 8.dp
    val tileGap = 12.dp

    /** 156 → 146 on the narrower screen, still showing two and a half. */
    val cardWidth = 146.dp
    val cardPadding = 14.dp
}

/**
 * One thing arriving on the shelf, in its turn.
 *
 * The step is the delivery's 70ms, capped so a long shelf does not turn into a
 * queue: past the cap everything lands together, because a reader waiting for
 * the eighth sheet is not watching an animation, they are waiting.
 *
 * Reduced motion skips the stagger entirely — it is the same request that
 * turns off everything else, and honouring it partly is not honouring it.
 */
@Composable
private fun Arriving(index: Int, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val reduceMotion = systemReduceMotion()
    val progress = remember(index) { Animatable(if (reduceMotion) 1f else 0f) }
    LaunchedEffect(index, reduceMotion) {
        if (reduceMotion) {
            progress.snapTo(1f)
            return@LaunchedEffect
        }
        kotlinx.coroutines.delay(minOf(index, HomeMetrics.STAGGER_CAP) * HomeMetrics.STAGGER_STEP_MS)
        progress.animateTo(1f, Beat.Transition.enter(false))
    }
    Box(
        modifier = modifier
            .alpha(progress.value)
            .graphicsLayer { translationY = (1f - progress.value) * HomeMetrics.ARRIVAL_RISE },
    ) {
        content()
    }
}
