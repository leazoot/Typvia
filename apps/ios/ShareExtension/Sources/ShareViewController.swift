// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import TypviaKit
import UIKit
import UniformTypeIdentifiers

/// The share sheet's entry point.
///
/// It writes one record into the App Group inbox and returns the reader to
/// where they were. It does not open the database — one process owns the
/// single writer, and it is the app — and it decides nothing the core decides:
/// the kind shown here is a guess the app will confirm.
final class ShareViewController: UIViewController {
    private var draft = SharedDraft(title: "", body: "", trigger: "", sort: .text)
    private var step: ShareStep = .composing
    private var host: UIHostingController<AnyView>?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear
        loadSharedText()
    }

    private func loadSharedText() {
        guard
            let item = extensionContext?.inputItems.first as? NSExtensionItem,
            let provider = item.attachments?.first(where: {
                $0.hasItemConformingToTypeIdentifier(UTType.plainText.identifier)
            })
        else { return present(text: "") }

        provider.loadItem(forTypeIdentifier: UTType.plainText.identifier) { [weak self] value, _ in
            let text = (value as? String) ?? (value as? NSAttributedString)?.string ?? ""
            DispatchQueue.main.async { self?.present(text: text) }
        }
    }

    private func present(text: String) {
        draft = SharedDraft(
            title: SharedDraft.title(from: text),
            body: text,
            trigger: "",
            sort: .text
        )
        installHost()
    }

    private func installHost() {
        host?.willMove(toParent: nil)
        host?.view.removeFromSuperview()
        host?.removeFromParent()

        let sheet = ShareSheet(
            draft: Binding(get: { self.draft }, set: { self.draft = $0 }),
            step: step,
            save: { [weak self] in self?.save() },
            cancel: { [weak self] in self?.close() },
            back: { [weak self] in self?.close() }
        )
        // Read as the host is built, which is every time this sheet appears.
        // This is a process of its own: nothing tells it the reader has since
        // chosen a different language, so it asks rather than remembers.
        .uiPreferences(UiPreferences.chosen())
        let controller = UIHostingController(rootView: AnyView(sheet))
        controller.view.translatesAutoresizingMaskIntoConstraints = false
        addChild(controller)
        view.addSubview(controller.view)
        controller.didMove(toParent: self)
        NSLayoutConstraint.activate([
            controller.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            controller.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            controller.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        host = controller
    }

    /// Hands the text to the inbox. A write that fails leaves the sheet open
    /// and says nothing was saved, because a sheet that closes quietly reads
    /// as "saved".
    private func save() {
        let trigger = draft.trigger.trimmingCharacters(in: .whitespaces)
        let item = InboxItem(
            title: draft.title,
            body: draft.body,
            trigger: trigger.isEmpty ? nil : trigger,
            snippetType: draft.sort.coreType,
            sharedAt: Int64(Date().timeIntervalSince1970 * 1000)
        )
        guard Inbox.append(item) else { return }
        // The position is the app's to know; the sheet reports what it did,
        // which is that the text is now waiting to be filed.
        step = .saved(trigger: item.trigger ?? "", position: UInt32(Inbox.read().count))
        installHost()
    }

    private func close() {
        extensionContext?.completeRequest(returningItems: nil)
    }
}
