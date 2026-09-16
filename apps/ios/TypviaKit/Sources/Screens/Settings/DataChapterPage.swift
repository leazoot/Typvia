// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import SwiftUI
import UniformTypeIdentifiers

/// The library's way out and its way back in.
///
/// The chapter used to state that this product exports, imports and deletes —
/// and did none of the three. Everything on this page is about the whole
/// library at once, so every sentence says what happened to the whole library,
/// and every refusal says, first, that nothing changed.
struct DataInAndOut: View {
    @Environment(\.tr) private var tr
    @ObservedObject var model: SettingsModel

    @State private var passphrase = ""
    @State private var again = ""
    @State private var restorePassphrase = ""
    @State private var document: BackupFile?
    @State private var isExporting = false
    @State private var isPicking = false
    @State private var picked: PickedFile?
    @State private var pickedName: String?
    @State private var pickedText: String?
    @State private var jsonKind: String?
    @State private var isWorking = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // One clause. Where the file goes and what seals it are answered
            // by the fields below before the reader could wonder.
            Text(tr("A backup is one file, sealed with a passphrase.", "备份是一个文件,用一句口令封好。"))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink2)
                .fixedSize(horizontal: false, vertical: true)

            takeOut
            bringIn
        }
        .fileExporter(
            isPresented: $isExporting,
            document: document,
            contentType: .json,
            defaultFilename: DataChapterCopy.backupFileName()
        ) { result in
            document = nil
            switch result {
            case .success: model.exportLanded(tr)
            // A cancelled save is not a failure and says nothing; a failed one
            // is a file that does not exist, and the page must not imply it
            // does.
            case .failure: model.refuse(.storage)
            }
        }
        .fileImporter(
            isPresented: $isPicking,
            allowedContentTypes: [.json, .plainText, .commaSeparatedText, .data]
        ) { result in
            guard case let .success(url) = result else { return }
            take(url)
        }
    }

    private var takeOut: some View {
        VStack(alignment: .leading, spacing: 0) {
            PaperLabel(tr("TAKE EVERYTHING OUT", "把全部带走"))
            PaperField(
                label: tr("BACKUP PASSPHRASE", "备份口令"),
                text: $passphrase,
                isSecret: true
            )
            PaperField(label: tr("AGAIN", "再来一次"), text: $again, isSecret: true)
            Text(
                // The reason it is asked twice, said rather than assumed:
                // nothing keeps this passphrase, so there is no later moment
                // at which a typo could be caught.
                tr(
                    "Typed twice because nothing keeps it. A backup whose passphrase went in wrong cannot be opened by anyone, including you.",
                    "要输两次,因为没有任何地方存着它。口令输错的备份,谁也打不开,包括你自己。"
                )
            )
            .typviaType(.caption)
            .foregroundStyle(Paper.ink3)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.lineInnerGap)
            StickVerb(
                title: isWorking
                    ? tr("Working…", "正在处理…")
                    : tr("Export everything", "导出全部"),
                isPrimary: true
            ) {
                Task { await exportEverything() }
            }
            .disabled(passphrase.isEmpty || isWorking)
            .opacity(passphrase.isEmpty || isWorking ? DataMetrics.disabled : 1)
            .padding(.top, SettingsMetrics.actionGap)
        }
        .padding(.top, SettingsMetrics.paragraphGap)
    }

    @ViewBuilder
    private var bringIn: some View {
        VStack(alignment: .leading, spacing: 0) {
            PaperLabel(tr("BRING SOMETHING IN", "把东西带进来"))
            if let pickedName {
                Text(pickedName)
                    .typviaType(.mono)
                    .foregroundStyle(Paper.ink)
                    .padding(.top, SettingsMetrics.lineInnerGap)
            }
            switch picked {
            case .none, .unreadable:
                Text(
                    tr(
                        "A Typvia backup, or a Markdown, CSV or JSON file of snippets.",
                        "一份 Typvia 备份,或者 Markdown、CSV、JSON 格式的片段文件。"
                    )
                )
                .typviaType(.caption)
                .foregroundStyle(Paper.ink3)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, SettingsMetrics.lineInnerGap)
            case .backup:
                restoreForm
            case let .known(format):
                StickVerb(title: tr("Bring these in", "把它们带进来"), isPrimary: true) {
                    Task { await bringIn(format: format) }
                }
                .disabled(isWorking)
                .padding(.top, SettingsMetrics.actionGap)
            case .askWhichJson:
                whichJson
            }
            StickVerb(
                title: picked == nil
                    ? tr("Choose a file", "选一个文件")
                    : tr("Choose a different file", "换一个文件")
            ) {
                model.clearDataReport()
                isPicking = true
            }
            .padding(.top, SettingsMetrics.actionGap)
            report
        }
        .padding(.top, SettingsMetrics.paragraphGap)
    }

    private var restoreForm: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(
                tr(
                    "That is a sealed backup. Restoring puts a whole library back, so it only goes into an empty one — nothing is merged and nothing is overwritten.",
                    "那是一份封好的备份。恢复是把一整个库放回来,所以它只能放进一个空库——不合并,也不覆盖。"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.lineInnerGap)
            PaperField(
                label: tr("ITS PASSPHRASE", "它的口令"),
                text: $restorePassphrase,
                isSecret: true
            )
            StickVerb(title: tr("Put it all back", "全部放回来"), isPrimary: true) {
                Task { await putItAllBack() }
            }
            .disabled(restorePassphrase.isEmpty || isWorking)
            .opacity(restorePassphrase.isEmpty || isWorking ? DataMetrics.disabled : 1)
            .padding(.top, SettingsMetrics.actionGap)
        }
    }

    /// Asked, not guessed: three products write `.json`, and importing one as
    /// another turns a library into gibberish.
    private var whichJson: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(
                tr(
                    "Three things write a .json file. Which one made this?",
                    "有三种东西都写 .json。这个是哪一种?"
                )
            )
            .typviaType(.bodyS)
            .foregroundStyle(Paper.ink2)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.top, SettingsMetrics.lineInnerGap)
            HStack(spacing: SettingsMetrics.actionGap) {
                ForEach(PickedFile.jsonKinds, id: \.self) { kind in
                    Button { jsonKind = kind } label: {
                        Text(name(of: kind))
                            .typviaType(jsonKind == kind ? .sectionTitle : .bodyS)
                            .foregroundStyle(jsonKind == kind ? Paper.ink : Paper.ink3)
                            .frame(minHeight: Tokens.Hit.minimum)
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(jsonKind == kind ? [.isSelected] : [])
                }
            }
            StickVerb(title: tr("Bring these in", "把它们带进来"), isPrimary: true) {
                guard let jsonKind else { return }
                Task { await bringIn(format: jsonKind) }
            }
            .disabled(jsonKind == nil || isWorking)
            .opacity(jsonKind == nil || isWorking ? DataMetrics.disabled : 1)
            .padding(.top, SettingsMetrics.actionGap)
        }
    }

    /// What happened, said where the thing that happened is.
    @ViewBuilder
    private var report: some View {
        if let said = model.dataSaid {
            Text(said)
                .typviaType(.bodyS)
                .foregroundStyle(Paper.ink)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, SettingsMetrics.rowGap)
        }
        if let refusal = model.dataRefusal {
            Text(DataChapterCopy.sentence(refusal, tr))
                .typviaType(.bodyS)
                .foregroundStyle(Paper.attention)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, SettingsMetrics.rowGap)
        }
    }

    private func name(of kind: String) -> String {
        switch kind {
        case "masscode": "massCode"
        case "copyq": "CopyQ"
        default: "Typvia"
        }
    }

    private func exportEverything() async {
        guard passphrase == again else {
            model.refuse(.typedDifferently)
            return
        }
        isWorking = true
        defer { isWorking = false }
        guard let text = await model.exportEverything(passphrase: passphrase) else { return }
        document = BackupFile(text: text)
        // Dropped the moment it has been handed over; this view keeps no copy.
        passphrase = ""
        again = ""
        isExporting = true
    }

    private func bringIn(format: String) async {
        guard let text = pickedText else { return }
        isWorking = true
        defer { isWorking = false }
        await model.bringIn(format: format, text: text, tr)
        forget()
    }

    private func putItAllBack() async {
        guard let text = pickedText else { return }
        isWorking = true
        defer { isWorking = false }
        await model.putItAllBack(passphrase: restorePassphrase, text: text, tr)
        restorePassphrase = ""
        if model.dataRefusal == nil { forget() }
    }

    /// Reads the picked file, bounded by **the core's own ceilings** rather
    /// than by a number invented here: a backup may be large, an import file
    /// may not, and reading more than the core would take pulls a file into
    /// memory only to be told it was never acceptable.
    private func take(_ url: URL) {
        forget()
        model.clearDataReport()
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        guard let handle = try? FileHandle(forReadingFrom: url) else {
            model.refuse(.notAFileThisReads)
            return
        }
        defer { try? handle.close() }
        let name = url.lastPathComponent
        let head = (try? handle.read(upToCount: DataMetrics.headBytes)) ?? Data()
        let kind = PickedFile.of(name: name, head: String(decoding: head, as: UTF8.self))
        guard kind != .unreadable else {
            pickedName = name
            picked = .unreadable
            model.refuse(.notAFileThisReads)
            return
        }
        let ceiling = kind == .backup ? backupMaxBytes() : importMaxBytes()
        try? handle.seek(toOffset: 0)
        // One byte past the ceiling, so "exactly at the limit" is allowed and
        // "over it" is known rather than silently truncated — a truncated
        // import file is one that imports half a library.
        guard let bytes = try? handle.read(upToCount: Int(ceiling) + 1) else {
            model.refuse(.notAFileThisReads)
            return
        }
        guard UInt64(bytes.count) <= ceiling else {
            model.refuse(.tooBig)
            return
        }
        pickedName = name
        pickedText = String(decoding: bytes, as: UTF8.self)
        picked = kind
    }

    private func forget() {
        picked = nil
        pickedName = nil
        pickedText = nil
        jsonKind = nil
    }
}

/// The exported backup, as a file the system's own save sheet can write.
///
/// It carries text this view never reads: what is inside is ciphertext, KDF
/// parameters and nothing else.
struct BackupFile: FileDocument {
    static var readableContentTypes: [UTType] { [.json] }

    let text: String

    init(text: String) { self.text = text }

    init(configuration: ReadConfiguration) throws {
        // Never read back through this type: a backup is opened by the core,
        // with a passphrase, not by a document reader.
        throw CocoaError(.fileReadUnsupportedScheme)
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

private enum DataMetrics {
    static let disabled = 0.4
    static let headBytes = 4096
}
