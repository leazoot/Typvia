// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// Joining an account, as a page.
///
/// A page and not a sheet: the product has one modal and it is the vault's.
/// The reader says where the library lives, shows a code, and then compares
/// the check string with the other screen — and only that last step installs
/// anything, which is why it is the one the screen cannot skip past.
public struct PairingScreen: View {
    @StateObject private var model: PairingModel
    @Environment(\.tr) private var tr

    private let close: () -> Void

    public init(store: TypviaStore, close: @escaping () -> Void) {
        _model = StateObject(wrappedValue: PairingModel(store: store))
        self.close = close
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            ScrollView(showsIndicators: false) {
                content
                    .padding(.horizontal, Tokens.Space.screenPadding)
                    .padding(.bottom, Tokens.Space.section)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Paper.base)
        .room(.settings)
        .swipeToGoBack(leave)
        .task { await model.load() }
        // Leaving drops the session. A code left running on a screen nobody is
        // looking at is a code somebody else could still be answering.
        .onDisappear { if model.step != .joined { model.drop() } }
    }

    private var header: some View {
        HStack {
            Button(action: leave) {
                Text(tr("Back", "返回"))
                    .typviaType(.bodyS)
                    .foregroundStyle(Paper.ink2)
                    .frame(minHeight: Tokens.Hit.minimum)
            }
            .buttonStyle(.plain)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, Tokens.Space.screenPadding)
        .padding(.top, PairingMetrics.headerTop)
    }

    @ViewBuilder
    private var content: some View {
        switch model.step {
        case .none:
            JoinTarget(model: model)
        case let .offering(code, _):
            PairingOffer(
                // No window rather than a spent one: a fallback that said zero
                // would print "expired" over a code that was just handed out.
                code: model.offered ?? PairingCode(code: code, secondsLeft: nil),
                useTextCode: nil
            )
            .padding(.top, PairingMetrics.top)
            waiting
        case .comparing:
            comparing
        case .joined:
            joined
        case .dropped:
            dropped
        }
        refusalLine
    }

    /// Nothing has been installed yet, and the screen keeps saying so for as
    /// long as it is true.
    private var waiting: some View {
        Text(
            tr(
                "Waiting for the other device. Nothing has been installed here yet.",
                "正在等另一台设备。这台上还什么都没装。"
            )
        )
        .typviaType(.mono)
        .foregroundStyle(Paper.ink3)
        .fixedSize(horizontal: false, vertical: true)
        .padding(.top, PairingMetrics.gap)
    }

    private var comparing: some View {
        VStack(alignment: .leading, spacing: PairingMetrics.gap) {
            SasCheck(
                reveal: model.reveal ?? SasReveal(sas: "", shown: 0),
                // Not a device name: this side of the exchange has never been
                // told one. What it does hold is the account's fingerprint,
                // which is the thing the other screen can also show.
                note: tr(
                    "Account fingerprint \(model.fingerprint ?? "")",
                    "账户指纹 \(model.fingerprint ?? "")"
                ),
                confirm: { Task { await model.confirm() } },
                reject: model.drop
            )
            // Only asked for where it can be needed, and never stored: the
            // master key is re-wrapped under this device's own password.
            VStack(alignment: .leading, spacing: PairingMetrics.innerGap) {
                Text(
                    tr(
                        "Master password for this device's vault — only if the account has one.",
                        "这台设备保险库的主密码——只有账户里有保险库时才需要。"
                    )
                )
                .typviaType(.caption)
                .foregroundStyle(Paper.ink3)
                .fixedSize(horizontal: false, vertical: true)
                SecureField(
                    "",
                    text: $model.masterPassword,
                    prompt: Text(tr("Master password", "主密码")).foregroundColor(Paper.ink3)
                )
                .typviaType(.body)
                .foregroundStyle(Paper.ink)
                .tint(Room.settings.accent)
                .textContentType(.password)
            }
        }
        .padding(.top, PairingMetrics.top)
    }

