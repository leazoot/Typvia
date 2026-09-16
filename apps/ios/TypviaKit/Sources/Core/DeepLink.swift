// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// Where a `typvia://` link points.
///
/// Everything that is not one of these is nothing. A link is the one channel
/// another app can aim at this one, so what it is allowed to ask for is a
/// short closed list, and anything outside it lands on the home screen rather
/// than being interpreted generously.
public enum DeepLink: Equatable, Sendable {
    case search(String)
    case snippet(String)
    case home

    public static let scheme = "typvia"

    /// Parses a URL, or decides it is not for us.
    ///
    /// - Returns: nil when the URL belongs to somebody else. A malformed
    ///   Typvia link resolves to `home` instead — it was aimed here, so the
    ///   reader ends up somewhere real rather than nowhere.
    public static func parse(_ url: URL) -> DeepLink? {
        guard url.scheme?.lowercased() == scheme else { return nil }
        let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
        switch url.host?.lowercased() {
        case "search":
            let query = components?.queryItems?.first { $0.name == "q" }?.value ?? ""
            return .search(query)
        case "snippet":
            // The id is whatever the path says; whether it exists is the
            // core's answer, asked later. An empty one is not an id.
            let id = url.pathComponents.dropFirst().first ?? ""
            return id.isEmpty ? .home : .snippet(String(id))
        case "home", .none:
            return .home
        default:
            // A host this version does not know is not an error to show; it is
            // a link from a newer build, and home is where it lands.
            return .home
        }
    }
}

/// What to do with a link that arrives before the reader is ready for it.
///
/// The order is the product's: the first run comes first, a link that arrives
/// during it waits, and it is honoured once the reader is through. Dropping it
/// would lose the thing they tapped; jumping to it would strand them in a
/// product they have not been introduced to.
@MainActor
public final class DeepLinkRouter: ObservableObject {
    /// The link to act on right now, if any.
    @Published public private(set) var pending: DeepLink?

    private var deferred: DeepLink?
    private var isOnboarding: Bool

    public init(isOnboarding: Bool) {
        self.isOnboarding = isOnboarding
    }

    public func open(_ url: URL) {
        guard let link = DeepLink.parse(url) else { return }
        receive(link)
    }

    func receive(_ link: DeepLink) {
        if isOnboarding {
            // Only the most recent one is kept: two taps before the app is
            // ready mean the reader wants the second thing.
            deferred = link
            return
        }
        pending = link
    }

    /// The first run finished. Whatever was waiting is honoured now.
    public func onboardingFinished() {
        isOnboarding = false
        if let deferred {
            pending = deferred
            self.deferred = nil
        }
    }

    /// The screen has acted on it.
    public func clear() {
        pending = nil
    }
}
