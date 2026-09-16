// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI

/// What the settings room can hand back.
public struct SettingsActions {
    public var pair: (() -> Void)?
    /// Leaves for this app's page in the system settings, where a keyboard is
    /// added. Nothing else in this room leaves the app.
    public var openSystemSettings: (() -> Void)?
    public var selectRoom: ((Room) -> Void)?
    public var availableRooms: Set<Room>

    public init(
        pair: (() -> Void)? = nil,
        openSystemSettings: (() -> Void)? = nil,
        selectRoom: ((Room) -> Void)? = nil,
        availableRooms: Set<Room> = []
    ) {
        self.pair = pair
        self.openSystemSettings = openSystemSettings
        self.selectRoom = selectRoom
        self.availableRooms = availableRooms
    }
}

/// Settings, as an editorial contents page.
///
/// Numbers stand in the left margin and double as the navigation; each chapter
/// is a whole page rather than a folding panel; there is not one icon in the
/// room. Switches appear only inside a chapter, never on this level, and never
/// more than two to a screen.
public struct SettingsScreen: View {
    @StateObject private var model: SettingsModel
    @ObservedObject private var preferences: UiPreferences
    @Environment(\.tr) private var tr

    private let actions: SettingsActions

    public init(
        store: TypviaStore,
        preferences: UiPreferences,
        actions: SettingsActions = SettingsActions()
    ) {
        _model = StateObject(wrappedValue: SettingsModel(store: store))
        self.preferences = preferences
        self.actions = actions
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let chapter = model.open {
                ChapterPage(
                    chapter: chapter, model: model, actions: actions, preferences: preferences
                ) {
                    model.closeChapter()
                }
            } else {
                contents
            }
            Spacer(minLength: 0)
            RoomBar(
                current: .settings,
                available: actions.availableRooms,
                select: actions.selectRoom
            )
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Paper.base)
        .room(model.open == .ai ? .ai : .settings)
        .task { await model.load() }
    }

    private var contents: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: 0) {
                Text(tr("Settings", "设置"))
                    .typviaType(.title2)
                    .foregroundStyle(Paper.ink)
                Text(tr("Local-first · open source", "本地优先 · 开源"))
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink3)
                    .padding(.top, SettingsMetrics.subtitleGap)
                VStack(alignment: .leading, spacing: Tokens.Space.group) {
                    ForEach(SettingsChapter.allCases) { chapter in
                        ChapterLine(
                            chapter: chapter,
                            value: value(for: chapter),
                            open: { model.openChapter(chapter) }
                        )
                    }
                }
                .padding(.top, Tokens.Space.section)
            }
            .padding(.horizontal, Tokens.Space.screenPadding)
            .padding(.top, SettingsMetrics.top)
        }
    }

    /// The right-hand column: each chapter's current state, in the reader's
    /// language. It is what a reader scans, so it is never left blank.
    private func value(for chapter: SettingsChapter) -> String {
        switch chapter {
        case .sync:
            return model.deviceCount > 0
                ? tr.counted(model.deviceCount, "device", "devices", "\(model.deviceCount) 台设备")
                : tr("not set up", "未设置")
        case .keyboard:
            return KeyboardChapterCopy.contentsValue(KeyboardInstall.current(), tr)
        case .ai:
            switch model.engine {
            case .onThisDevice: return tr("on this device", "本地")
            case .ownKey: return tr("your own key", "自己的密钥")
            case .off: return tr("off", "已关闭")
            }
        case .vault:
            return VaultChapterCopy.contentsValue(model.vault, tr)
        case .appearance:
            switch preferences.language {
            case .system: return tr("follows the system", "跟随系统")
            case .en: return "English"
            case .zh: return "中文"
            }
        case .data:
            return tr.counted(model.stats.total, "piece", "pieces", "\(model.stats.total) 枚")
        }
    }
}

enum SettingsMetrics {
    static let top: CGFloat = 22
    static let subtitleGap: CGFloat = 8
    static let numberWidth: CGFloat = 44
    static let lineGap: CGFloat = 16
    static let lineInnerGap: CGFloat = 5
    static let headerNumberTop: CGFloat = 14
    static let headerGap: CGFloat = 18
    static let paragraphGap: CGFloat = 26
    static let rowGap: CGFloat = 22
    static let optionGap: CGFloat = 14
    static let optionPadding: CGFloat = 16
    static let figureGap: CGFloat = 10
    static let actionGap: CGFloat = 26
    static let dangerGap: CGFloat = 40
    static let switchRowGap: CGFloat = 30
}