    private var joined: some View {
        VStack(alignment: .leading, spacing: PairingMetrics.gap) {
            Text(tr("This device is in.", "这台设备加入了。"))
                .typviaType(.title2)
                .foregroundStyle(Paper.ink)
            Text(
                tr(
                    "Your snippets will arrive on their own. Secrets stay locked until you open the vault here.",
                    "片段会自己到。保险库里的东西在这台设备上解锁之前保持锁定。"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            StickVerb(title: tr("Done", "好"), isPrimary: true, action: close)
        }
        .padding(.top, PairingMetrics.top)
    }

    /// Stopped — by the reader saying the characters differ, or by leaving.
    /// What is said first is what is still true.
    private var dropped: some View {
        VStack(alignment: .leading, spacing: PairingMetrics.gap) {
            Text(tr("Nothing was installed here.", "这台设备上什么都没装。"))
                .typviaType(.title2)
                .foregroundStyle(Paper.ink)
            Text(
                tr(
                    "The session is dropped and the code is dead. Your snippets on both devices are untouched.",
                    "会话已经断开,那个码作废了。两台设备上已有的片段都没有被动过。"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: PairingMetrics.gap) {
                StickVerb(title: tr("Try again", "再来一次"), isPrimary: true, action: model.startOver)
                StickVerb(title: tr("Leave it", "先不了"), action: close)
            }
        }
        .padding(.top, PairingMetrics.top)
    }

    @ViewBuilder
    private var refusalLine: some View {
        if let refusal = model.refusal {
            VStack(alignment: .leading, spacing: PairingMetrics.innerGap) {
                Text(refusal.sentence(tr))
                    .typviaType(.caption)
                    .foregroundStyle(Paper.attention)
                    .fixedSize(horizontal: false, vertical: true)
                Text(tr("Nothing was installed here.", "这台设备上什么都没装。"))
                    .typviaType(.caption)
                    .foregroundStyle(Paper.ink3)
            }
            .padding(.top, PairingMetrics.gap)
        }
    }

    private func leave() {
        if model.step != .joined { model.drop() }
        close()
    }
}

/// Where the library this device is joining actually lives.
///
/// The delivery does not draw this step — its pairing frames start at the code
/// — but a code cannot be started without knowing which account it is for, so
/// it is built in the settings chapter's own language: words rather than
/// controls, one question at a time, no icons.
struct JoinTarget: View {
    @Environment(\.tr) private var tr
    @ObservedObject var model: PairingModel

    var body: some View {
        VStack(alignment: .leading, spacing: PairingMetrics.gap) {
            Text(tr("Join an account", "加入一个账户"))
                .typviaType(.title2)
                .foregroundStyle(Paper.ink)
            Text(
                tr(
                    "The key passes between the two devices. The server only carries the ciphertext.",
                    "钥匙在两台设备之间直接交换。服务器只搬密文。"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            if model.canHoldAKey == false {
                // Said here rather than after a filled-in form: this device
                // cannot keep a sync key, so no address would have worked.
                Text(
                    tr(
                        "This device cannot keep a sync key, so it cannot join an account yet. Everything else works, and your snippets stay on it.",
                        "这台设备存不住同步钥匙,所以暂时加入不了账户。其余功能照常,片段也都还在。"
                    )
                )
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)
            }
            HStack(spacing: PairingMetrics.gap) {
                choice(tr("My own server", "自建服务器"), .server)
                choice(tr("WebDAV", "WebDAV"), .webdav)
            }
            Text(explanation)
                .typviaType(.caption)
                .foregroundStyle(Paper.ink3)
                .fixedSize(horizontal: false, vertical: true)
            fields
            StickVerb(title: tr("Show the code", "出示配对码"), isPrimary: true) {
                Task { await model.begin() }
            }
            .disabled(!model.canBegin || model.isWorking)
            .opacity(model.canBegin && !model.isWorking ? 1 : PairingMetrics.disabled)
        }
        .padding(.top, PairingMetrics.top)
    }

    private var explanation: String {
        switch model.target {
        case .server:
            return tr(
                "The address and the account id are both on the other device's sync page.",
                "服务器地址与账户 ID 都在另一台设备的同步页上。"
            )
        case .webdav:
            return tr(
                "Point at the same folder the other device uses — the account is read from the storage itself.",
                "指向另一台设备用的同一个目录——账户信息直接从存储里读。"
            )
        }
    }

    @ViewBuilder
    private var fields: some View {
        VStack(alignment: .leading, spacing: PairingMetrics.gap) {
            field(tr("Address", "地址"), text: $model.address)
            switch model.target {
            case .server:
                field(tr("Account id", "账户 ID"), text: $model.accountId)
            case .webdav:
                field(tr("Username", "用户名"), text: $model.username)
                secureField(tr("Password", "密码"), text: $model.password)
            }
        }
    }

    private func choice(_ title: String, _ target: PairingTarget) -> some View {
        Button { model.target = target } label: {
            Text(title)
                .typviaType(.bodyS)
                .foregroundStyle(model.target == target ? Paper.ink : Paper.ink3)
                .frame(minHeight: Tokens.Hit.minimum)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(model.target == target ? [.isSelected] : [])
    }

    private func field(_ label: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: PairingMetrics.innerGap) {
            Text(label)
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
            TextField("", text: text)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink)
                .tint(Room.settings.accent)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            rule
        }
    }

    private func secureField(_ label: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: PairingMetrics.innerGap) {
            Text(label)
                .typviaType(.monoLabel)
                .foregroundStyle(Paper.ink3)
            SecureField("", text: text)
                .typviaType(.mono)
                .foregroundStyle(Paper.ink)
                .tint(Room.settings.accent)
                .textContentType(.password)
            rule
        }
    }

    private var rule: some View {
        Rectangle()
            .fill(Paper.rule)
            .frame(height: Tokens.Line.hairlineWidth)
            .accessibilityHidden(true)
    }
}

enum PairingMetrics {
    static let headerTop: CGFloat = 14
    static let top: CGFloat = 26
    static let gap: CGFloat = 18
    static let innerGap: CGFloat = 8
    static let disabled = 0.4
}
