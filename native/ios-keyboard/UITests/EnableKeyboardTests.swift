// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// SPIKE: UI test that enables the TypviaKb keyboard in Settings,
// switches to it in the host app, and asserts the snapshot snippet got
// inserted. Driving the UI from inside the simulator avoids host-side mouse
// synthesis entirely.

import XCTest

final class EnableKeyboardTests: XCTestCase {
    /// iOS 18 Settings is SwiftUI; rows surface as buttons/staticTexts rather
    /// than cells, so match any tappable descendant by label.
    func row(_ app: XCUIApplication, _ labels: [String]) -> XCUIElement {
        let clauses = labels.map { "label CONTAINS '\($0)'" }.joined(separator: " OR ")
        return app.descendants(matching: .any).matching(
            NSPredicate(format: "(elementType == 9 OR elementType == 48 OR elementType == 75) AND (\(clauses))")
        ).firstMatch
    }

    func testEnableKeyboardAndInsertSnippet() {
        let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
        settings.launch()

        let general = row(settings, ["通用", "General"])
        XCTAssertTrue(general.waitForExistence(timeout: 10), "General row")
        general.tap()

        let keyboard = row(settings, ["键盘", "Keyboard"])
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10), "Keyboard row")
        keyboard.tap()

        // The "键盘 N" row is the first list row on the Keyboard page; its
        // element type varies across iOS versions, so tap by normalized
        // coordinate (just below the nav bar) instead of an element query.
        sleep(1)
        settings.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.16)).tap()

        // Already enabled from an earlier run?
        let alreadyEnabled = settings.staticTexts["TypviaKb"].firstMatch
        if !alreadyEnabled.waitForExistence(timeout: 3) {
            let add = row(settings, ["添加新键盘", "Add New Keyboard"])
            XCTAssertTrue(add.waitForExistence(timeout: 10), "Add New Keyboard row")
            add.tap()

            let typvia = row(settings, ["TypviaKb"])
            XCTAssertTrue(typvia.waitForExistence(timeout: 10), "TypviaKb entry")
            typvia.tap()
        }

        // Host app: focus lands in the text view automatically.
        let host = XCUIApplication(bundleIdentifier: "dev.typvia.spike.kbhost")
        host.launch()
        let textView = host.textViews["hostTextView"]
        XCTAssertTrue(textView.waitForExistence(timeout: 10), "host text view")
        sleep(2)

        // Switch to TypviaKb via the globe key if a system keyboard is up.
        let next = host.buttons.matching(
            NSPredicate(
                format: "label CONTAINS 'Next keyboard' OR label CONTAINS '下一个键盘' OR identifier == 'Next keyboard'"
            )
        ).firstMatch
        if next.waitForExistence(timeout: 5) {
            next.press(forDuration: 1.2)
            let entry = host.tables.staticTexts["TypviaKb"].firstMatch
            if entry.waitForExistence(timeout: 5) {
                entry.tap()
            }
        }

        // The spike keyboard auto-inserts the first snippet ~2s after load.
        let inserted = NSPredicate(format: "value CONTAINS 'Yesterday'")
        expectation(for: inserted, evaluatedWith: textView)
        waitForExpectations(timeout: 20)

        // Hold the final state so an external screenshot can capture it.
        sleep(5)
    }
}
