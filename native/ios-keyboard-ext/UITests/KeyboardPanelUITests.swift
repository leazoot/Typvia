// XCUITest closing the loop for the formal keyboard: keyboard enablement is
// not programmable, so the test drives the Settings app inside the simulator,
// switches to the Typvia keyboard in a native host text view, taps a snippet
// row, and asserts the inserted text. Rows rendering at all is the
// fullAccess=false snapshot-readability proof: RequestsOpenAccess is false
// in the extension's Info.plist, so iOS never grants it full access, and the
// row data can only have come from the App Group snapshot file.

import XCTest

final class KeyboardPanelUITests: XCTestCase {
    private let keyboardName = "Typvia Keyboard"

    /// iOS 18 Settings is SwiftUI; rows surface as buttons/staticTexts/cells
    /// depending on the screen, so match any tappable descendant by label.
    private func row(_ app: XCUIApplication, _ labels: [String]) -> XCUIElement {
        let clauses = labels.map { "label CONTAINS '\($0)'" }.joined(separator: " OR ")
        return app.descendants(matching: .any).matching(
            NSPredicate(
                format: "(elementType == 9 OR elementType == 48 OR elementType == 75) AND (\(clauses))")
        ).firstMatch
    }

    private func enableKeyboardInSettings() {
        let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
        settings.launch()

        let general = row(settings, ["通用", "General"])
        XCTAssertTrue(general.waitForExistence(timeout: 10), "General row")
        general.tap()

        let keyboard = row(settings, ["键盘", "Keyboard"])
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10), "Keyboard row")
        keyboard.tap()

        // The "Keyboards N" row is the first list row on the Keyboard page;
        // its element type varies across iOS versions, so tap by normalized
        // coordinate (just below the nav bar), as validated by the spike.
        sleep(1)
        settings.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.16)).tap()

        // Already enabled from an earlier run? The enabled row reads
        // "Typvia Keyboard — KbTestHost", so match by contains.
        let alreadyEnabled = row(settings, [keyboardName])
        if !alreadyEnabled.waitForExistence(timeout: 3) {
            let add = row(settings, ["添加新键盘", "Add New Keyboard"])
            XCTAssertTrue(add.waitForExistence(timeout: 10), "Add New Keyboard row")
            add.tap()

            // Third-party keyboards list under their container app's
            // display name; the harness host is KbTestHost.
            let typvia = row(settings, [keyboardName, "KbTestHost"])
            XCTAssertTrue(typvia.waitForExistence(timeout: 10), "Typvia keyboard entry")
            typvia.tap()

            // Some iOS 18 builds open a per-app page with a toggle instead
            // of adding on tap; flip it when present.
            let toggle = settings.switches.firstMatch
            if toggle.waitForExistence(timeout: 3) {
                if (toggle.value as? String) == "0" { toggle.tap() }
                let done = settings.buttons["Done"].firstMatch
                if done.waitForExistence(timeout: 2) { done.tap() }
            }

            // Do not leave Settings until the keyboard is really listed;
            // terminating early can race the preference write.
            let enabled = row(settings, [keyboardName, "KbTestHost"])
            XCTAssertTrue(enabled.waitForExistence(timeout: 10), "keyboard listed after add")
        }
        settings.terminate()
    }

    private func switchToTypviaKeyboard(in host: XCUIApplication) {
        // If a system keyboard is up, hold the globe key and pick Typvia
        // from the input-mode list (which may surface under the host or
        // under springboard depending on the iOS build).
        let next = host.buttons.matching(
            NSPredicate(
                format: "label CONTAINS 'Next keyboard' OR label CONTAINS '下一个键盘' OR identifier == 'Next keyboard'"
            )
        ).firstMatch
        if next.waitForExistence(timeout: 5) {
            next.press(forDuration: 1.2)
            let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
            let byName = NSPredicate(format: "label CONTAINS %@", keyboardName)
            for app in [host, springboard] {
                let entry = app.staticTexts.matching(byName).firstMatch
                if entry.waitForExistence(timeout: 3) {
                    entry.tap()
                    return
                }
            }
        }
    }

    /// The keyboard extension's elements can attach under the host app or
    /// under springboard; check both.
    private func panelElement(_ host: XCUIApplication, _ identifier: String) -> XCUIElement {
        let inHost = host.descendants(matching: .any).matching(identifier: identifier).firstMatch
        if inHost.exists { return inHost }
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let inBoard =
            springboard.descendants(matching: .any).matching(identifier: identifier).firstMatch
        return inBoard.exists ? inBoard : inHost
    }

    func testEnableKeyboardReadSnapshotWithoutFullAccessAndInsert() {
        // Launch the host first: this installs it (registering the embedded
        // keyboard with the system) and writes the snapshot fixture into
        // the shared App Group container; the keyboard process (a different
        // process, no full access) must read it back through the group.
        let host = XCUIApplication()
        host.launchArguments = ["--seed-snapshot"]
        host.launch()
        var textView = host.textViews["hostTextView"]
        XCTAssertTrue(textView.waitForExistence(timeout: 10), "host text view")

        enableKeyboardInSettings()

        host.activate()
        textView = host.textViews["hostTextView"]
        XCTAssertTrue(textView.waitForExistence(timeout: 10), "host text view again")
        textView.tap()
        sleep(2)

        switchToTypviaKeyboard(in: host)

        // Panel up and populated from the snapshot = fullAccess=false read
        // proof. s2 (Docker logs) is the recent entry, so it lists first.
        let recentRow = panelElement(host, "kbRow-s2")
        XCTAssertTrue(recentRow.waitForExistence(timeout: 10), "recent snippet row")

        // Tap the multiline snippet and assert the text landed in the host.
        let standupRow = panelElement(host, "kbRow-s1")
        XCTAssertTrue(standupRow.waitForExistence(timeout: 5), "standup row")
        standupRow.tap()
        let inserted = NSPredicate(format: "value CONTAINS 'No blockers'")
        expectation(for: inserted, evaluatedWith: textView)
        waitForExpectations(timeout: 10)

        // Sensitive red line: the locked row exists, shows no plaintext,
        // and tapping it inserts nothing.
        let lockedRow = panelElement(host, "kbRow-s9")
        XCTAssertTrue(lockedRow.waitForExistence(timeout: 5), "locked vault row")
        XCTAssertTrue(
            lockedRow.label.contains("Locked"),
            "locked row must show the generic label, got: \(lockedRow.label)")
        let before = textView.value as? String ?? ""
        lockedRow.tap()
        sleep(1)
        XCTAssertEqual(textView.value as? String ?? "", before, "locked row must not insert")

        // Hold the panel on screen so an external phys_footprint sample can
        // be taken while the keyboard is up (memory red line ~60MB).
        sleep(8)
    }
}
