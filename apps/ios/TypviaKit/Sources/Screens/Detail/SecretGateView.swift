// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The vault, asked for from inside the editor.
///
/// It takes over the composing stick rather than opening a dialog: the product
/// has one modal and it is the vault room's own. What it says first is what is
/// about to happen to the words already typed — they are going somewhere that
/// needs making or opening — and never that anything has gone wrong, because
/// nothing has.
struct SecretGateView: View {
    @Environment(\.tr) private var tr
    @ObservedObject var model: DetailModel

    let gate: SecretGate

    @State private var password = ""
    @State private var confirmation = ""
    @State private var showsPasswordField = false

    var body: some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickRowGap) {
            Text(statement)
                .typviaType(.sectionTitle)
                .foregroundStyle(VaultRoom.ink)
                .fixedSize(horizontal: false, vertical: true)
            switch gate {
            case .setup:
                setup
            case .shut:
                shut
            }
            troubleLine
            VaultAction(title: tr("Not now", "先不")) {
                clear()
                model.dismissGate()
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var statement: String {
        switch gate {
        case .setup:
            return tr(
                "There is no vault on this device yet. This is the secret that makes one.",
                "这台设备上还没有保险库。这一条会把它建起来。"
            )
        case .shut:
            return tr(
                "The vault is shut. Open it and this goes straight in.",
                "保险库锁着。开一下,这条就直接进去。"
            )
        }
    }

    private var setup: some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
            secureField(tr("Master password", "主密码"), text: $password)
            secureField(tr("Type it again", "再输一次"), text: $confirmation)
            Text(
                tr(
                    "It is not stored and not sent anywhere — the vault opens on this device or not at all.",
                    "主密码不落库、不上传——保险库只在这台设备上开得了。"
                )
            )
            .typviaType(.mono)
            .foregroundStyle(VaultRoom.ink3)
            .fixedSize(horizontal: false, vertical: true)
            VaultAction(title: tr("Make it and save", "建起来并存下"), isPrimary: true) {
                Task {
                    let password = password
                    let confirmation = confirmation
                    // Cleared before the call rather than after it: the fields
                    // have handed over what they hold and have no further use
                    // for it, whichever way the answer comes back.
                    clear()
                    await model.makeVault(password: password, confirmation: confirmation)
                }
            }
            .disabled(password.isEmpty || confirmation.isEmpty)
            .opacity(password.isEmpty || confirmation.isEmpty ? DetailMetrics.disabledOpacity : 1)
        }
    }

    private var shut: some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
            VaultAction(title: tr("Open with Face ID", "用面容 ID 打开"), isPrimary: true) {
                Task { await model.openVaultWithBiometrics() }
            }
            if showsPasswordField {
                secureField(tr("Master password", "主密码"), text: $password)
                VaultAction(title: tr("Open and save", "打开并存下"), isPrimary: true) {
                    Task {
                        let password = password
                        clear()
                        await model.openVault(password: password)
                    }
                }
                .disabled(password.isEmpty)
                .opacity(password.isEmpty ? DetailMetrics.disabledOpacity : 1)
            } else {
                VaultAction(title: tr("Use the password instead", "用密码代替")) {
                    showsPasswordField = true
                }
            }
        }
    }

    /// What the vault said, in a sentence the reader can act on. A mismatch is
    /// the screen's own observation; everything else is the core's.
    ///
    /// It is not red. Nothing in the ink room goes red — a password that did
    /// not open the vault is a password that did not open the vault, and the
    /// room says so in the same ink it says everything else in.
    @ViewBuilder
    private var troubleLine: some View {
        if let sentence = troubleSentence {
            Text(sentence)
                .typviaType(.caption)
                .foregroundStyle(VaultRoom.ink2)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var troubleSentence: String? {
        if model.mismatch {
            return tr("The two do not match.", "两次输入不一样。")
        }
        guard let refusal = model.refusal else { return nil }
        switch (gate, refusal) {
        // While making a vault, a correctable input can only be the one rule
        // the vault has about the password it is handed: how short it may be.
        // The number is the core's; this only repeats it.
        case (.setup, .invalid):
            let minimum = Int(masterPasswordMinLength())
            return tr(
                "A master password needs at least \(minimum) characters.",
                "主密码至少要 \(minimum) 个字符。"
            )
        case (.shut, .notPermitted):
            return tr(
                "That did not open it. The vault is still shut.",
                "这个没打开它。库还是锁着的。"
            )
        default:
            return refusal.sentence(tr)
        }
    }

    private func secureField(_ label: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: DetailMetrics.stickFieldGap) {
            SecureField("", text: text, prompt: Text(label).foregroundColor(VaultRoom.ink3))
                .typviaType(.body)
                .foregroundStyle(VaultRoom.ink)
                .tint(VaultRoom.accent)
                .textContentType(.password)
            Rectangle()
                .fill(VaultRoom.accent)
                .frame(height: Tokens.Line.searchHeight)
                .opacity(Tokens.Line.searchFocusOpacity)
                .accessibilityHidden(true)
        }
    }

    private func clear() {
        password = ""
        confirmation = ""
    }
}
