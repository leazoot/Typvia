// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import Foundation

/// What kind of file the reader picked.
///
/// The product does not guess where it cannot know. A `.md` or `.csv` file
/// says what it is; a `.json` file might be a Typvia backup, this product's
/// own export, a massCode dump or a CopyQ tab — so the backup is recognised by
/// the marker it actually carries, and the remaining three are **asked about**
/// rather than guessed at. A wrong guess imports somebody's library as
/// gibberish.
public enum PickedFile: Equatable, Sendable {
    /// A sealed Typvia backup: it needs a passphrase and an empty library.
    case backup
    /// A format the file's own name settles. The string is the core's
    /// vocabulary, never a word invented here.
    case known(format: String)
    /// A `.json` that is not a backup. Only the reader knows which product
    /// wrote it.
    case askWhichJson
    case unreadable

    /// The three a `.json` file could be, in the core's vocabulary.
    public static let jsonKinds = ["json", "masscode", "copyq"]

    /// - Parameters:
    ///   - name: the file's own name, as the picker reported it.
    ///   - head: the first part of the text — enough to carry the outer marker
    ///     of a sealed backup, which is written in the clear.
    public static func of(name: String, head: String) -> PickedFile {
        if head.contains("\"\(backupFormatMarker())\"") { return .backup }
        switch (name as NSString).pathExtension.lowercased() {
        case "md", "markdown", "txt": return .known(format: "markdown")
        case "csv": return .known(format: "csv")
        case "json": return .askWhichJson
        default: return .unreadable
        }
    }
}

/// Why an export, import or restore did not happen. Never the raw error.
public enum DataRefusal: Equatable, Sendable, CaseIterable {
    case libraryNotEmpty
    case didNotOpen
    case notAFileThisReads
    case tooBig
    case typedDifferently
    case storage

    public init(_ error: CoreError) {
        switch error {
        case .Conflict: self = .libraryNotEmpty
        case .PermissionDenied: self = .didNotOpen
        case .Validation: self = .notAFileThisReads
        default: self = .storage
        }
    }
}

/// What the data chapter says, as values.
///
/// The rule this holds: **a report says what happened to the library, and a
/// failure says what did not**. Import and restore are both all-or-nothing in
/// the core, so every sentence here can promise that much without hedging.
public enum DataChapterCopy {
    /// What one import did, including what it would not take.
    public static func imported(_ report: ImportReport, _ tr: Translator) -> String {
        var lines = [
            report.imported == 1
                ? tr("1 snippet came in.", "进来 1 枚片段。")
                : tr("\(report.imported) snippets came in.", "进来 \(report.imported) 枚片段。"),
        ]
        // Named, not counted: a reader told "3 conflicts" cannot go and look
        // at the three they already have.
        if !report.conflicts.isEmpty {
            let names = report.conflicts.joined(separator: ", ")
            let zhNames = report.conflicts.joined(separator: "、")
            lines.append(
                tr(
                    "These triggers are already yours, so those entries stayed out: \(names).",
                    "这些触发词你已经用了,对应的条目没有进来:\(zhNames)。"
                )
            )
        }
        if !report.skipped.isEmpty {
            let count = report.skipped.count
            lines.append(
                count == 1
                    ? tr("1 entry could not be read.", "有 1 条读不了。")
                    : tr("\(count) entries could not be read.", "有 \(count) 条读不了。")
            )
        }
        return lines.joined(separator: " ")
    }

    /// What a restore brought back. Counts only — this page opens no body.
    public static func restored(_ report: BackupRestored, _ tr: Translator) -> String {
        // Each noun decides its own plural; what joins them differs by
        // language, which is why the pieces are assembled and not formatted.
        let snippets = tr.counted(report.snippets, "snippet", "snippets", "\(report.snippets) 枚片段")
        let folders = tr.counted(report.folders, "folder", "folders", "\(report.folders) 个文件夹")
        let tags = tr.counted(report.tags, "tag", "tags", "\(report.tags) 个标签")
        let head = tr(
            "Back: \(snippets), \(folders), \(tags).",
            "回来了:\(snippets)、\(folders)、\(tags)。"
        )
        let vault = report.vaultRestored
            ? tr(
                "The vault came with it, and it opens with the master password it had.",
                "保险库也回来了,用它原来的主密码打开。"
            )
            : tr("There was no vault in that backup.", "那份备份里没有保险库。")
        return "\(head) \(vault)"
    }

    /// Why it did not happen — and, first, what is still true.
    public static func sentence(_ refusal: DataRefusal, _ tr: Translator) -> String {
        let said: String
        switch refusal {
        case .libraryNotEmpty:
            said = tr(
                "A backup can only be restored into an empty library, and this one already has snippets in it.",
                "备份只能恢复到一个空库里,而这个库里已经有片段了。"
            )
        case .didNotOpen:
            said = tr(
                "That backup did not open with that passphrase.",
                "这个口令打不开那份备份。"
            )
        case .notAFileThisReads:
            said = tr("Nothing here can read that file.", "这里读不了那种文件。")
        case .tooBig:
            said = tr(
                "That file is larger than this can take.",
                "那个文件超过了能处理的大小。"
            )
        case .typedDifferently:
            said = tr(
                "The two typings differ, so nothing was written.",
                "两次输入不一样,所以什么都没写。"
            )
        case .storage:
            said = tr("Something under the page failed.", "底下出了点问题。")
        }
        return "\(said) \(nothingChanged(tr))"
    }

    /// Said after every refusal on this page, because this is the one page
    /// where the reader's whole library is at stake.
    public static func nothingChanged(_ tr: Translator) -> String {
        tr("Your library is exactly as it was.", "你的库和刚才一模一样。")
    }

    /// A backup's file name. The millisecond stamp keeps every export its own
    /// file rather than quietly replacing the last one.
    public static func backupFileName(at moment: Date = Date()) -> String {
        "Typvia-backup-\(Int64(moment.timeIntervalSince1970 * 1000))"
    }
}
