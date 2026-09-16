// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import TypviaKit
import UIKit
import WidgetKit

/// The window's root.
///
/// It opens the database once and hands it to whichever room is open. Rooms
/// arrive one stage at a time; the bar only offers doors to the ones that have
/// been built, so a word in it is never a dead end.
struct RootView: View {
    @State private var store: TypviaStore?
    @State private var room: Room = .home
    @State private var open: OpenSnippet?
    /// Joining an account takes over the screen while it runs: it is a flow
    /// with a live session behind it, not a panel to glance at.
    @State private var isPairing = false
    @StateObject private var preferences = UiPreferences()
    /// Whether the reader is still being introduced. Nil until the marker
    /// beside the database has been read — the shell shows paper rather than
    /// guessing, because guessing wrong means either flashing the product
    /// before introducing it or introducing it to someone who has been using
    /// it for a month.
    @State private var isOnboarding: Bool?
    /// Links wait until that is known, and through the first run if there is
    /// one. Dropping a link loses what the reader tapped; following it strands
    /// them in a product they have not been introduced to.
    @StateObject private var links = DeepLinkRouter(isOnboarding: true)
    @Environment(\.scenePhase) private var scenePhase

    init() {
        _store = State(initialValue: RootView.sharedStore)
    }

    /// One store for the process, shared with the background handler. A second
    /// one over the same directory would put two writers on one database.
    ///
    /// It is `nonisolated` because the background handler is registered before
    /// the first view exists and runs off the main actor; the store itself is
    /// an actor, so what crosses here is a handle, not shared mutable state.
    nonisolated static let sharedStore: TypviaStore? =
        try? TypviaStore(dataDirectory: RootView.dataDirectory)

    var body: some View {
        if let store {
            shell(in: store)
                .uiPreferences(preferences)
                // Anything the share sheet left is filed on the way in. It
                // runs on every activation, not just launch: a reader can
                // share while the app is still in memory behind them.
                .task {
                    await readFirstRun(store)
                    await InboxIntake.run(store: store)
                    await publishSnapshot(store)
                    // Ask for a wake-up only where there is something to send.
                    let status = try? await store.perform { try $0.syncStatus() }
                    if BackgroundSync.shouldSchedule(
                        isConfigured: status?.configured ?? false,
                        isEnabled: status?.enabled ?? false
                    ) {
                        BackgroundSync.schedule()
                    }
                }
                .onOpenURL { links.open($0) }
                .onChange(of: links.pending) { link in
                    guard let link else { return }
                    follow(link)
                    links.clear()
                }
                .onChange(of: scenePhase) { phase in
                    guard phase == .active else { return }
                    Task {
                        await InboxIntake.run(store: store)
                        await publishSnapshot(store)
                    }
                }
                // One snippet at a time, over whichever room opened it. The
                // product has one modal and it is the vault's, so this is a
                // full screen rather than a sheet.
                .overlay { openSnippet(in: store) }
                .overlay {
                    if isPairing {
                        PairingScreen(store: store, close: { isPairing = false })
                            .transition(.opacity)
                    }
                }
        } else {
            UnavailableView()
        }
    }

    /// The introduction, the product, or neither yet.
    @ViewBuilder
    private func shell(in store: TypviaStore) -> some View {
        switch isOnboarding {
        case .none:
            // The marker has not been read yet. Blank paper, not a spinner:
            // this lasts one file check, and the product has no spinners.
            Paper.base.ignoresSafeArea()
        case .some(true):
            FirstRunFlow(
                openSystemSettings: RootView.openSystemSettings,
                finish: { finishOnboarding(in: store) }
            )
        case .some(false):
            room(in: store)
        }
    }

    /// Whether the reader has been introduced. Unreadable counts as yes: the
    /// product working is a better failure than an introduction nobody can
    /// get past.
    private func readFirstRun(_ store: TypviaStore) async {
        guard isOnboarding == nil else { return }
        let done = (try? await store.perform { $0.onboardingCompleted() }) ?? true
        isOnboarding = !done
        if done { links.onboardingFinished() }
    }

    /// The reader is through — by reaching the end or by saying they would
    /// rather just start. Both are being through.
    ///
    /// The screen changes first and the marker is written after: a marker that
    /// fails to write brings the introduction back next launch, which is a far
    /// smaller thing than a reader waiting on a file.
    private func finishOnboarding(in store: TypviaStore) {
        isOnboarding = false
        links.onboardingFinished()
        Task { try? await store.perform { try $0.markOnboardingComplete() } }
    }

