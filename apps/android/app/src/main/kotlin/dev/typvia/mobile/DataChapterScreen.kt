// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.ByteArrayOutputStream
import java.io.InputStream
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.Tokens
import dev.typvia.mobile.ui.TypviaType
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.typvia_mobile_ffi.backupMaxBytes
import uniffi.typvia_mobile_ffi.importMaxBytes

/**
 * The data chapter: the library's way out and its way back in.
 *
 * Everything on this page is about the whole library at once, so every
 * sentence on it says what happened to the whole library — and every refusal
 * says, first, that nothing changed. This is the one page where being vague
 * costs the reader everything they have.
 */
@Composable
fun DataChapterScreen(
    store: TypviaStore,
    onClose: () -> Unit,
    onOpenBin: () -> Unit,
    onLibraryChanged: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tr = LocalTranslator.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    var passphrase by remember { mutableStateOf("") }
    var again by remember { mutableStateOf("") }
    var restorePassphrase by remember { mutableStateOf("") }
    /** The file the reader picked, once it is known what kind it is. */
    var picked by remember { mutableStateOf<PickedFile?>(null) }
    var pickedName by remember { mutableStateOf<String?>(null) }
    var pickedText by remember { mutableStateOf<String?>(null) }
    /** Which of the three JSON kinds the reader said it is. */
    var jsonKind by remember { mutableStateOf<String?>(null) }
    // Two reports, not one: a sentence about the export belongs under the
    // export verb and a sentence about a file belongs under the file. Held in
    // one place at the foot of the page, they landed below the fold — the
    // reader tapped, the picker closed, and the page looked like nothing had
    // happened. An answer nobody scrolls to is an answer nobody gets.
    var exportSaid by remember { mutableStateOf<String?>(null) }
    var exportRefusal by remember { mutableStateOf<DataRefusal?>(null) }
    var intakeSaid by remember { mutableStateOf<String?>(null) }
    var intakeRefusal by remember { mutableStateOf<DataRefusal?>(null) }
    var isWorking by remember { mutableStateOf(false) }

    fun forget() {
        picked = null
        pickedName = null
        pickedText = null
        jsonKind = null
        restorePassphrase = ""
    }

    val writeBackup = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/json"),
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            isWorking = true
            exportRefusal = null
            DataKeeper.export(store, passphrase).fold(
                onSuccess = { document ->
                    val written = withContext(Dispatchers.IO) { write(context, uri, document) }
                    // The passphrase is dropped either way: it was carried
                    // across once, and this page keeps no copy of it.
                    passphrase = ""
                    again = ""
                    if (written) {
                        exportSaid = tr(
                            "Everything is in that file, sealed with the passphrase you typed. Nothing but you and that passphrase opens it.",
                            "所有东西都在那个文件里,用你刚输入的口令封着。除了你和那句口令,没有别的能打开它。",
                        )
                    } else {
                        exportRefusal = DataRefusal.Storage
                    }
                },
                onFailure = { exportRefusal = DataRefusal.of(it) },
            )
            isWorking = false
        }
    }

    val pickFile = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            isWorking = true
            intakeSaid = null
            intakeRefusal = null
            forget()
            when (val read = withContext(Dispatchers.IO) { read(context, uri) }) {
                is FileRead.Ok -> {
                    pickedName = read.name
                    pickedText = read.text
                    picked = PickedFile.of(read.name, read.text.take(HEAD_CHARS))
                    if (picked == PickedFile.Unreadable) {
                        intakeRefusal = DataRefusal.NotAFileThisReads
                    }
                }
                FileRead.TooBig -> intakeRefusal = DataRefusal.TooBig
                FileRead.Unreadable -> intakeRefusal = DataRefusal.NotAFileThisReads
            }
            isWorking = false
        }
    }

    fun bringIn(format: String) {
        val text = pickedText ?: return
        scope.launch {
            isWorking = true
            intakeRefusal = null
            DataKeeper.import(store, format, text).fold(
                onSuccess = { report ->
                    intakeSaid = DataChapterCopy.imported(report, tr)
                    forget()
                    onLibraryChanged()
                },
                onFailure = { intakeRefusal = DataRefusal.of(it) },
            )
            isWorking = false
        }
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .background(Paper.base)
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(horizontal = Tokens.Space.screenPadding),
    ) {
        Text(
            text = tr("Back", "返回"),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier
                .padding(top = 14.dp, bottom = SettingsMetrics.headGap)
                .clickable(onClick = onClose),
        )
        Text(
            text = "06",
            style = TypviaType.Title1.style(tr.language),
            color = Paper.ink3,
        )
        Text(
            text = tr("Your data", "你的数据"),
            style = TypviaType.Title2.style(tr.language),
            color = Paper.ink,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )
        Text(
            // Stated once, at the top, because it is what makes the rest of
            // the page safe to use: nothing here phones anywhere.
            text = tr(
                "A backup is one file, made on this device and sealed with a passphrase you choose. It goes wherever you put it.",
                "备份是一个文件,在这台设备上生成,用你自己定的口令封好。放到哪里由你决定。",
            ),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink2,
            modifier = Modifier.padding(top = SettingsMetrics.rowGap),
        )

        Label(tr("TAKE EVERYTHING OUT", "把全部带走"))
        PaperField(
            label = tr("BACKUP PASSPHRASE", "备份口令"),
            value = passphrase,
            isSecret = true,
            onValueChange = { passphrase = it },
        )
        PaperField(
            label = tr("AGAIN", "再来一次"),
            value = again,
            isSecret = true,
            onValueChange = { again = it },
        )
        Text(
            // The reason this page asks twice, said rather than assumed: there
            // is no way to check the passphrase later, because nothing here
            // keeps it.
            text = tr(
                "Typed twice because nothing keeps it. A backup whose passphrase went in wrong cannot be opened by anyone, including you.",
                "要输两次,因为没有任何地方存着它。口令输错的备份,谁也打不开,包括你自己。",
            ),
            style = TypviaType.Caption.style(tr.language),
            color = Paper.ink3,
            modifier = Modifier.padding(top = DataMetrics.innerGap),
        )
        PaperVerb(
            isPrimary = true,
            label = if (isWorking) {
                tr("Working…", "正在处理…")
            } else {
                tr("Export everything", "导出全部")
            },
            enabled = passphrase.isNotBlank() && !isWorking,
        ) {
            if (!VaultKeeper.typedTwiceMatches(passphrase, again)) {
                exportRefusal = DataRefusal.TypedDifferently
                return@PaperVerb
            }
            exportRefusal = null
            exportSaid = null
            writeBackup.launch("Typvia-backup-${System.currentTimeMillis()}.json")
        }
        Report(said = exportSaid, refusal = exportRefusal)

        Label(tr("BRING SOMETHING IN", "把东西带进来"))
        pickedName?.let { name ->
            Text(
                text = name,
                style = TypviaType.Mono.style(tr.language),
                color = Paper.ink,
                modifier = Modifier.padding(top = DataMetrics.innerGap),
            )
        }
        when (val current = picked) {
            null, PickedFile.Unreadable -> Text(
                text = tr(
                    "A Typvia backup, or a Markdown, CSV or JSON file of snippets.",
                    "一份 Typvia 备份,或者 Markdown、CSV、JSON 格式的片段文件。",
                ),
                style = TypviaType.Caption.style(tr.language),
                color = Paper.ink3,
                modifier = Modifier.padding(top = DataMetrics.innerGap),
            )

            PickedFile.Backup -> {
                Text(
                    text = tr(
                        "That is a sealed backup. Restoring puts a whole library back, so it only goes into an empty one — nothing is merged and nothing is overwritten.",
                        "那是一份封好的备份。恢复是把一整个库放回来,所以它只能放进一个空库——不合并,也不覆盖。",
                    ),
                    style = TypviaType.BodyS.style(tr.language),
                    color = Paper.ink2,
                    modifier = Modifier.padding(top = DataMetrics.innerGap),
                )
                PaperField(
                    label = tr("ITS PASSPHRASE", "它的口令"),
                    value = restorePassphrase,
                    isSecret = true,
                    onValueChange = { restorePassphrase = it },
                )
                PaperVerb(
                    isPrimary = true,
                    label = tr("Put it all back", "全部放回来"),
                    enabled = restorePassphrase.isNotBlank() && !isWorking,
                ) {
                    val text = pickedText ?: return@PaperVerb
                    scope.launch {
                        isWorking = true
                        intakeRefusal = null
                        DataKeeper.restore(store, restorePassphrase, text).fold(
                            onSuccess = { report ->
                                intakeSaid = DataChapterCopy.restored(report, tr)
                                forget()
                                onLibraryChanged()
                            },
                            onFailure = { intakeRefusal = DataRefusal.of(it) },
                        )
                        isWorking = false
                    }
                }
            }

            is PickedFile.Known -> PaperVerb(
                isPrimary = true,
                label = tr("Bring these in", "把它们带进来"),
                enabled = !isWorking,
            ) { bringIn(current.format) }

            PickedFile.AskWhichJson -> {
                Text(
                    // Asked, not guessed: three different products write
                    // `.json`, and importing one as another turns a library
                    // into gibberish.
                    text = tr(
                        "Three things write a .json file. Which one made this?",
                        "有三种东西都写 .json。这个是哪一种?",
                    ),
                    style = TypviaType.BodyS.style(tr.language),
                    color = Paper.ink2,
                    modifier = Modifier.padding(top = DataMetrics.innerGap),
                )
                Row(modifier = Modifier.padding(top = DataMetrics.innerGap)) {
                    for (kind in PickedFile.JSON_KINDS) {
                        Text(
                            text = when (kind) {
                                "masscode" -> "massCode"
                                "copyq" -> "CopyQ"
                                else -> tr("Typvia", "Typvia")
                            },
                            style = TypviaType.BodyS.style(tr.language),
                            color = if (jsonKind == kind) Paper.ink else Paper.ink3,
                            modifier = Modifier
                                .padding(end = DataMetrics.gap)
                                .clickable { jsonKind = kind },
                        )
                    }
                }
                PaperVerb(
                    isPrimary = true,
                    label = tr("Bring these in", "把它们带进来"),
                    enabled = jsonKind != null && !isWorking,
                ) { jsonKind?.let(::bringIn) }
            }
        }
        PaperVerb(
            label = if (picked == null) {
                tr("Choose a file", "选一个文件")
            } else {
                tr("Choose a different file", "换一个文件")
            },
            enabled = !isWorking,
        ) { pickFile.launch(arrayOf("*/*")) }
        Report(said = intakeSaid, refusal = intakeRefusal)

        Label(tr("THE BIN", "回收站"))
        PaperVerb(label = tr("What is in the bin", "回收站里有什么"), onTap = onOpenBin)

        Box(modifier = Modifier.height(Tokens.Space.section))
    }
}

