// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// The vault, shut. Three things and nothing else.
struct ShutVault: View {
    @Environment(\.tr) private var tr

    let isOpening: Bool
    @Binding var showsPasswordField: Bool
    @Binding var password: String
    let openWithBiometrics: () -> Void
    let openWithPassword: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer(minLength: 0)
            Caret(height: VaultMetrics.caretHeight, capped: true, color: VaultRoom.accent)
                .padding(.bottom, VaultMetrics.caretGap)
            Text(tr("What is in here is only on your device.", "这里的东西只在你手上。"))
                .typviaType(.title2)
                .foregroundStyle(VaultRoom.ink)
                .fixedSize(horizontal: false, vertical: true)
            // Seven dots where a count would be. How much is in the vault is
            // a fact about the vault, and a shut vault states none.
            Text(BodyPreview.shortMask)
                .typviaType(.mono)
                .foregroundStyle(VaultRoom.ink3)
                .padding(.top, VaultMetrics.lineGap)
                .accessibilityLabel(tr("Not counted", "不计数"))
            actions
                .padding(.top, VaultMetrics.actionGap)
            Spacer(minLength: 0)
        }
        // The room fills the page it is on. A dark room sized to its text
        // leaves the page showing down one side, which is how the ink room
        // stops being a room.
        .frame(maxWidth: .infinity, alignment: .leading)
        .opacity(isOpening ? VaultMetrics.openingDim : 1)
    }

    @ViewBuilder
    private var actions: some View {
        VStack(alignment: .leading, spacing: VaultMetrics.actionRowGap) {
            VaultAction(title: tr("Open with Face ID", "用面容 ID 打开"), isPrimary: true) {
                openWithBiometrics()
            }
            if showsPasswordField {
                PasswordField(password: $password, submit: openWithPassword)
            } else {
                VaultAction(title: tr("Use the password instead", "用密码代替")) {
                    showsPasswordField = true
                }
            }
        }
    }
}

/// What the room says when a door did not open.
///
/// It takes both halves. The core answers with a kind of refusal, and this
/// side knows which door was pressed; neither alone can tell a wrong master
/// password from a Face ID that was never given a key to hold. Said with the
/// kind alone, the room blamed Face ID for both — and for a check that had
/// never been asked to run.
enum VaultRoomCopy {
    /// What is still true comes first: the vault is shut.
    static func headline(door: VaultDoor, refusal: Refusal?, _ tr: Translator) -> String {
        switch (door, refusal) {
        case (.faceId, .clash):
            // Nothing was recognised or refused: there is no gated copy of the
            // key on this device, so the read found nothing to ask about.
            return tr(
                "Face ID has no key on this device yet. The vault is still shut.",
                "面容 ID 在这台设备上还没有钥匙。库还是锁着的。"
            )
        case (.faceId, _):
            return tr(
                "Face ID did not open it. The vault is still shut.",
                "面容 ID 没能打开它。库还是锁着的。"
            )
        case (.masterPassword, .notPermitted):
            // A wrong password and too many tries arrive as one kind, so this
            // names neither.
            return tr(
                "It did not open with that. The vault is still shut.",
                "用那个没能打开。库还是锁着的。"
            )
        case (.masterPassword, _):
            return tr(
                "It did not open just now. The vault is still shut.",
                "刚才没能打开。库还是锁着的。"
            )
        }
    }

    static func detail(door: VaultDoor, refusal: Refusal?, _ tr: Translator) -> String {
        switch (door, refusal) {
        case (.faceId, .clash):
            return tr(
                "The master password opens it. Settings 04 is where Face ID is given a copy of the key.",
                "用主密码就能打开。到设置 04 可以给面容 ID 留一份钥匙副本。"
            )
        case (.faceId, _):
            return tr(
                "Try again, or use the master password — either one opens it. There is no attempt limit, and failing does not lock the device.",
                "再试一次,或者用主密码——两种都能打开,没有次数上限里的惩罚,失败也不会锁死设备。"
            )
        case (.masterPassword, _):
            return tr(
                "Type it again — there is no attempt limit, and failing does not lock the device.",
                "再输一次——没有次数上限,输错也不会锁死设备。"
            )
        }
    }

