// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import XCTest

@testable import TypviaKit

/// The data chapter: what a file is taken to be, and what the page promises
/// about the library after each outcome.
final class DataChapterTests: XCTestCase {
    private let tr = Translator(language: .en)

    // MARK: - What the file is

    /// A sealed backup is recognised by the marker it carries, not by being
    /// named `.json` — a backup the reader renamed is still a backup.
    func testABackupIsKnownByItsMarkerWhateverItIsCalled() {
        let sealed = #"{"format":"\#(backupFormatMarker())","version":1,"sealed":"…"}"#

        XCTAssertEqual(PickedFile.of(name: "library.json", head: sealed), .backup)
        XCTAssertEqual(PickedFile.of(name: "whatever.txt", head: sealed), .backup)
    }

    /// Three products write `.json`, and importing one as another turns a
    /// library into gibberish. Where the product cannot know, it asks.
    func testAJsonThatIsNotABackupIsAskedAboutRatherThanGuessedAt() {
        XCTAssertEqual(
            PickedFile.of(name: "snippets.json", head: #"[{"body":"x"}]"#),
            .askWhichJson
        )
        XCTAssertEqual(PickedFile.jsonKinds.count, 3)
    }

    func testAnExtensionThatSaysWhatItIsGoesStraightThrough() {
        XCTAssertEqual(PickedFile.of(name: "notes.MD", head: "# hi"), .known(format: "markdown"))
        XCTAssertEqual(PickedFile.of(name: "rows.csv", head: "a,b"), .known(format: "csv"))
        XCTAssertEqual(PickedFile.of(name: "photo.png", head: ""), .unreadable)
    }

    // MARK: - What the page says afterwards

    /// Conflicts are named, not counted: a reader told "3 conflicts" cannot go
    /// and look at the three they already have.
    func testAnImportReportNamesTheTriggersThatStayedOut() {
        let report = ImportReport(
            imported: 2,
            conflicts: [";ssh", ";deploy"],
            skipped: [ImportSkipped(label: "row 4", reason: "empty")]
        )

        let said = DataChapterCopy.imported(report, tr)

        XCTAssertTrue(said.hasPrefix("2 snippets came in."))
        XCTAssertTrue(said.contains(";ssh, ;deploy"))
        XCTAssertTrue(said.contains("1 entry could not be read"))
    }

    /// English has a plural and one snippet is not "1 snippets".
    func testOneSnippetIsSaidAsOne() {
        let one = ImportReport(imported: 1, conflicts: [], skipped: [])

        XCTAssertEqual(DataChapterCopy.imported(one, tr), "1 snippet came in.")
    }

    /// Every noun in the restore line decides its own plural.
    func testARestoreOfOneOfEachThingSaysEachOfThemInTheSingular() {
        let one = BackupRestored(
            snippets: 1, folders: 1, tags: 1, versions: 1, vaultRestored: false
        )

        XCTAssertTrue(
            DataChapterCopy.restored(one, tr).hasPrefix("Back: 1 snippet, 1 folder, 1 tag."),
            "each count decides its own plural"
        )
    }

    /// A restore either brought the vault or it did not, and silence about it
    /// leaves the reader wondering where their secrets went.
    func testARestoreSaysWhetherTheVaultCameWithIt() {
        let withVault = BackupRestored(
            snippets: 12, folders: 2, tags: 3, versions: 40, vaultRestored: true
        )
        XCTAssertTrue(DataChapterCopy.restored(withVault, tr).contains("The vault came with it"))

        let without = BackupRestored(
            snippets: 12, folders: 2, tags: 3, versions: 40, vaultRestored: false
        )
        XCTAssertTrue(DataChapterCopy.restored(without, tr).contains("no vault in that backup"))
    }

    /// This is the page where a reader's whole library is at stake, so every
    /// refusal on it ends by saying the library is untouched.
    func testEveryRefusalEndsBySayingTheLibraryIsUnchanged() {
        for refusal in DataRefusal.allCases {
            XCTAssertTrue(
                DataChapterCopy.sentence(refusal, tr).hasSuffix(DataChapterCopy.nothingChanged(tr)),
                "\(refusal) does not say what is still true"
            )
        }
    }

    /// Every export is its own file: a name that repeated would quietly
    /// replace the backup made a minute ago.
    func testEachExportGetsItsOwnFileName() {
        let first = DataChapterCopy.backupFileName(at: Date(timeIntervalSince1970: 1))
        let second = DataChapterCopy.backupFileName(at: Date(timeIntervalSince1970: 2))

        XCTAssertNotEqual(first, second)
        XCTAssertTrue(first.hasPrefix("Typvia-backup-"))
    }
}
