// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import WidgetKit
import TypviaKit

@main
struct TypviaApp: App {
    /// The background handler has to be registered before launching finishes,
    /// so it happens here rather than in a view. The store it uses is the
    /// app's one handle, opened lazily by the root.
    init() {
        BackgroundSync.register(
            store: { RootView.sharedStore },
            // A background round that landed rows has rewritten what the
            // widget draws from; the system will not notice on its own.
            afterRound: { WidgetCenter.shared.reloadAllTimelines() }
        )
    }

    var body: some Scene {
        WindowGroup {
            RootView()
        }
    }
}