    /// Whether pressing the same door again could answer differently.
    ///
    /// A Face ID with no key copy to read will find none the second time, and
    /// this product does not offer a word whose whole effect is the same
    /// refusal. For the master password the field is already on screen, which
    /// is the retry.
    static func retryCouldDiffer(door: VaultDoor, refusal: Refusal?) -> Bool {
        door == .faceId && refusal != .clash
    }
}

/// A door that did not open.
///
/// What is said first is what is still true: the vault is shut, and it opens
/// without a network. Then the way in that is left. No shaking, no red, no
/// count of remaining attempts — there is no attempt limit to count.
struct DidNotOpen: View {
    @Environment(\.tr) private var tr

    let door: VaultDoor
    let refusal: Refusal?
    @Binding var showsPasswordField: Bool
    @Binding var password: String
    let retry: () -> Void
    let openWithPassword: () -> Void

    var body: some View {
        let offersRetry = VaultRoomCopy.retryCouldDiffer(door: door, refusal: refusal)
        return VStack(alignment: .leading, spacing: 0) {
            Spacer(minLength: 0)
            // The caret rests for a beat rather than animating an alarm.
            Caret(height: VaultMetrics.caretHeight, behaviour: .still, capped: true, color: VaultRoom.accent)
                .padding(.bottom, VaultMetrics.caretGap)
            Text(VaultRoomCopy.headline(door: door, refusal: refusal, tr))
                .typviaType(.title2)
                .foregroundStyle(VaultRoom.ink)
                .fixedSize(horizontal: false, vertical: true)
            Text(VaultRoomCopy.detail(door: door, refusal: refusal, tr))
                .typviaType(.bodyS)
                .foregroundStyle(VaultRoom.ink2)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, VaultMetrics.lineGap)
            VStack(alignment: .leading, spacing: VaultMetrics.actionRowGap) {
                if offersRetry {
                    VaultAction(title: tr("Try again", "再试一次"), isPrimary: true, action: retry)
                }
                // Where trying the same door again is the same answer, the way
                // that does work is already open rather than one tap away.
                if showsPasswordField || !offersRetry {
                    PasswordField(password: $password, submit: openWithPassword)
                } else {
                    VaultAction(title: tr("Enter the master password", "输入主密码")) {
                        showsPasswordField = true
                    }
                }
            }
            .padding(.top, VaultMetrics.actionGap)
            Text(
                tr(
                    "It opens offline: the key is on this device and the check never goes to a server.",
                    "离线也能开:密钥在这台设备上,验证不经过服务器。"
                )
            )
            .typviaType(.mono)
            .foregroundStyle(VaultRoom.ink3)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, VaultMetrics.actionGap)
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The vault, open: names and when each was last taken out. The bodies stay
/// where they are until one is asked for, one at a time.
struct OpenVault: View {
    @Environment(\.tr) private var tr

