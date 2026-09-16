// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Read-only access to the KeyboardSnapshot document for the home-screen
// widget: the same App Group file the keyboard reads — zero new data
// surface. Row selection lives in Rust (`widgetEntries`: favorites first,
// sensitive excluded entirely), so the ordering/content rules have exactly
// one implementation across platforms.

import Foundation

enum WidgetSnapshot {
    /// Must match the `com.apple.security.application-groups` entitlement
    /// of the host app and this extension.
    static let appGroupId = "group.dev.typvia.mobile"
    /// File name the host writes atomically; the host side owns this name.
    static let snapshotFileName = "snapshot.json"

    /// Rows for the requested size tier. Empty covers every degraded case
    /// the same way — no container, no file yet, unreadable bytes, failed
    /// validation — because the widget's answer to all of them is the same
    /// empty state. Errors are deliberately not logged: snapshot-adjacent
    /// messages could reach system logs.
    static func rows(limit: UInt32) -> [SnapshotEntry] {
        guard
            let container = FileManager.default.containerURL(
                forSecurityApplicationGroupIdentifier: appGroupId)
        else { return [] }
        let url = container.appendingPathComponent(snapshotFileName)
        guard
            let data = try? Data(contentsOf: url),
            let json = String(data: data, encoding: .utf8)
        else { return [] }
        return (try? widgetEntries(json: json, limit: limit)) ?? []
    }
}
