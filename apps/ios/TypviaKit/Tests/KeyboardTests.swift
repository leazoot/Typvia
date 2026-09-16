// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

@MainActor
final class KeyboardTests: XCTestCase {
    /// Three entries, one of them locked, two named as recent.
    private func snapshot() -> String {
        """
        {"snapshot_version":1,"generated_at":1700000000000,"device_id":"device-1",
        "snippets":[
        {"id":"s1","title":"Docker logs","snippet_type":"command","trigger":";dlog",
        "trigger_mode":"delimiter","folder_id":null,"is_favorite":false,
        "body":"docker logs -f app"},
        {"id":"s2","title":"Standup","snippet_type":"text","trigger":null,
        "trigger_mode":null,"folder_id":null,"is_favorite":true,
        "body":"Yesterday I shipped"},
        {"id":"s9","encrypted_metadata":[1,2,3]}
        ],
        "recent_ids":["s2","s1"],"favorite_ids":["s2"],"folder_metadata":[]}
        """
    }

    /// The bench prints "how many I can reach out of how many I have", and the
    /// second number is the snapshot's entry count. Taking it from the recent
    /// list is how this row once printed "3 / 1".
    func testTheBenchCountsWhatTheSnapshotCarriesNotWhatIsRecent() {
        let model = KeyboardModel()

        model.apply(snapshot())

        XCTAssertEqual(model.reach.total, 3)
        XCTAssertEqual(model.reach.available, 3)
        XCTAssertTrue(model.reach.isComplete)
    }

    /// Reach is what the bench is holding, not what the query hit. Filtering
    /// narrows the list; the row's own match count says how many — and if
    /// filtering moved the reach as well, the panel would report the bench
    /// shrinking every time somebody typed.
    func testFilteringNarrowsTheListWithoutShrinkingTheBench() {
        let model = KeyboardModel()
        model.apply(snapshot())

        model.query = ";dlog"

        XCTAssertEqual(model.tiles.map(\.title), ["Docker logs"])
        XCTAssertEqual(model.reach, KeyboardReach(available: 3, total: 3))
    }

    /// No snapshot is not an error to report: the keyboard says what it can
    /// reach, which is nothing, and offers the way to fix it.
    func testWithoutASnapshotTheBenchSaysNothingIsReachable() {
        let model = KeyboardModel()

        model.apply(nil)

        XCTAssertEqual(model.reach.total, 0)
        XCTAssertEqual(model.reach.available, 0)
        XCTAssertTrue(model.tiles.isEmpty)
        XCTAssertEqual(model.phase, .limited(available: 0, total: 0))
    }
}

/// The undo bar's own clock.
///
/// Its life was written down as a number and never acted on, so the bar sat on
/// top of the tile run until the reader typed something to get their bench
/// back. What is asserted here is the number itself and the rule around it —
/// the timing belongs to a running keyboard, which the walkthrough owns.
final class InsertedNoteTests: XCTestCase {
    func testTheUndoBarHasAShortLifeRatherThanNone() {
        XCTAssertGreaterThan(InsertedNote.life, 0)
        // Long enough to read and reach, short enough that it is not furniture:
        // the bar covers the tiles while it is there.
        XCTAssertLessThanOrEqual(InsertedNote.life, 6)
    }
}

/// How a body goes into somebody else's text field.
///
/// The product types rather than pastes, which is a promise about what the
/// host app sees: a sequence of insertions, at reading speed, that the reader
/// can stop. What is tested here is the arithmetic behind that — the view
/// itself types into a proxy only a running extension has.
final class TypeInTests: XCTestCase {
    func testAShortBodyIsTypedAtTheDeliverysRate() {
        guard case let .character(interval) = TypeIn.plan(characterCount: 4) else {
            return XCTFail("four characters are worth typing")
        }
        XCTAssertEqual(interval, Tokens.Motion.typeInPerCharacter, accuracy: 0.0001)
    }

    /// The cap is on the whole insertion, not on each character: a longer body
    /// types faster rather than taking longer.
    func testALongerBodyStillFinishesWithinTheCap() {
        let count = 60
        guard case let .character(interval) = TypeIn.plan(characterCount: count) else {
            return XCTFail("sixty characters are still worth typing")
        }
        XCTAssertLessThan(interval, Tokens.Motion.typeInPerCharacter)
        XCTAssertEqual(
            interval * Double(count), Tokens.Motion.typeInCap, accuracy: 0.0001
        )
    }

    /// Past the point where the motion could be seen, the body simply lands.
    /// Hundreds of edits to a host app's field to animate something invisible
    /// is a cost with nothing on the other side of it.
    func testAVeryLongBodyLandsAtOnce() {
        XCTAssertEqual(TypeIn.plan(characterCount: 5_000), .atOnce)
    }

    func testASingleCharacterIsNotAnAnimation() {
        XCTAssertEqual(TypeIn.plan(characterCount: 1), .atOnce)
        XCTAssertEqual(TypeIn.plan(characterCount: 0), .atOnce)
    }
}
