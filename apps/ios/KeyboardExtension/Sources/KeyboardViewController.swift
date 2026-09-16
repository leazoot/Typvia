// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import TypviaKit
import UIKit

/// The keyboard's host controller. UIKit is not a choice here — the system
/// hands extensions a `UIInputViewController` — so SwiftUI is hosted inside it.
///
/// This process never opens the database. It reads the snapshot the app wrote
/// into the shared container, and it types character by character into
/// whatever field the reader is in, because the product types rather than
/// pastes: nothing here touches the clipboard.
final class KeyboardViewController: UIInputViewController {
    private let model = KeyboardModel()
    /// The reader's language and appearance, read from the App Group the app
    /// writes them to. This process is not in the app's composition and is
    /// never told, so it asks — here and again every time it comes up.
    private let preferences = UiPreferences()
    /// The characters this keyboard has actually put in the field, so undo
    /// takes back exactly those and not one more. A body that was still being
    /// typed when the reader changed their mind is the case that matters: the
    /// count is what went in, never what was going to.
    private var typed = 0
    private var typing: Task<Void, Never>?

    override func viewDidLoad() {
        super.viewDidLoad()

        let panel = UIHostingController(
            rootView: PanelHost(model: model, preferences: preferences, controller: self)
        )
        panel.view.translatesAutoresizingMaskIntoConstraints = false
        addChild(panel)
        view.addSubview(panel.view)
        panel.didMove(toParent: self)

        NSLayoutConstraint.activate([
            panel.view.topAnchor.constraint(equalTo: view.topAnchor),
            panel.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            panel.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            panel.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            view.heightAnchor.constraint(equalToConstant: KeyboardBand.total),
        ])
    }

    /// A keyboard is built once and kept, so a reader who changes the app's
    /// language would otherwise go on getting the old one until the system
    /// ended this process. Asked at the moment the keyboard comes up, which is
    /// the moment after they changed it.
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        preferences.reload()
    }

    /// Types a body into the field the reader is actually in — character by
    /// character, at the delivery's rate, so it reads as typing rather than
    /// as a paste appearing.
    func type(_ body: String) {
        typing?.cancel()
        typed = 0
        switch TypeIn.plan(characterCount: body.count) {
        case .atOnce:
            textDocumentProxy.insertText(body)
            typed = body.count
            Haptic.medium.play()
        case let .character(interval):
            typing = Task { @MainActor [weak self] in
                for character in body {
                    guard !Task.isCancelled, let self else { return }
                    self.textDocumentProxy.insertText(String(character))
                    self.typed += 1
                    try? await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
                }
                guard !Task.isCancelled else { return }
                // The confirmation is at the end of the word, not the start:
                // it means "that is in", and until the last character it is
                // not in.
                Haptic.medium.play()
            }
        }
    }

    /// Takes back exactly what this keyboard put in, and nothing else.
    func undo() {
        typing?.cancel()
        typing = nil
        for _ in 0..<typed {
            textDocumentProxy.deleteBackward()
        }
        typed = 0
    }
}

/// Holds the panel's state so the controller does not have to be a view.
private struct PanelHost: View {
    @ObservedObject var model: KeyboardModel
    @ObservedObject var preferences: UiPreferences
    weak var controller: KeyboardViewController?

    var body: some View {
        KeyboardPanel(
            phase: model.phase,
            tiles: model.tiles,
            reach: model.reach,
            query: Binding(get: { model.query }, set: { model.query = $0 }),
            actions: KeyboardActions(
                insert: insert,
                undo: undo,
                takeOutSecret: takeOutSecret,
                switchKeyboard: { controller?.advanceToNextInputMode() }
            )
        )
        .uiPreferences(preferences)
        .onAppear { model.load() }
    }

    private func insert(_ id: String) {
        guard let body = model.body(for: id) else { return }
        // The haptic belongs to the controller now: it fires when the last
        // character lands, which is a different moment from this one.
        controller?.type(body)
        model.markUsed(id, title: model.tiles.first { $0.id == id }?.title ?? "")
    }

    private func undo() {
        controller?.undo()
        model.backToBench()
    }

    /// A secret is not taken out here. The keyboard shows what would happen
    /// and waits for the system's own gate; unlocking a vault inside a
    /// keyboard is a thing this product does not do.
    private func takeOutSecret(_ id: String) {
        let title = model.tiles.first { $0.id == id }?.title ?? ""
        model.openSecret(id, title: title)
    }
}