/** What happened, said where the thing that happened is. */
@Composable
private fun Report(said: String?, refusal: DataRefusal?) {
    val tr = LocalTranslator.current
    said?.let { words ->
        Text(
            text = words,
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.ink,
            modifier = Modifier.padding(top = DataMetrics.gap),
        )
    }
    refusal?.let { why ->
        Text(
            text = DataChapterCopy.sentence(why, tr),
            style = TypviaType.BodyS.style(tr.language),
            color = Paper.attention,
            modifier = Modifier.padding(top = DataMetrics.gap),
        )
    }
}

@Composable
private fun Label(text: String) {
    val tr = LocalTranslator.current
    Text(
        text = text,
        style = TypviaType.MonoLabel.style(tr.language),
        color = Paper.ink3,
        modifier = Modifier.padding(top = Tokens.Space.group),
    )
}

/** What came of trying to read the file the reader picked. */
private sealed interface FileRead {
    data class Ok(val name: String, val text: String) : FileRead

    data object TooBig : FileRead

    data object Unreadable : FileRead
}

/**
 * Reads a picked file, bounded by **the core's own ceilings** rather than by a
 * number invented here: a backup may be large, an import file may not, and a
 * host that reads more than the core would take pulls a file into memory only
 * to be told it was never acceptable.
 */
