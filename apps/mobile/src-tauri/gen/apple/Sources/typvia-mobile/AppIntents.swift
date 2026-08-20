// App Intents entry points: pure entry, zero content. The intent
// carries no snippet data and takes no parameters — running it opens the
// app, whose first screen is the search surface ("open" is "search").
// Parameterized deep links need a URL-to-route channel and stay out of v1.

import AppIntents

struct OpenTypviaIntent: AppIntent {
  static let title: LocalizedStringResource = "Open Typvia"
  static let description = IntentDescription("Opens Typvia at the snippet search screen.")
  static let openAppWhenRun: Bool = true

  @MainActor
  func perform() async throws -> some IntentResult {
    .result()
  }
}

struct TypviaShortcuts: AppShortcutsProvider {
  static var appShortcuts: [AppShortcut] {
    AppShortcut(
      intent: OpenTypviaIntent(),
      phrases: [
        "Open \(.applicationName)",
        "Search \(.applicationName)",
      ],
      shortTitle: "Open Typvia",
      systemImageName: "character.cursor.ibeam"
    )
  }
}
