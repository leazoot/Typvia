// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import TypviaKit
import WidgetKit

@main
struct TypviaWidgetBundle: WidgetBundle {
    var body: some Widget {
        TypviaWidget()
    }
}

struct TypviaWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "dev.typvia.mobile.widget", provider: Provider()) { entry in
            WidgetFace(size: entry.size, rows: entry.rows, total: entry.total)
                // Read when the entry was built, not here: a widget's face is
                // rendered from a value, and asking the store during a render
                // would be asking at the one moment the answer cannot change
                // anything.
                .uiPreferences(entry.chosen)
                .widgetPaper()
        }
        .configurationDisplayName("Typvia")
        .description("Your snippets, one tap from the home screen.")
        .supportedFamilies([.systemSmall, .systemMedium, .accessoryRectangular])
    }
}

extension View {
    /// Puts the paper behind the whole widget rather than behind the words.
    ///
    /// From iOS 17 a widget that declares no container background gets the
    /// system's own — the face then sits as a cream card inside a white one,
    /// with its text against the top edge of the smaller card. Declaring it
    /// also hands the system the margins, which is why nothing is padded here;
    /// on iOS 16, where there are no such margins, the face pads itself.
    @ViewBuilder
    func widgetPaper() -> some View {
        if #available(iOS 17.0, *) {
            containerBackground(Paper.base, for: .widget)
        } else {
            padding(WidgetMetrics.margin).background(Paper.base)
        }
    }
}

enum WidgetMetrics {
    static let margin: CGFloat = 16
}

struct Entry: TimelineEntry {
    let date: Date
    let size: WidgetSize
    let rows: [WidgetRow]
    let total: UInt32
    /// The reader's language and appearance, as they stood when this entry was
    /// built. This process is not told when either changes, so it is read on
    /// every entry rather than held.
    var chosen = UiPreferences.Chosen()
}

/// Reads the snapshot the app wrote. The widget never opens the database, and
/// it never asks for a body — the rows it draws are titles and marks.
struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> Entry {
        // An empty face, not sample rows: on a home screen a sample snippet is
        // indistinguishable from a real one.
        Entry(
            date: .distantPast,
            size: size(for: context),
            rows: [],
            total: 0,
            chosen: UiPreferences.chosen()
        )
    }

    func getSnapshot(in context: Context, completion: @escaping (Entry) -> Void) {
        completion(read(context))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<Entry>) -> Void) {
        // Refresh is event-driven from the host, never on a clock: the snippet
        // set only changes when the reader changes it.
        completion(Timeline(entries: [read(context)], policy: .never))
    }

    private func read(_ context: Context) -> Entry {
        let family = size(for: context)
        let chosen = UiPreferences.chosen()
        guard let json = SharedContainer.snapshotJSON() else {
            // An empty face is still a face the reader chose the language of.
            return Entry(date: .distantPast, size: family, rows: [], total: 0, chosen: chosen)
        }
        // Which snippets a widget gets, and in what order, is the shared
        // engine's answer — the extension only draws it.
        let entries = (try? widgetEntries(json: json, limit: UInt32(family.rowLimit))) ?? []
        // The count is how many the snapshot carries, not how many rows fit:
        // this list is cut to the family's row limit, so counting the rows
        // would report the size of the widget rather than the size of the
        // library. It stops at the snapshot — the app's own total is a number
        // this process cannot check.
        let total = (try? parseSnapshot(json: json).entryTotal) ?? UInt32(entries.count)
        return Entry(
            date: .distantPast,
            size: family,
            rows: entries.map(WidgetRow.init(entry:)),
            total: total,
            chosen: chosen
        )
    }

    private func size(for context: Context) -> WidgetSize {
        switch context.family {
        case .systemMedium: .medium
        case .accessoryRectangular, .accessoryInline, .accessoryCircular: .lock
        default: .small
        }
    }
}
