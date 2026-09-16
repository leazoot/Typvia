// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Room
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypeSort
import dev.typvia.mobile.ui.TypeSortMark
import dev.typvia.mobile.ui.TypviaType
import androidx.compose.ui.Alignment
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.imePadding

/**
 * Writing one down.
 *
 * The whole page is the body: a name, the words, and the one verb. Kind,
 * trigger and folder are not here yet — the delivery puts them in a composing
 * stick, which this platform has not built, and offering half of one would be
 * worse than offering none.
 *
 * @param onSave hands back what was typed. Whether it can be saved — a trigger
 *   that collides, a kind that does not exist — is the core's answer, and it
 *   comes back as [refusal] rather than being guessed at here.
 */
@Composable
fun ComposeScreen(
    refusal: String?,
    onSave: (title: String, body: String, sort: TypeSort, trigger: String, folderId: String?) -> Unit,
    onCancel: () -> Unit,
    folders: List<uniffi.typvia_mobile_ffi.Folder> = emptyList(),
    onMakeFolder: ((String) -> Unit)? = null,
    initialTitle: String = "",
    initialBody: String = "",
    initialSort: TypeSort = TypeSort.Text,
    initialTrigger: String = "",
    initialFolderId: String? = null,
    /**
     * Whether this is a new snippet. Only a new one may be filed as a secret:
     * turning an existing one into a secret is a different act — re-encrypt,
     * rebuild the index, drop the plaintext history — and it is offered where
     * that snippet is read, not in the kind picker.
     */
    isNew: Boolean = true,
    /** The vault's door, when the kind chosen needs one. Drawn above the verb. */
    gate: (@Composable () -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    var title by remember { mutableStateOf(initialTitle) }
    var body by remember { mutableStateOf(initialBody) }
    var sort by remember { mutableStateOf(initialSort) }
    var trigger by remember { mutableStateOf(initialTrigger) }
    var folderId by remember { mutableStateOf(initialFolderId) }
    var namingFolder by remember { mutableStateOf(false) }
    var folderName by remember { mutableStateOf("") }
    // The body is what makes it saveable. A name is optional: leave it and the
    // shared layer files this under its first line, which is the same answer
    // the other platform gives.
    //
    // A secret is the exception, and deliberately so: a title is searchable,
    // so borrowing the first line of a secret would put the secret itself in
    // the index. It has to be named by hand, and the page says so rather than
    // sending something the vault will refuse.
    val needsName = sort == TypeSort.Secret && title.isBlank()
    val canSave = body.isNotBlank() && !needsName

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Paper.base)
            .verticalScroll(rememberScrollState())
            // Room for the keyboard, and a page that can scroll under it. The
            // data chapter has had both since it was built; this one — the
            // page whose whole job is typing — had neither.
            .imePadding()
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 14.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Verb(text = tr("Cancel", "取消"), isPrimary = false, onClick = onCancel)
            Verb(
                text = tr("Save", "存下"),
                isPrimary = true,
                enabled = canSave,
                onClick = { onSave(title, body, sort, trigger, folderId) },
            )
        }
        // The door, when the kind chosen needs one. It arrives in the
        // vault's own material — the material break is how this product says
        // *this is a different kind of thing*.
        gate?.invoke()
        Field(
            value = title,
            onValueChange = { title = it },
            placeholder = tr("Give it a name", "起个名字"),
            style = TypviaType.Title2,
            modifier = Modifier.padding(top = ComposeMetrics.titleTop),
        )
        if (needsName) {
            Text(
                text = tr(
                    "A secret needs a name of its own — a name can be searched, so this one cannot be borrowed from what you typed.",
                    "密钥要自己起个名字——名字是可以被搜到的,所以不能从正文里借。",
                ),
                style = TypviaType.BodyS.style(tr.language),
                color = Paper.ink2,
                modifier = Modifier.padding(top = ComposeMetrics.fieldGap),
            )
        }
        Field(
            value = body,
            onValueChange = { body = it },
            placeholder = tr("The words you keep typing", "你总在重复打的那句话"),
            style = TypviaType.Mono,
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = ComposeMetrics.bodyTop),
        )
        // The tools sit under the words, where a thumb reaches — the delivery
        // puts them in a strip along the bottom rather than in a top bar.
        Text(
            text = tr("KIND", "类型"),
            style = TypviaType.MonoLabel.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = ComposeMetrics.stickTop),
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = ComposeMetrics.fieldGap),
        ) {
            // Eight when writing something new, seven when editing: a secret
            // is written through the vault, and this platform can now open
            // one. Changing an existing snippet into a secret is not a change
            // of kind, so that mark stays out of this row.
            for (option in TypeSort.entries.filter { isNew || it != TypeSort.Secret }) {
                // The mark keeps the size the frames draw it; what a thumb has
                // to hit does not have to be the mark. Each takes an equal
                // share of the row and the platform's own minimum height, so
                // eight small squares become eight targets a finger can land
                // on without aiming.
                Box(
                    contentAlignment = Alignment.Center,
                    modifier = Modifier
                        .weight(1f)
                        .heightIn(min = Tokens.Hit.minimum)
                        .clickable { sort = option },
                ) {
                    TypeSortMark(
                        sort = option,
                        inverted = option == sort,
                        accessibilityLabel = option.name(tr),
                    )
                }
            }
        }
        Text(
            text = tr("TRIGGER", "触发词"),
            style = TypviaType.MonoLabel.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = ComposeMetrics.stickTop),
        )
        Field(
            value = trigger,
            onValueChange = { trigger = it },
            placeholder = tr("none", "没有"),
            style = TypviaType.Mono,
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = ComposeMetrics.fieldGap),
        )
        // Always here, even with nothing to offer yet: a field that only shows
        // up once a folder exists is a field nobody can get their first folder
        // from.
        Text(
            text = tr("FOLDER", "文件夹"),
            style = TypviaType.MonoLabel.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = ComposeMetrics.stickTop),
        )
        Row(
            modifier = Modifier.padding(top = ComposeMetrics.fieldGap),
            horizontalArrangement = Arrangement.spacedBy(ComposeMetrics.stickTop),
        ) {
            for (folder in folders) {
                Text(
                    text = folder.name,
                    style = TypviaType.BodyS.style(tr.language),
                    color = if (folder.id == folderId) Paper.ink else Paper.ink3,
                    modifier = Modifier.clickable {
                        folderId = if (folderId == folder.id) null else folder.id
                    },
                )
            }
            if (onMakeFolder != null && !namingFolder) {
                Text(
                    text = tr("New folder", "新建文件夹"),
                    style = TypviaType.BodyS.style(tr.language),
                    color = Paper.ink2,
                    modifier = Modifier.clickable { namingFolder = true },
                )
            }
        }
        if (namingFolder && onMakeFolder != null) {
            Row(
                modifier = Modifier.padding(top = ComposeMetrics.fieldGap),
                horizontalArrangement = Arrangement.spacedBy(ComposeMetrics.stickTop),
            ) {
                Field(
                    value = folderName,
                    onValueChange = { folderName = it },
                    placeholder = tr("Name it", "起个名字"),
                    style = TypviaType.BodyS,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    text = tr("Make it", "建"),
                    style = TypviaType.SectionTitle.style(tr.language),
                    color = Paper.ink,
                    modifier = Modifier.clickable {
                        onMakeFolder(folderName)
                        folderName = ""
                        namingFolder = false
                    },
                )
            }
        }
        if (refusal != null) {
            // On the line under what caused it, never in a dialog: the whole
            // product has one modal and this is not it.
            Text(
                text = refusal,
                style = TypviaType.Caption.style(tr.language),
                color = Paper.attention,
                modifier = Modifier.padding(top = ComposeMetrics.refusalTop),
            )
        }
    }
}

