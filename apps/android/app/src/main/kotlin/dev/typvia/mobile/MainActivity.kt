// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

package dev.typvia.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import android.app.Activity
import androidx.compose.runtime.LaunchedEffect as Effect
import androidx.compose.ui.platform.LocalContext
import androidx.core.os.ConfigurationCompat
import androidx.core.view.WindowCompat
import dev.typvia.mobile.ffi.TypviaStore
import dev.typvia.mobile.ui.LocalRoom
import dev.typvia.mobile.ui.Paper
import dev.typvia.mobile.ui.VaultRoom
import androidx.compose.ui.graphics.toArgb
import androidx.compose.animation.Crossfade
import dev.typvia.mobile.ui.Beat
import dev.typvia.mobile.ui.LocalReduceMotion
import dev.typvia.mobile.ui.Room
import dev.typvia.mobile.ui.systemReduceMotion
import dev.typvia.mobile.ui.RoomBar
import dev.typvia.mobile.ui.LocalTranslator
import dev.typvia.mobile.ui.Translator
import dev.typvia.mobile.ui.TypeSort
import dev.typvia.mobile.ui.TypviaTheme
import dev.typvia.mobile.ui.isDarkPaper
import androidx.compose.runtime.CompositionLocalProvider
import java.io.File

/**
 * The window's root.
 *
 * It opens the database once and hands it to whichever room is open. Rooms
 * arrive one stage at a time; the bar only offers doors to the ones that have
 * been built, so a word in it is never a dead end.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val store = TypviaStore.open(File(filesDir, "typvia"))
        val dataDir = dataDir
        val tags = ConfigurationCompat.getLocales(resources.configuration).let { locales ->
            (0 until locales.size()).mapNotNull { locales.get(it)?.toLanguageTag() }
        }
        // Read here rather than inside the composition: the first frame has to
        // be the one the reader chose. A window that opens in the system's
        // appearance and turns over a frame later is the app telling them their
        // setting is something it applies afterwards.
        val preferences = UiPreferences.open(this)
        setContent {
            TypviaTheme(
                translator = Translator(preferences.language.resolved(tags)),
                appearance = preferences.appearance,
            ) {
                Shell(store = store, dataDir = dataDir, preferences = preferences)
            }
        }
    }
}

/** The rooms that have screens here. The others are not printed. */
private val builtRooms = setOf(Room.Home, Room.Library, Room.Vault, Room.Settings)

