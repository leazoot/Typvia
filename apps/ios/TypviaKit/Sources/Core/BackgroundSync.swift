// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import BackgroundTasks
import Foundation

/// Syncing while the app is not on screen.
///
/// There is no second sync orchestration here: the background path runs the
/// same round the foreground does, which is the whole point — a second
/// implementation would be a second set of bugs, and the one place they would
/// show is the place nobody is watching.
public enum BackgroundSync {
    /// The identifier the app declares. It has to match the one in the
    /// Info.plist exactly; the system refuses a registration that does not.
    public static let taskId = "dev.typvia.mobile.refresh"

    /// How long to ask the system to wait before the next attempt.
    ///
    /// It is a request, not a schedule: iOS decides when — or whether — a
    /// background refresh actually runs, and a product that promises "every
    /// fifteen minutes" is promising something it does not control.
    public static let interval: TimeInterval = 15 * 60

    /// Whether a round is worth asking for at all. Sync that was never set up
    /// has nothing to send, and waking for it would spend a budget the system
    /// gives sparingly.
    public static func shouldSchedule(isConfigured: Bool, isEnabled: Bool) -> Bool {
        isConfigured && isEnabled
    }

    /// Asks for the next wake-up.
    public static func schedule(now: Date = Date()) {
        let request = BGAppRefreshTaskRequest(identifier: taskId)
        request.earliestBeginDate = now.addingTimeInterval(interval)
        // A refused submission is not worth reporting to anybody: the system
        // declines for reasons the reader cannot act on (budget, low power,
        // a simulator with no background scheduling at all).
        try? BGTaskScheduler.shared.submit(request)
    }

    /// Registers the handler. Called once, before the app finishes launching —
    /// the system requires it that early and throws otherwise.
    /// - Parameter afterRound: what the host does once a round has landed
    ///   rows. This framework is linked into the keyboard as well, and the
    ///   keyboard has no business linking WidgetKit to redraw a widget it
    ///   cannot see — so telling the system belongs to the app, not here.
    public static func register(
        store: @escaping @Sendable () -> TypviaStore?,
        afterRound: (@Sendable () -> Void)? = nil
    ) {
        BGTaskScheduler.shared.register(
            forTaskWithIdentifier: taskId, using: nil
        ) { task in
            handle(task, store: store(), afterRound: afterRound)
        }
    }

    static func handle(
        _ task: BGTask,
        store: TypviaStore?,
        afterRound: (@Sendable () -> Void)? = nil
    ) {
        // The next request goes in first. If this round is killed for running
        // long, there is still a wake-up booked; asking afterwards would mean
        // a timeout quietly ends all future syncing.
        schedule()

        guard let store else { return task.setTaskCompleted(success: false) }

        let work = Task {
            let ok = (try? await store.perform { try $0.syncNow() }) != nil
            // A round that brought rows in changed what the keyboard should be
            // able to reach. Nobody will be looking at the app when this runs,
            // so if the document is not rewritten here it stays as it was until
            // the reader next opens the app — and the keyboard goes on offering
            // the library as it stood before the sync.
            if ok, await SnapshotPublish.run(store: store) { afterRound?() }
            task.setTaskCompleted(success: ok)
        }
        // The system's warning that time is up. Cancelling leaves the round
        // half-done in the outbox, which is where it is designed to survive.
        task.expirationHandler = { work.cancel() }
    }
}