    /// Sends the reader to this app's page in the system settings, where the
    /// keyboard is added. Nothing else in the shell leaves the app.
    private static func openSystemSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
    }

    @ViewBuilder
    private func room(in store: TypviaStore) -> some View {
        switch room {
        case .library:
            LibraryScreen(
                store: store,
                actions: LibraryActions(
                    open: { open = .existing($0) },
                    compose: { open = .fresh(nil) },
                    selectRoom: select,
                    availableRooms: RootView.builtRooms
                )
            )
        case .vault:
            VaultScreen(
                store: store,
                actions: VaultActions(
                    open: { open = .existing($0) },
                    // Opened on the secret kind already: this room's only way
                    // in is putting one away, and arriving at a blank editor
                    // with eight kinds to choose from hides that.
                    putOneAway: { open = .fresh(.secret) },
                    selectRoom: select,
                    availableRooms: RootView.builtRooms
                )
            )
        case .settings:
            SettingsScreen(
                store: store,
                preferences: preferences,
                actions: SettingsActions(
                    pair: { isPairing = true },
                    openSystemSettings: RootView.openSystemSettings,
                    selectRoom: select,
                    availableRooms: RootView.builtRooms
                )
            )
        case .home, .ai:
            HomeScreen(
                store: store,
                actions: HomeActions(
                    open: { open = .existing($0) },
                    compose: { open = .fresh(nil) },
                    selectRoom: select,
                    availableRooms: RootView.builtRooms
                )
            )
        }
    }

    @ViewBuilder
    private func openSnippet(in store: TypviaStore) -> some View {
        switch open {
        case let .existing(id):
            DetailScreen(
                store: store,
                snippetId: id,
                actions: DetailActions(close: { closeSnippet(in: store) }, copy: copyToClipboard)
            )
            .transition(.opacity)
        case let .fresh(sort):
            DetailScreen(
                store: store,
                clipboard: nil,
                sort: sort,
                actions: DetailActions(close: { closeSnippet(in: store) }, copy: copyToClipboard)
            )
            .transition(.opacity)
        case .none:
            EmptyView()
        }
    }

    /// Leaving an entry is the moment the library may have changed — it is the
    /// one screen that writes. The extensions are told on the way out rather
    /// than on every keystroke of an edit.
    private func closeSnippet(in store: TypviaStore) {
        open = nil
        Task { await publishSnapshot(store) }
    }

    /// Republishes what the keyboard and the widget read, and asks the system
    /// to redraw the widget. Only the app reaches here: the extensions read
    /// this document, they never write it.
    private func publishSnapshot(_ store: TypviaStore) async {
        guard await SnapshotPublish.run(store: store) else { return }
        WidgetCenter.shared.reloadAllTimelines()
    }

    /// Copying a normal snippet is the clipboard's ordinary use. Secrets never
    /// reach here: their screen has no copy verb at all.
    private func copyToClipboard(_ body: String) {
        UIPasteboard.general.string = body
    }

    private enum OpenSnippet: Equatable {
        case existing(String)
        /// A blank editor, on a kind already chosen where the room that led
        /// here knows which one.
        case fresh(TypeSort?)
    }

    /// The rooms that have screens. All four the bar prints now have one.
    static let builtRooms: Set<Room> = [.home, .library, .vault, .settings]

    /// Acts on a link. Where it cannot go — a snippet that no longer exists —
    /// the screen it opens says so, rather than this deciding in advance.
    private func follow(_ link: DeepLink) {
        switch link {
        case .home, .search:
            room = .home
            open = nil
        case let .snippet(id):
            open = .existing(id)
        }
    }

    private func select(_ destination: Room) {
        guard RootView.builtRooms.contains(destination) else { return }
        room = destination
    }

    /// The app's own container. The extensions read a snapshot from the App
    /// Group instead; the database itself has exactly one process.
    nonisolated private static var dataDirectory: URL {
        URL.applicationSupportDirectory.appendingPathComponent("Typvia", isDirectory: true)
    }
}

/// The database could not be opened — a failed migration, or a container the
/// system will not give us. There is no screen behind this and nothing to
/// retry into, so it says the one true thing: the data was left alone.
private struct UnavailableView: View {
    @Environment(\.tr) private var tr

    var body: some View {
        ZStack {
            Paper.base.ignoresSafeArea()
            VStack(alignment: .leading, spacing: 14) {
                Caret(height: 34, capped: true)
                Text(tr("Your library was left untouched.", "你的资料库没有被动过。"))
                    .typviaType(.title2)
                    .foregroundStyle(Paper.ink)
                Text(
                    tr(
                        "Typvia could not open it on this device. Nothing was written, and reopening the app tries again.",
                        "这台设备上打不开它。没有写入任何东西,重开一次会再试。"
                    )
                )
                .typviaType(.body)
                .foregroundStyle(Paper.ink2)
            }
            .padding(40)
        }
        .room(.home)
    }
}
