// Home-screen and lock-screen widgets. The widget is a navigation surface:
// rows are title + type mark only (no body preview), every tap leaves for
// the app through the two controlled typvia:// deep links, and the
// lock-screen face is a red line — a pure entry with zero snippet data.
// Refresh is event-driven: the timeline policy is `.never` and the host
// reloads all timelines after each snapshot write.

import SwiftUI
import WidgetKit

@main
struct TypviaWidgetBundle: WidgetBundle {
    var body: some Widget {
        TypviaHomeWidget()
        TypviaLockEntryWidget()
    }
}

/// System surfaces render the system locale: the in-app locale preference
/// lives in WebView localStorage and cannot reach an extension.
private func tr(_ en: String, _ zh: String) -> String {
    (Locale.preferredLanguages.first?.hasPrefix("zh") ?? false) ? zh : en
}

private func deepLink(_ suffix: String) -> URL {
    // The scheme is our own two-route surface; a constant-prefix URL always
    // parses, so the fallback exists only to satisfy the type system.
    URL(string: "typvia://\(suffix)") ?? URL(fileURLWithPath: "/")
}

struct RowsEntry: TimelineEntry {
    let date: Date
    let rows: [SnapshotEntry]
}

struct RowsProvider: TimelineProvider {
    func placeholder(in context: Context) -> RowsEntry {
        RowsEntry(date: Date(), rows: [])
    }

    func getSnapshot(in context: Context, completion: @escaping (RowsEntry) -> Void) {
        completion(RowsEntry(date: Date(), rows: WidgetSnapshot.rows(limit: 8)))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<RowsEntry>) -> Void) {
        // One entry, no polling: the host pokes reloadAllTimelines() after
        // every successful snapshot write.
        let entry = RowsEntry(date: Date(), rows: WidgetSnapshot.rows(limit: 8))
        completion(Timeline(entries: [entry], policy: .never))
    }
}

struct TypviaHomeWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "TypviaHome", provider: RowsProvider()) { entry in
            HomeWidgetView(entry: entry)
        }
        .configurationDisplayName("Typvia")
        .description(tr("Your snippets, one tap from Home", "主屏一点,直达你的片段"))
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

struct HomeWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: RowsEntry

    /// Row count per size tier: small is a pure entry.
    private var rowLimit: Int {
        family == .systemLarge ? 8 : 4
    }

    var body: some View {
        Group {
            if family == .systemSmall {
                pureEntry
            } else {
                rowList
            }
        }
        .widgetBackground(Color(PanelTheme.paper))
    }

    /// Small tier: wordmark + caret, one tap lands on search.
    private var pureEntry: some View {
        VStack(alignment: .leading, spacing: 8) {
            wordmark
            Rectangle()
                .fill(Color(PanelTheme.accent))
                .frame(width: 2, height: 18)
            Spacer(minLength: 0)
            Text(tr("Search", "搜索"))
                .font(.system(size: 12))
                .foregroundColor(Color(PanelTheme.secondary))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(14)
        .widgetURL(deepLink("search"))
    }

    private var rowList: some View {
        VStack(alignment: .leading, spacing: 0) {
            Link(destination: deepLink("search")) {
                wordmark.frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.bottom, 4)
            if entry.rows.isEmpty {
                Spacer(minLength: 0)
                Link(destination: deepLink("search")) {
                    Text(tr("Open Typvia to save your first snippet", "打开 Typvia 保存第一条片段"))
                        .font(.system(size: 12))
                        .foregroundColor(Color(PanelTheme.secondary))
                        .frame(maxWidth: .infinity, alignment: .center)
                }
                Spacer(minLength: 0)
            } else {
                ForEach(entry.rows.prefix(rowLimit), id: \.id) { row in
                    Link(destination: deepLink("snippet/\(row.id)")) {
                        rowView(row)
                    }
                }
                Spacer(minLength: 0)
            }
        }
        .padding(.horizontal, 14)
        .padding(.top, 10)
        .padding(.bottom, 8)
    }

    private var wordmark: some View {
        Text("TYPVIA")
            .font(.system(size: 11, weight: .medium, design: .monospaced))
            .kerning(1.3)
            .foregroundColor(Color(PanelTheme.meta))
    }

    private func rowView(_ row: SnapshotEntry) -> some View {
        let mark = TypeMark.mark(for: row.snippetType)
        return HStack(spacing: 0) {
            Text(mark)
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundColor(Color(PanelTheme.meta))
                .frame(width: 30, alignment: .leading)
                .accessibilityHidden(true)
            Text(row.title)
                .font(.system(size: 14))
                .foregroundColor(Color(PanelTheme.ink))
                .lineLimit(1)
        }
        .frame(minHeight: family == .systemLarge ? 32 : 30, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityLabel("\(TypeMark.word(for: mark)), \(row.title)")
    }
}

/// Lock-screen face: pure entry, zero snippet data. The circle
/// carries the wordmark initial only and opens the search-first Home.
struct TypviaLockEntryWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "TypviaLockEntry", provider: RowsProvider()) { _ in
            Text("T")
                .font(.system(size: 20, weight: .medium, design: .monospaced))
                .widgetBackground(Color.clear)
                .widgetURL(deepLink("search"))
        }
        .configurationDisplayName("Typvia")
        .description(tr("Open Typvia search", "打开 Typvia 搜索"))
        .supportedFamilies([.accessoryCircular])
    }
}

extension View {
    /// iOS 17 requires containerBackground on widget roots; 16 keeps the
    /// plain background. One place, both paths.
    @ViewBuilder
    func widgetBackground(_ color: Color) -> some View {
        if #available(iOSApplicationExtension 17.0, *) {
            containerBackground(for: .widget) { color }
        } else {
            background(color)
        }
    }
}