@Composable
private fun Field(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    style: TypviaType,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    val room = LocalRoomAccent()
    var isFocused by remember { mutableStateOf(false) }
    Box(modifier = modifier) {
        // Behind the field, not under it: a placeholder on its own line is a
        // second line of text, and the reader starts typing beneath their own
        // prompt. Gone the moment there is a caret — it used to look only at
        // whether the field was empty, so the hint and the caret were drawn on
        // top of each other.
        if (value.isEmpty() && !isFocused) {
            Text(
                text = placeholder,
                style = style.style(tr.language),
                color = Paper.ink3,
            )
        }
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = style.style(tr.language).copy(color = Paper.ink),
            cursorBrush = SolidColor(room),
            modifier = Modifier
                .fillMaxWidth()
                .onFocusChanged { isFocused = it.isFocused },
        )
    }
}

@Composable
private fun LocalRoomAccent() = Room.Home.accent

@Composable
private fun Verb(
    text: String,
    isPrimary: Boolean,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    val tr = LocalTranslator.current
    Text(
        text = text,
        style = (if (isPrimary) TypviaType.SectionTitle else TypviaType.BodyS).style(tr.language),
        color = when {
            !enabled -> Paper.ink3
            isPrimary -> Paper.ink
            else -> Paper.ink2
        },
        modifier = Modifier
            .padding(vertical = 12.dp)
            .clickable(enabled = enabled, onClick = onClick),
    )
}

object ComposeMetrics {
    val stickTop = 26.dp
    val fieldGap = 8.dp
    val markGap = 6.dp
    val titleTop = 26.dp
    val bodyTop = 26.dp
    val refusalTop = 14.dp
}
