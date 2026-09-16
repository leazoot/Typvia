// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation
import SwiftUI

/// The two things a reader can set about how the product looks.
///
/// They are stored in the App Group rather than in the app's own defaults, so
/// a surface that is not the app can read them — a keyboard that ignores the
/// app's language setting is the mixed-language screen the design forbids,
/// arriving through the back door.
///
/// Storing them there is not the same as reading them there. Each surface has
/// to ask, at the moment it appears: the keyboard in its controller, the share
/// sheet as it builds its host, the widget as it builds a timeline entry. This
/// comment once claimed all of them while only the app read anything, which is
/// how a promise ends up in a file nobody rereads.
///
/// Nothing else lives here. These are UI preferences; anything about snippets
/// belongs to the database.
public enum UiPreference {
    /// Which language the interface is rendered in.
    public enum Language: String, CaseIterable, Sendable {
        /// Whatever the phone is set to.
        case system
        case en
        case zh

        /// Resolves to the one that actually gets rendered.
        public func resolved(preferred: [String] = Locale.preferredLanguages) -> UiLanguage {
            switch self {
            case .system: UiLanguage.resolve(preferred: preferred)
            case .en: .en
            case .zh: .zh
            }
        }
    }

    /// Light, dark, or the phone's own.
    public enum Appearance: String, CaseIterable, Sendable {
        case system
        case light
        case dark

        public var colorScheme: ColorScheme? {
            switch self {
            case .system: nil
            case .light: .light
            case .dark: .dark
            }
        }
    }
}

/// Reads and writes the two preferences.
///
/// An unset preference is `system`, and an unreadable store is also `system` —
/// the product works without ever being configured, and a failure to read a
/// preference must not stop it from starting.
@MainActor
public final class UiPreferences: ObservableObject {
    @Published public var language: UiPreference.Language {
        didSet { write(Keys.language, language.rawValue) }
    }

    @Published public var appearance: UiPreference.Appearance {
        didSet { write(Keys.appearance, appearance.rawValue) }
    }

    private let defaults: UserDefaults?

    public init(defaults: UserDefaults? = UserDefaults(suiteName: SharedContainer.appGroup)) {
        self.defaults = defaults
        let stored = UiPreferences.chosen(defaults: defaults)
        language = stored.language
        appearance = stored.appearance
    }

    private func write(_ key: String, _ value: String) {
        defaults?.set(value, forKey: key)
    }

    /// Reads the store again.
    ///
    /// For the surfaces that are not the app: an extension is a process of its
    /// own and is not told when the reader chooses something, so it has to ask
    /// again at the moment it comes up. Without this a keyboard built once
    /// keeps the language it was built with until the system happens to end
    /// the process.
    public func reload() {
        let stored = UiPreferences.chosen(defaults: defaults)
        if stored.language != language { language = stored.language }
        if stored.appearance != appearance { appearance = stored.appearance }
    }

    /// The two choices as they stand, read without an object to observe them.
    ///
    /// A widget is drawn from a value in a process that has no moment to hold
    /// an observable and nothing to observe it from; a share sheet builds its
    /// host once and is gone. They read here, so the parse of an unknown or
    /// missing value exists once rather than once per surface.
    public nonisolated static func chosen(
        defaults: UserDefaults? = UserDefaults(suiteName: SharedContainer.appGroup)
    ) -> Chosen {
        Chosen(
            language: UiPreference.Language(
                rawValue: defaults?.string(forKey: Keys.language) ?? ""
            ) ?? .system,
            appearance: UiPreference.Appearance(
                rawValue: defaults?.string(forKey: Keys.appearance) ?? ""
            ) ?? .system
        )
    }

    /// Both preferences as one value.
    public struct Chosen: Equatable, Sendable {
        public let language: UiPreference.Language
        public let appearance: UiPreference.Appearance

        public init(
            language: UiPreference.Language = .system,
            appearance: UiPreference.Appearance = .system
        ) {
            self.language = language
            self.appearance = appearance
        }
    }

    /// The keys are namespaced the way the desktop's are, so the two products
    /// read as one when someone goes looking.
    enum Keys {
        static let language = "tv.ui.locale"
        static let appearance = "tv.ui.theme"
    }
}

extension View {
    /// Applies the reader's choices to a whole tree: the language every copy
    /// pair renders in, and the appearance the paper takes.
    @MainActor
    public func uiPreferences(_ preferences: UiPreferences) -> some View {
        uiPreferences(
            UiPreferences.Chosen(
                language: preferences.language,
                appearance: preferences.appearance
            )
        )
    }

    /// The same, from a value rather than from something to observe — which is
    /// all a widget's face has.
    public func uiPreferences(_ chosen: UiPreferences.Chosen) -> some View {
        environment(\.tr, Translator(language: chosen.language.resolved()))
            .modifier(ChosenAppearance(appearance: chosen.appearance))
    }
}

/// The chosen appearance, applied at the two layers that read it.
///
/// The surfaces do not agree on which lever they honour: `preferredColorScheme`
/// reaches the window a screen sits in and does nothing in a widget, while the
/// environment value is what a dynamic colour resolves against. One choice, set
/// in both places — and set in neither when the choice is to follow the phone,
/// because an environment pinned to one scheme is not "system", it is that
/// scheme.
private struct ChosenAppearance: ViewModifier {
    let appearance: UiPreference.Appearance

    @ViewBuilder
    func body(content: Content) -> some View {
        if let scheme = appearance.colorScheme {
            content
                .environment(\.colorScheme, scheme)
                .preferredColorScheme(scheme)
        } else {
            content
        }
    }
}