    let entries: [VaultEntry]
    let clock: RelockClock?
    let open: ((String) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            countLine
            relockLine
            // Neither lazy nor scrolling. A vault holds at most a hundred
            // rows, so laziness buys nothing — and both a lazy stack and a
            // scroll view render as an empty box when this is put on a page
            // for review. Scrolling belongs to the screen, which is one level
            // up and has the height to scroll within.
            VStack(alignment: .leading, spacing: VaultMetrics.rowGap) {
                ForEach(entries) { entry in
                    row(entry)
                }
            }
            .padding(.top, VaultMetrics.listTop)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var countLine: some View {
        Text(tr.counted(entries.count, "piece", "pieces", "\(entries.count) 枚"))
            .typviaType(.heading)
            .foregroundStyle(VaultRoom.ink)
    }

    /// The idle window as a line that shrinks. Visible out of the corner of an
    /// eye, and not hurrying anybody.
    @ViewBuilder
    private var relockLine: some View {
        if let clock {
            VStack(alignment: .leading, spacing: VaultMetrics.rowInnerGap) {
                Text(tr("Shuts again in \(clock.clockText)", "\(clock.clockText) 后回锁"))
                    .typviaType(.mono)
                    .foregroundStyle(VaultRoom.ink3)
                Rectangle()
                    .fill(VaultRoom.accent)
                    .frame(height: VaultMetrics.clockLineHeight)
                    .scaleEffect(x: clock.remaining, anchor: .leading)
                    .accessibilityHidden(true)
            }
            .padding(.top, VaultMetrics.clockGap)
        }
    }

    private func row(_ entry: VaultEntry) -> some View {
        HStack(alignment: .top, spacing: VaultMetrics.lineGap) {
            TypeSortMark(.secret, on: .inkRoom, accessibilityLabel: tr("Secret", "密钥"))
            VStack(alignment: .leading, spacing: VaultMetrics.rowInnerGap) {
                Text(entry.title)
                    .typviaType(.sectionTitle)
                    .foregroundStyle(VaultRoom.ink)
                Text(lastUsed(entry))
                    .typviaType(.mono)
                    .foregroundStyle(VaultRoom.ink3)
            }
            Spacer(minLength: 0)
            if let open {
                Button { open(entry.id) } label: {
                    Text(tr("Take out", "取出"))
                        .typviaType(.bodyS)
                        .foregroundStyle(VaultRoom.accent)
                        .frame(minHeight: Tokens.Hit.minimum)
                }
                .buttonStyle(.plain)
            }
        }
        .accessibilityElement(children: .combine)
    }

    private func lastUsed(_ entry: VaultEntry) -> String {
        guard let stamp = entry.lastUsedAt else {
            return tr("never taken out", "从未取出")
        }
        return tr(
            "taken out \(RelativeStamp.text(for: stamp, language: tr.language))",
            "\(RelativeStamp.text(for: stamp, language: tr.language))取出"
        )
    }
}

/// No vault has been made here yet. Not a locked door — there is nothing to
/// unlock, and saying "locked" would send the reader looking for a key.
struct AbsentVault: View {
    @Environment(\.tr) private var tr

    /// Absent only where this host has no editor to open. Present, it is the
    /// room's one way in — and until it existed this screen was two lines of
    /// prose telling the reader a vault appears when they save a secret, with
    /// nowhere on it to save one from.
    var putOneAway: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer(minLength: 0)
            Caret(height: VaultMetrics.caretHeight, capped: true, color: VaultRoom.accent)
                .padding(.bottom, VaultMetrics.caretGap)
            Text(tr("There is no vault on this device yet.", "这台设备上还没有保险库。"))
                .typviaType(.title2)
                .foregroundStyle(VaultRoom.ink)
                .fixedSize(horizontal: false, vertical: true)
            Text(
                tr(
                    "Save a password or a key and one is made, with a master password only you know.",
                    "存下一条密码或密钥就会建起来,主密码只有你知道。"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(VaultRoom.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, VaultMetrics.lineGap)
            if let putOneAway {
                // The verb says the act, not the machinery: nobody sets out to
                // make a vault. The master password is asked for at the moment
                // the first secret is actually put away.
                VaultAction(
                    title: tr("Put something in it", "存一条进去"),
                    isPrimary: true,
                    action: putOneAway
                )
                .padding(.top, VaultMetrics.actionGap)
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A word, in the ink room. The vault's key action is one of the three places
/// its accent is allowed.
struct VaultAction: View {
    let title: String
    var isPrimary = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .typviaType(isPrimary ? .sectionTitle : .bodyS)
                .foregroundStyle(isPrimary ? VaultRoom.accent : VaultRoom.ink2)
                .frame(minHeight: Tokens.Hit.minimum)
        }
        .buttonStyle(.plain)
    }
}

/// The master password, typed. It is never held anywhere but this field, and
/// the field is emptied the moment it has been handed over.
struct PasswordField: View {
    @Environment(\.tr) private var tr

    @Binding var password: String
    let submit: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: VaultMetrics.fieldGap) {
            SecureField("", text: $password, prompt: Text(tr("Master password", "主密码")).foregroundColor(VaultRoom.ink3))
                .typviaType(.body)
                .foregroundStyle(VaultRoom.ink)
                .tint(VaultRoom.accent)
                .textContentType(.password)
                .submitLabel(.go)
                .onSubmit(submit)
            Rectangle()
                .fill(VaultRoom.accent)
                .frame(height: Tokens.Line.searchHeight)
                .opacity(Tokens.Line.searchFocusOpacity)
                .accessibilityHidden(true)
            VaultAction(title: tr("Open", "打开"), isPrimary: true, action: submit)
        }
    }
}