@Composable
private fun Shell(store: TypviaStore, dataDir: File, preferences: UiPreferences) {
    var room by remember { mutableStateOf(Room.Home) }
    var shelf by remember { mutableStateOf<HomeShelf?>(null) }
    var chapters by remember { mutableStateOf<List<Chapter>?>(null) }
    var vault by remember { mutableStateOf(VaultPhase.Absent) }
    // Nil is "not read"; an empty list is a vault with nothing in it. They
    // look the same on screen only if the screen is careless.
    var vaultEntries by remember { mutableStateOf<List<VaultEntry>?>(null) }
    var vaultRefusal by remember { mutableStateOf<VaultRefusal?>(null) }
    /**
     * A secret typed out while the vault was shut or absent. It is held here,
     * not thrown away: changing your mind about the vault is not changing your
     * mind about the words.
     */
    var pendingSecret by remember { mutableStateOf<PendingSecret?>(null) }
    /** Which settings chapter is open as its own page, if any. */
    var openChapterPage by remember { mutableStateOf<SettingsChapter?>(null) }
    /** Whether this device is in the middle of joining an account. */
    var isJoining by remember { mutableStateOf(false) }
    var vaultStatus by remember { mutableStateOf<uniffi.typvia_mobile_ffi.VaultStatus?>(null) }
    var syncStatus by remember { mutableStateOf<uniffi.typvia_mobile_ffi.SyncStatus?>(null) }
    var syncDevices by remember { mutableStateOf<List<uniffi.typvia_mobile_ffi.SyncDevice>?>(null) }
    /** What the last round did, in words. Null until one has been run here. */
    var lastRound by remember { mutableStateOf<String?>(null) }
    var syncWorking by remember { mutableStateOf(false) }
    /**
     * Words currently out of the vault, and nothing else about them. Held for
     * as long as they are on screen and dropped by leaving the page — this app
     * keeps no second copy of a secret anywhere.
     */
    var takenOut by remember { mutableStateOf<String?>(null) }
    var summary by remember { mutableStateOf<SettingsSummary?>(null) }
    var isWriting by remember { mutableStateOf(false) }
    // Editing is the same page as writing, opened on something that exists.
    var editing by remember { mutableStateOf<uniffi.typvia_mobile_ffi.Snippet?>(null) }
    var folders by remember { mutableStateOf(emptyList<uniffi.typvia_mobile_ffi.Folder>()) }
    var query by remember { mutableStateOf("") }
    var results by remember { mutableStateOf<List<RecallTile>?>(null) }
    var open by remember { mutableStateOf<uniffi.typvia_mobile_ffi.Snippet?>(null) }
    var openChapter by remember { mutableStateOf<TypeSort?>(null) }
    /** What may be asked of a model about the snippet that is open, and where it got to. */
    var aiActions by remember { mutableStateOf<List<AiActionRow>>(emptyList()) }
    var aiPhase by remember { mutableStateOf<AiStripPhase>(AiStripPhase.Idle) }
    /** The engines this device has, as the chapter reads them. Null until read. */
    var aiProviders by remember {
        mutableStateOf<List<uniffi.typvia_mobile_ffi.AiProviderRow>?>(null)
    }
    var aiRefusal by remember { mutableStateOf<AiRefusal?>(null) }
    var aiWorking by remember { mutableStateOf(false) }
    var isInTheBin by remember { mutableStateOf(false) }
    var binRows by remember { mutableStateOf<List<uniffi.typvia_mobile_ffi.Snippet>?>(null) }
    var chapterRows by remember { mutableStateOf<List<uniffi.typvia_mobile_ffi.Snippet>?>(null) }
    var writeRefusal by remember { mutableStateOf<String?>(null) }
    /// The library could not be read. Not the same state as "still reading",
    /// and the screen must not sit on the second one forever while meaning
    /// the first.
    var unreadable by remember { mutableStateOf(false) }

    // Reading again on the way back in, not only on the way in. Sync can land
    // rows while this screen is behind something else, and a shelf that only
    // ever reads once shows a library that stopped changing the moment the
    // reader first looked at it.
    var round by remember { mutableStateOf(0) }
    // Read once per composition from the system's own list. A keyboard the
    // reader has not switched on is a fact about their phone, not about this
    // app, so it is read where the phone keeps it.
    val context = LocalContext.current
    // Read again on the way back in, not only on the way in: the reader
    // leaves this app to switch the keyboard on, and a row that answered once
    // at launch would still say "not on yet" when they returned from doing it.
    val keyboardIsOn = remember(context, round) {
        // Asked through the input-method service rather than by reading the
        // secure setting: that string is not an app's to read on current
        // Android, and a read that always fails would leave this row blank
        // forever while looking like it had asked.
        runCatching {
            context.getSystemService(android.view.inputmethod.InputMethodManager::class.java)
                ?.enabledInputMethodList
                ?.any { it.packageName == context.packageName }
        }.getOrNull()
    }

    val lifecycleOwner = LocalLifecycleOwner.current
    val scope = rememberCoroutineScope()

    // What every write does afterwards: the screens read again, and the
    // document the keyboard reads is written again.
    //
    // These used to be two separate lines repeated at each call site, and the
    // newest call site — saving a *new* snippet, the one path every reader
    // takes first — had only the first of them. The result was a keyboard that
    // had never heard of the snippet the reader had just saved, until they
    // happened to leave the app and come back. One call, so the pair cannot
    // drift apart again.
    suspend fun libraryChanged() {
        round += 1
        SnapshotPublish.run(store, dataDir)
    }
    DisposableEffect(lifecycleOwner, store) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                scope.launch {
                    // Anything the share sheet left is filed on the way in,
                    // before the shelf is read — otherwise what just arrived
                    // is missing from the screen that is about to be drawn.
                    // Every resume, not only launch: a reader can share while
                    // this app is still in memory behind them.
                    InboxIntake.run(store, dataDir)
                    libraryChanged()
                }
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // Every keystroke asks again. Which snippets match is the core's answer —
    // ranking, trigger weighting, what a sensitive row may show — and this only
    // carries the question across.
    // Read when a snippet is opened rather than once at launch: the set of
    // actions changes with the library, and a page offering an action that was
    // deleted is a page whose only verb refuses.
    LaunchedEffect(store, open?.id) {
        aiPhase = AiStripPhase.Idle
        aiActions = if (open == null) emptyList() else AiKeeper.actions(store).orEmpty()
    }

    LaunchedEffect(store, query) {
        val trimmed = query.trim()
        if (trimmed.isEmpty()) {
            results = null
            return@LaunchedEffect
        }
        results = runCatching {
            store.perform { core -> core.searchAll(trimmed, 20u).map(RecallTile::of) }
        }.getOrNull()
    }

    LaunchedEffect(store, round) {
        // One pass, inside one call that already holds the database. The
        // ordering rules are the core's: `recent` is recall order, `used` is
        // the one that answers "which do I type most".
        val read = runCatching {
            store.perform { core ->
                HomeShelf.assemble(
                    total = core.snippetCount("all", null, null),
                    recent = core.snippetListPage("recent", null, null, HomeShelf.RECENT_LIMIT, 0u),
                    mostUsed = core.snippetListPage(
                        "used", null, null, HomeShelf.TRIGGER_PAGE_LIMIT, 0u,
                    ),
                )
            }
        }
        read.exceptionOrNull()?.let { trouble ->
            // A cancelled read is not a failed one. This effect restarts on
            // every resume, and the old one is cancelled as it does — calling
            // that "the library could not be read" would put a failure on
            // screen every time the reader came back to the app.
            if (trouble is CancellationException) throw trouble
            // The kind of failure, never its words: an engine message is
            // written for whoever reads a log, and this product renders one
            // language at a time.
            android.util.Log.w("Typvia", "the shelf could not be read: ${trouble.javaClass.name}")
            unreadable = true
        }
        shelf = read.getOrNull()
        val roomsRead = runCatching {
            store.perform { core ->
                val status = core.vaultStatus()
                val phase = VaultPhase.of(status)
                // Sync is allowed to be absent — a device with no gated key
                // storage has no account and no devices — and that must not
                // take the library's chapters down with it. One call failing
                // is not the library failing.
                val sync = runCatching { core.syncStatus() }.getOrNull()
                val devices = runCatching { core.syncDevices() }.getOrNull()
                Triple(
                    Chapter.assemble(
                        counts = TypeSort.entries.associateWith { sort ->
                            core.snippetCount("all", null, sort.coreType)
                        },
                        vaultOpen = status.unlocked,
                    ),
                    phase,
                    SettingsSummary(
                        total = core.snippetCount("all", null, null),
                        isSyncConfigured = sync?.configured ?: false,
                        deviceCount = devices?.size ?: 0,
                        vaultPhase = phase,
                        // Asked of the system rather than assumed: whether
                        // this product's keyboard is switched on is the
                        // system's answer, and "could not ask" is left blank.
                        isKeyboardOn = keyboardIsOn,
                        aiEngines = runCatching { core.aiProviderList().size }.getOrNull(),
                    ),
                )
            }
        }
        roomsRead.exceptionOrNull()?.let { trouble ->
            if (trouble is CancellationException) throw trouble
            android.util.Log.w("Typvia", "the rooms could not be read: ${trouble.javaClass.name}")
            // The same three states the shelf has. A room that cannot be read
            // must not sit on "reading" — that is the most patient kind of
            // lie, and it looks exactly like a slow device.
            unreadable = true
        }
        val rooms = roomsRead.getOrNull()
        folders = runCatching {
            store.perform { it.folderListChildren(null) }
        }.getOrDefault(folders)
        chapters = rooms?.first
        // Absent is a claim about the device, not a way to say "did not read".
        // It only stands when the core actually answered.
        vault = rooms?.second ?: vault
        // Kept because the vault's own chapter states the window it re-locks
        // after, and that number is the core's, not a constant on this side.
        vaultStatus = runCatching { store.perform { it.vaultStatus() } }.getOrNull() ?: vaultStatus
        syncStatus = SyncKeeper.status(store) ?: syncStatus
        syncDevices = SyncKeeper.devices(store) ?: syncDevices
        aiProviders = AiKeeperSettings.providers(store) ?: aiProviders
        summary = rooms?.third
        // Names and timestamps only, and only while it is open. The bridge
        // returns no bodies for these rows and this asks for none.
        if (vault == VaultPhase.Open) {
            vaultEntries = runCatching {
                store.perform { core ->
                    VaultOrder.sort(
                        core.vaultList(100u, 0u).map {
                            VaultEntry(it.id, it.title, it.lastUsedAt)
                        },
                    )
                }
            }.getOrNull()
        }
    }

    // The system's own bars take the room's paper. The delivery is explicit
    // that the page is one whole sheet rather than something drawn under the
    // navigation bar — which means that when the sheet turns to ink, the strip
    // the system draws has to turn with it, or there is a seam along the
    // bottom of the vault.
    val activity = LocalContext.current as? Activity
    // The reader's choice, not the phone's: the bars belong to the same sheet
    // as the page, and a light strip along a dark page is a seam.
    val darkPaper = isDarkPaper
    val onInk = room == Room.Vault
    val paper = if (onInk) VaultRoom.base else Paper.base
    Effect(activity, paper) {
        val window = activity?.window ?: return@Effect
        @Suppress("DEPRECATION")
        window.navigationBarColor = paper.toArgb()
        @Suppress("DEPRECATION")
        window.statusBarColor = paper.toArgb()
        // Dark paper needs light icons and the other way round; the system
        // does not work this out from the colour it was handed.
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightNavigationBars = !onInk && !darkPaper
            isAppearanceLightStatusBars = !onInk && !darkPaper
        }
    }

    val tr = LocalTranslator.current

    CompositionLocalProvider(LocalRoom provides room) {
        if (isJoining) {
            // The system's back gesture is the way out of a page on this
            // platform, and every page here had only the word on the screen.
            // Each layer below gives the two doors **one** leaving — a second
            // copy of "what closing does" is a copy that drifts.
            val leave = {
                // Leaving drops the session on this device. A code left
                // running on a screen nobody is looking at is a code somebody
                // else could still be answering.
                scope.launch { PairingKeeper.cancel(store) }
                isJoining = false
            }
            BackHandler(onBack = leave)
            PairingScreen(
                store = store,
                onLeave = leave,
                onJoined = {
                    scope.launch {
                        syncStatus = SyncKeeper.status(store) ?: syncStatus
                        syncDevices = SyncKeeper.devices(store) ?: syncDevices
                        // Joining brings nothing across by itself, but the
                        // rooms now count a device and an account they did not
                        // a moment ago.
                        libraryChanged()
                    }
                },
            )
            return@CompositionLocalProvider
        }
        // Above the chapters, not below them. The recycle bin is opened
        // from inside the data chapter and that chapter stays open behind
        // it — so with the bin underneath, opening it set a state nothing
        // ever drew, and the verb in the data chapter did nothing at all.
        if (isInTheBin) {
            val closeTheBin = {
                isInTheBin = false
                binRows = null
            }
            BackHandler(onBack = closeTheBin)
            TrashScreen(
                rows = binRows,
                onRestore = { id ->
                    scope.launch {
                        val restored = runCatching {
                            store.perform { it.snippetRestore(id) }
                        }
                        if (restored.isSuccess) {
                            binRows = binRows?.filterNot { it.id == id }
                            libraryChanged()
                        }
                    }
                },
                onEmpty = {
                    scope.launch {
                        val ids = binRows?.map { it.id } ?: emptyList()
                        val destroyed = SnippetWriter.emptyTheBin(store, ids)
                        // Re-read rather than assume: if some could not be
                        // destroyed, what is left is what the screen must show.
                        binRows = runCatching {
                            store.perform { it.trashList(100u, 0u) }
                        }.getOrNull()
                        // Only the screens are re-read. What the bin holds
                        // was already out of the keyboard's document when it
                        // was thrown away, so destroying it changes nothing
                        // there — and writing an identical document is work
                        // done to look consistent.
                        if (destroyed > 0) round += 1
                    }
                },
                onClose = closeTheBin,
            )
            return@CompositionLocalProvider
        }
        openChapterPage?.let { chapter ->
            // One handler for all six chapters: every one of them closes by
            // going back to the contents page, and stating that once is what
            // keeps a chapter added later from being the one without it.
            BackHandler { openChapterPage = null }
            if (chapter == SettingsChapter.Sync) {
                SyncChapterScreen(
                    status = syncStatus,
                    devices = syncDevices,
                    lastRound = lastRound,
                    isWorking = syncWorking,
                    onClose = { openChapterPage = null },
                    onSyncNow = {
                        scope.launch {
                            syncWorking = true
                            SyncKeeper.runRound(store).fold(
                                onSuccess = { round ->
                                    lastRound = SyncChapterCopy.round(round, tr)
                                    syncStatus = SyncKeeper.status(store)
                                    syncDevices = SyncKeeper.devices(store)
                                    // A round can bring rows in. The screens
                                    // and the keyboard's document both have to
                                    // hear about that.
                                    libraryChanged()
                                },
                                onFailure = {
                                    // The kind of failure, never its words: an
                                    // engine message is written for a log.
                                    lastRound = tr(
                                        "That round did not go through. Nothing was lost.",
                                        "这一轮没走通。什么都没丢。",
                                    )
                                },
                            )
                            syncWorking = false
                        }
                    },
                    onJoin = { isJoining = true },
                    onSwitch = { on ->
                        scope.launch {
                            syncWorking = true
                            val changed = if (on) {
                                SyncKeeper.switchOn(store)
                            } else {
                                SyncKeeper.switchOff(store)
                            }
                            changed.onSuccess { syncStatus = it }
                            syncWorking = false
                        }
                    },
                )
                return@CompositionLocalProvider
            }
            if (chapter == SettingsChapter.Data) {
                DataChapterScreen(
                    store = store,
                    onClose = { openChapterPage = null },
                    onOpenBin = {
                        isInTheBin = true
                        scope.launch {
                            binRows = runCatching {
                                store.perform { it.trashList(100u, 0u) }
                            }.getOrNull()
                        }
                    },
                    // An import or a restore can put a whole library on this
                    // device. Everything that reads the library — the screens
                    // and the document the keyboard reads — has to hear that.
                    onLibraryChanged = { scope.launch { libraryChanged() } },
                )
                return@CompositionLocalProvider
            }
            if (chapter == SettingsChapter.Ai) {
                AiChapterScreen(
                    engine = AiEngine.inEffect(aiProviders),
                    refusal = aiRefusal,
                    isWorking = aiWorking,
                    onClose = { openChapterPage = null },
                    onChoose = { engine, model, baseUrl, apiKey ->
                        scope.launch {
                            aiWorking = true
                            AiKeeperSettings.choose(store, engine, model, baseUrl, apiKey).fold(
                                onSuccess = {
                                    aiRefusal = null
                                    // Re-read rather than assume: what is in
                                    // effect is the set of providers, and a
                                    // page that assumed its own write landed
                                    // would report a setting nobody made.
                                    aiProviders = AiKeeperSettings.providers(store)
                                    // The contents page counts engines.
                                    round += 1
                                },
                                onFailure = { aiRefusal = AiRefusal.of(it) },
                            )
                            aiWorking = false
                        }
                    },
                )
                return@CompositionLocalProvider
            }
            if (chapter == SettingsChapter.Appearance) {
                AppearanceChapterScreen(
                    preferences = preferences,
                    onClose = { openChapterPage = null },
                )
                return@CompositionLocalProvider
            }
            if (chapter == SettingsChapter.Keyboard) {
                KeyboardChapterScreen(
                    isOn = keyboardIsOn,
                    onClose = { openChapterPage = null },
                    // Switching a keyboard on is the system's own screen: no
                    // app may do it, and this is the shortest way there.
                    onOpenSystemList = {
                        runCatching {
                            context.startActivity(
                                android.content.Intent(
                                    android.provider.Settings.ACTION_INPUT_METHOD_SETTINGS,
                                ).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK),
                            )
                        }
                    },
                    onSwitchToIt = {
                        runCatching {
                            context.getSystemService(
                                android.view.inputmethod.InputMethodManager::class.java,
                            )?.showInputMethodPicker()
                        }
                    },
                )
                return@CompositionLocalProvider
            }
            if (chapter == SettingsChapter.Vault) {
                VaultChapterScreen(
                    phase = vault,
                    status = vaultStatus,
                    onClose = { openChapterPage = null },
                    onShut = if (vault == VaultPhase.Open) {
                        {
                            scope.launch {
                                VaultKeeper.shut(store).onSuccess {
                                    vault = VaultPhase.of(it)
                                    vaultEntries = null
                                    vaultStatus = it
                                    round += 1
                                }
                            }
                        }
                    } else {
                        null
                    },
                )
                return@CompositionLocalProvider
            }
        }
        openChapter?.let { sort ->
            if (open == null) {
                val closeChapter = {
                    openChapter = null
                    chapterRows = null
                }
                BackHandler(onBack = closeChapter)
                ChapterScreen(
                    sort = sort,
                    rows = chapterRows,
                    folders = folders,
                    onOpen = { id ->
                        scope.launch {
                            open = runCatching { store.perform { it.snippetGet(id) } }.getOrNull()
                        }
                    },
                    onNeedMore = {
                        scope.launch {
                            val already = chapterRows ?: return@launch
                            val more = runCatching {
                                store.perform {
                                    it.snippetListPage(
                                        "all", null, sort.coreType,
                                        ChapterPaging.PAGE, already.size.toUInt(),
                                    )
                                }
                            }.getOrNull().orEmpty()
                            // Nothing new means the end; appending an empty
                            // page would keep asking for it forever.
                            if (more.isNotEmpty()) chapterRows = already + more
                        }
                    },
                    onCompose = { isWriting = true },
                    onClose = closeChapter,
                )
                return@CompositionLocalProvider
            }
        }
        open?.let { snippet ->
            val closeSnippet = {
                open = null
                // Leaving the page puts the words away, whatever the timer was
                // going to do.
                takenOut = null
            }
            BackHandler(onBack = closeSnippet)
            DetailScreen(
                snippet = snippet,
                onCopy = { body ->
                    val clipboard = activity?.getSystemService(android.content.ClipboardManager::class.java)
                    clipboard?.setPrimaryClip(
                        android.content.ClipData.newPlainText("Typvia", body),
                    )
                    // Counted after the words have gone somewhere, never
                    // before: the shelf of "used recently" is built from this,
                    // and a count that includes what was never delivered is a
                    // shelf that recommends things nobody used.
                    scope.launch {
                        SnippetWriter.recordUse(store, snippet.id)
                        libraryChanged()
                    }
                },
                onEdit = { editing = snippet },
                onTakeOut = if (snippet.securityLevel != "normal") {
                    {
                        scope.launch {
                            VaultKeeper.takeOut(store, snippet.id).onSuccess { words ->
                                takenOut = words
                                // Taking a secret out is using it — the vault
                                // list is ordered by when each was last taken
                                // out, which is only true if it is written
                                // down.
                                SnippetWriter.recordUse(store, snippet.id)
                                // Out for as long as it takes to read and use,
                                // then covered again without being asked. A
                                // phone left on a table stops showing a key.
                                delay(VaultKeeper.TAKEN_OUT_SECONDS * 1000)
                                takenOut = null
                            }
                            // A failure needs no words here: the vault says the
                            // same thing by staying covered.
                        }
                    }
                } else {
                    null
                },
                takenOut = takenOut,
                onPutInVault = if (snippet.securityLevel == "normal") {
                    {
                        scope.launch {
                            VaultKeeper.putIn(store, snippet.id).onSuccess {
                                open = runCatching {
                                    store.perform { it.snippetGet(snippet.id) }
                                }.getOrNull()
                                libraryChanged()
                            }
                        }
                    }
                } else {
                    null
                },
                onTrash = {
                    scope.launch {
                        if (SnippetWriter.trash(store, snippet.id).isSuccess) {
                            open = null
                            // The shelf and the chapter both counted it; both
                            // have to be told, and the keyboard reads a
                            // document that counted it too.
                            chapterRows = chapterRows?.filterNot { it.id == snippet.id }
                            libraryChanged()
                        }
                    }
                },
                onClose = closeSnippet,
                // Offered only where the words are this page's to hand over: a
                // secret's page has already returned before the strip would be
                // composed, and passing nothing here says the same thing twice
                // on purpose.
                aiStrip = if (snippet.securityLevel == "normal" && snippet.body != null) {
                    {
                        AiStrip(
                            phase = aiPhase,
                            actions = aiActions,
                            onRun = { action ->
                                scope.launch {
                                    val body = snippet.body ?: return@launch
                                    if (!AiKeeper.hasEngine(store)) {
                                        aiPhase = AiStripPhase.NoEngine
                                        return@launch
                                    }
                                    aiPhase = AiStripPhase.Working
                                    AiKeeper.run(
                                        store,
                                        action,
                                        body,
                                        // The snippet's own state, passed
                                        // through. This side does not vouch.
                                        isSensitive = snippet.securityLevel != "normal",
                                    ).fold(
                                        onSuccess = { aiPhase = it },
                                        onFailure = {
                                            aiPhase = AiStripPhase.Refused(AiRefusal.of(it))
                                        },
                                    )
                                }
                            },
                            onTake = { text ->
                                scope.launch {
                                    val saved = SnippetWriter.update(
                                        store,
                                        snippet,
                                        snippet.title,
                                        text,
                                        TypeSort.ofCoreType(snippet.snippetType) ?: TypeSort.Text,
                                        snippet.trigger ?: "",
                                        snippet.folderId,
                                    )
                                    if (saved.isSuccess) {
                                        aiPhase = AiStripPhase.Idle
                                        open = saved.getOrNull()
                                        libraryChanged()
                                    } else {
                                        // The words came back and could not be
                                        // written down. Said where the reader
                                        // is looking, and **the output stays
                                        // on screen** — throwing it away to
                                        // report that it could not be saved
                                        // would make one failure into two.
                                        aiPhase = (aiPhase as? AiStripPhase.Produced)
                                            ?.copy(refusal = AiRefusal.Storage)
                                            ?: AiStripPhase.Refused(AiRefusal.Storage)
                                    }
                                }
                            },
                            onDismiss = { aiPhase = AiStripPhase.Idle },
                        )
                    }
                } else {
                    null
                },
            )
            return@CompositionLocalProvider
        }
        editing?.let { subject ->
            val stopEditing = {
                writeRefusal = null
                editing = null
            }
            BackHandler(onBack = stopEditing)
            ComposeScreen(
                isNew = false,
                refusal = writeRefusal,
                initialTitle = subject.title,
                initialBody = subject.body ?: "",
                initialSort = TypeSort.ofCoreType(subject.snippetType) ?: TypeSort.Text,
                initialTrigger = subject.trigger ?: "",
                folders = folders,
                initialFolderId = subject.folderId,
                onMakeFolder = { name ->
                    scope.launch {
                        SnippetWriter.makeFolder(store, name)
                        folders = runCatching {
                            store.perform { it.folderListChildren(null) }
                        }.getOrDefault(folders)
                    }
                },
                onSave = { title, body, sort, trigger, folderId ->
                    scope.launch {
                        val saved = SnippetWriter.update(
                            store, subject, title, body, sort, trigger, folderId,
                        )
                        if (saved.isSuccess) {
                            writeRefusal = null
                            editing = null
                            // The page behind it is showing the old words.
                            open = saved.getOrNull()
                            libraryChanged()
                        } else {
                            writeRefusal = tr(
                                "It could not be written just now. Nothing changed.",
                                "刚才没写进去。什么都没有改动。",
                            )
                        }
                    }
                },
                onCancel = stopEditing,
            )
            return@CompositionLocalProvider
        }
        if (isWriting) {
            val stopWriting = {
                writeRefusal = null
                isWriting = false
            }
            BackHandler(onBack = stopWriting)
            ComposeScreen(
                refusal = writeRefusal,
                folders = folders,
                onMakeFolder = { name ->
                    scope.launch {
                        SnippetWriter.makeFolder(store, name)
                        folders = runCatching {
                            store.perform { it.folderListChildren(null) }
                        }.getOrDefault(folders)
                    }
                },
                gate = pendingSecret?.let { held ->
                    {
                        SecretGate(
                            phase = vault,
                            refusal = vaultRefusal,
                            shortestPassword = VaultKeeper.shortestPassword(),
                            onMake = { password, again ->
                                scope.launch {
                                    if (!VaultKeeper.typedTwiceMatches(password, again)) {
                                        vaultRefusal = VaultRefusal.TypedDifferently
                                        return@launch
                                    }
                                    VaultKeeper.make(store, password).fold(
                                        onSuccess = {
                                            vault = VaultPhase.of(it)
                                            finishSecret(store, held) { done ->
                                                vaultRefusal = null
                                                pendingSecret = null
                                                isWriting = !done
                                                if (done) {
                                                    writeRefusal = null
                                                    libraryChanged()
                                                } else {
                                                    // The door opened and the
                                                    // write still failed. The
                                                    // gate is gone by now, so
                                                    // the page has to be the
                                                    // one that says it —
                                                    // otherwise the gate just
                                                    // vanishes and nothing
                                                    // happens.
                                                    writeRefusal = tr(
                                                        "It could not be written just now. Nothing changed.",
                                                        "刚才没写进去。什么都没有改动。",
                                                    )
                                                }
                                            }
                                        },
                                        onFailure = { vaultRefusal = VaultRefusal.of(it) },
                                    )
                                }
                            },
                            onOpen = { password ->
                                scope.launch {
                                    VaultKeeper.open(store, password).fold(
                                        onSuccess = {
                                            vault = VaultPhase.of(it)
                                            finishSecret(store, held) { done ->
                                                vaultRefusal = null
                                                pendingSecret = null
                                                isWriting = !done
                                                if (done) {
                                                    writeRefusal = null
                                                    libraryChanged()
                                                } else {
                                                    // The door opened and the
                                                    // write still failed. The
                                                    // gate is gone by now, so
                                                    // the page has to be the
                                                    // one that says it —
                                                    // otherwise the gate just
                                                    // vanishes and nothing
                                                    // happens.
                                                    writeRefusal = tr(
                                                        "It could not be written just now. Nothing changed.",
                                                        "刚才没写进去。什么都没有改动。",
                                                    )
                                                }
                                            }
                                        },
                                        onFailure = { vaultRefusal = VaultRefusal.of(it) },
                                    )
                                }
                            },
                        )
                    }
                },
                onSave = { title, body, sort, trigger, folderId ->
                    scope.launch {
                        // A secret never goes through the ordinary door. The
                        // core refuses it there anyway; routing it here means
                        // the reader is asked for the vault instead of being
                        // handed a refusal they cannot act on.
                        if (sort == TypeSort.Secret) {
                            val held = PendingSecret(title, body, trigger, folderId)
                            if (vault != VaultPhase.Open) {
                                vaultRefusal = null
                                pendingSecret = held
                                return@launch
                            }
                            finishSecret(store, held) { done ->
                                if (done) {
                                    writeRefusal = null
                                    isWriting = false
                                    libraryChanged()
                                } else {
                                    writeRefusal = tr(
                                        "It could not be written just now. Nothing changed.",
                                        "刚才没写进去。什么都没有改动。",
                                    )
                                }
                            }
                            return@launch
                        }
                        val saved = SnippetWriter.write(store, title, body, sort, trigger, folderId)
                        if (saved.isSuccess) {
                            writeRefusal = null
                            isWriting = false
                            libraryChanged()
                        } else {
                            // Said on the screen that caused it, and the words
                            // stay where they are: a refusal is not a reason to
                            // throw away what somebody just wrote.
                            writeRefusal = tr(
                                "It could not be written just now. Nothing changed.",
                                "刚才没写进去。什么都没有改动。",
                            )
                        }
                    }
                },
                onCancel = stopWriting,
            )
            return@CompositionLocalProvider
        }

        val reduceMotion = LocalReduceMotion.current
        Column(modifier = Modifier.fillMaxSize().background(paper)) {
            // A room arrives by cross-fade rather than by cutting: 240ms, the
            // delivery's page-turn beat, collapsing to one short fade when the
            // reader has asked the system for less motion.
            Crossfade(
                targetState = room,
                animationSpec = Beat.Transition.enter(reduceMotion),
                label = "room",
                modifier = Modifier.weight(1f),
            ) { current ->
            when (current) {
                Room.Library -> LibraryScreen(
                    chapters = chapters,
                    isUnreadable = unreadable,
                    onOpenChapter = { sort ->
                        openChapter = sort
                        scope.launch {
                            chapterRows = runCatching {
                                store.perform {
                                    it.snippetListPage(
                                        "all", null, sort.coreType, ChapterPaging.PAGE, 0u,
                                    )
                                }
                            }.getOrNull()
                        }
                    },
                )
                Room.Vault -> VaultScreen(
                    phase = vault,
                    entries = vaultEntries,
                    refusal = vaultRefusal,
                    shortestPassword = VaultKeeper.shortestPassword(),
                    onMake = { password, again ->
                        scope.launch {
                            // Two typings first, and nothing sent when they
                            // differ: the one thing standing between a reader
                            // and a typo they cannot see.
                            if (!VaultKeeper.typedTwiceMatches(password, again)) {
                                vaultRefusal = VaultRefusal.TypedDifferently
                                return@launch
                            }
                            VaultKeeper.make(store, password).fold(
                                onSuccess = {
                                    vaultRefusal = null
                                    vault = VaultPhase.of(it)
                                    vaultEntries = VaultKeeper.list(store)
                                    // The rest of the app counts by this too:
                                    // the library's secret chapter says "shut"
                                    // until it is read again, and a chapter
                                    // still saying that after the vault opened
                                    // is the app disagreeing with itself.
                                    round += 1
                                },
                                onFailure = { vaultRefusal = VaultRefusal.of(it) },
                            )
                        }
                    },
                    onOpen = { password ->
                        scope.launch {
                            VaultKeeper.open(store, password).fold(
                                onSuccess = {
                                    vaultRefusal = null
                                    vault = VaultPhase.of(it)
                                    vaultEntries = VaultKeeper.list(store)
                                    // The rest of the app counts by this too:
                                    // the library's secret chapter says "shut"
                                    // until it is read again, and a chapter
                                    // still saying that after the vault opened
                                    // is the app disagreeing with itself.
                                    round += 1
                                },
                                onFailure = { vaultRefusal = VaultRefusal.of(it) },
                            )
                        }
                    },
                    onShut = {
                        scope.launch {
                            VaultKeeper.shut(store).onSuccess {
                                vaultRefusal = null
                                vault = VaultPhase.of(it)
                                round += 1
                                // What was listed goes with the shutting. A
                                // list left on screen after the door closes is
                                // the room still talking about its contents.
                                vaultEntries = null
                            }
                        }
                    },
                )
                Room.Settings -> SettingsScreen(
                    onOpenChapter = { openChapterPage = it },
                    summary = summary,
                    language = preferences.language,
                    isUnreadable = unreadable,
                )
                else -> HomeScreen(
                    shelf = shelf,
                    isUnreadable = unreadable,
                    query = query,
                    onQueryChange = { typed ->
                        query = typed
                        results = null
                    },
                    results = results,
                    onCompose = { isWriting = true },
                    onOpen = { id ->
                        scope.launch {
                            open = runCatching {
                                store.perform { it.snippetGet(id) }
                            }.getOrNull()
                        }
                    },
                )
            }
            }
            RoomBar(
                current = room,
                available = builtRooms,
                // The vault's material breaks from the rest of the app, and
                // the bar standing on it breaks with it.
                onInkRoom = room == Room.Vault,
                select = { room = it },
            )
        }
    }
}