private fun read(context: Context, uri: Uri): FileRead {
    val name = displayName(context, uri) ?: return FileRead.Unreadable
    val head = runCatching {
        context.contentResolver.openInputStream(uri)?.use { stream ->
            String(stream.readAtMost(HEAD_BYTES.toLong()), Charsets.UTF_8)
        }
    }.getOrNull() ?: return FileRead.Unreadable
    val ceiling = when (PickedFile.of(name, head)) {
        PickedFile.Backup -> backupMaxBytes().toLong()
        PickedFile.Unreadable -> return FileRead.Unreadable
        else -> importMaxBytes().toLong()
    }
    val bytes = runCatching {
        context.contentResolver.openInputStream(uri)?.use { stream ->
            // One byte past the ceiling, so "exactly at the limit" is allowed
            // and "over it" is known rather than silently truncated — a
            // truncated import file is one that imports half a library.
            stream.readAtMost(ceiling + 1)
        }
    }.getOrNull() ?: return FileRead.Unreadable
    if (bytes.size.toLong() > ceiling) return FileRead.TooBig
    return FileRead.Ok(name, String(bytes, Charsets.UTF_8))
}

/** Reads at most [limit] bytes. Written out rather than taken from the JDK
 * because the shortest way there needs a newer Android than this app asks for,
 * and a file read is not worth raising the floor of the whole product. */
private fun InputStream.readAtMost(limit: Long): ByteArray {
    val out = ByteArrayOutputStream()
    val chunk = ByteArray(READ_CHUNK)
    var taken = 0L
    while (taken < limit) {
        val want = minOf(chunk.size.toLong(), limit - taken).toInt()
        val got = read(chunk, 0, want)
        if (got <= 0) break
        out.write(chunk, 0, got)
        taken += got
    }
    return out.toByteArray()
}

private fun displayName(context: Context, uri: Uri): String? = runCatching {
    context.contentResolver
        .query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
}.getOrNull()

private fun write(context: Context, uri: Uri, text: String): Boolean = runCatching {
    context.contentResolver.openOutputStream(uri)?.use { stream ->
        stream.write(text.toByteArray(Charsets.UTF_8))
    } != null
}.getOrDefault(false)

/** Enough of the file to carry a sealed backup's outer marker, which is
 * written in the clear ahead of the ciphertext. */
private const val HEAD_BYTES = 4096

private const val HEAD_CHARS = 4096

/** How much is pulled off the stream at a time. */
private const val READ_CHUNK = 64 * 1024

private object DataMetrics {
    val gap = 18.dp
    val innerGap = 8.dp
}
