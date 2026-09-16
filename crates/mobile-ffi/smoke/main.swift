// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Host Swift smoke for the UniFFI surface: typed calls through the
// generated bindings against a snapshot document in the core JSON shape
// (snippets[] with normal + sensitive variants). Run by
// build-xcframework.sh; exits non-zero on any mismatch. The sensitive
// envelope bytes spell a deliberately fake marker so the red-line checks
// can assert nothing echoes it.

import Foundation

// "AKIA_FAKE" as bytes: 65,75,73,65,95,70,65,75,69
let json = """
{"snapshot_version":1,"generated_at":1700000000000,"device_id":"device-1",\
"snippets":[\
{"id":"s1","title":"Docker logs","snippet_type":"command","trigger":":dlog",\
"trigger_mode":"delimiter","folder_id":"f1","is_favorite":false,\
"body":"docker logs -f app"},\
{"id":"s2","title":"Standup","snippet_type":"text","trigger":null,\
"trigger_mode":null,"folder_id":null,"is_favorite":true,\
"body":"Yesterday I shipped"},\
{"id":"s9","encrypted_metadata":[65,75,73,65,95,70,65,75,69]}\
],\
"recent_ids":["s2","s1"],"favorite_ids":["s2"],\
"folder_metadata":[{"id":"f2","name":"Shell","sort_order":1},\
{"id":"f1","name":"Email","sort_order":0}]}
"""

var failures: [String] = []

do {
    let overview = try parseSnapshot(json: json)
    if overview.snapshotVersion != 1 { failures.append("version=\(overview.snapshotVersion)") }
    if overview.deviceId != "device-1" { failures.append("device=\(overview.deviceId)") }
    if overview.recentTotal != 2 || overview.favoriteTotal != 1 {
        failures.append("counts=\(overview.recentTotal)/\(overview.favoriteTotal)")
    }
    // Three entries, two of them recent: the denominator an extension prints
    // is the entry count, never one of the usage counts.
    if overview.entryTotal != 3 { failures.append("entries=\(overview.entryTotal)") }
    if overview.folderTitles != ["Email", "Shell"] {
        failures.append("titles=\(overview.folderTitles)")
    }
} catch {
    failures.append("parse threw \(error)")
}

do {
    let hits = try filterFolderTitles(json: json, query: "sHe")
    if hits != ["Shell"] { failures.append("filter=\(hits)") }
} catch {
    failures.append("filter threw \(error)")
}

do {
    let entries = try snapshotEntries(json: json)
    if entries.map(\.id) != ["s2", "s1", "s9"] {
        failures.append("order=\(entries.map(\.id))")
    }
    if let s2 = entries.first(where: { $0.id == "s2" }) {
        if s2.title != "Standup" || !s2.isFavorite || !s2.isRecent || s2.isSensitive {
            failures.append("s2 fields wrong")
        }
    } else {
        failures.append("s2 missing")
    }
    // Red line: the sensitive entry carries no plaintext through the FFI.
    if let s9 = entries.first(where: { $0.id == "s9" }) {
        if !s9.isSensitive || s9.title != "" || s9.snippetType != ""
            || s9.trigger != nil || s9.folderId != nil {
            failures.append("sensitive entry leaked fields")
        }
    } else {
        failures.append("sensitive entry missing from list")
    }
} catch {
    failures.append("entries threw \(error)")
}

do {
    if try entryBody(json: json, id: "s1") != "docker logs -f app" {
        failures.append("body(s1) wrong")
    }
    if try entryBody(json: json, id: "s9") != nil {
        failures.append("sensitive body was not nil")
    }
    if try entryBody(json: json, id: "ghost") != nil {
        failures.append("unknown id yielded a body")
    }
} catch {
    failures.append("body threw \(error)")
}

do {
    let hits = try filterEntries(json: json, query: "DOCKER")
    if hits.map(\.id) != ["s1"] { failures.append("search=\(hits.map(\.id))") }
    let marker = try filterEntries(json: json, query: "AKIA_FAKE")
    if !marker.isEmpty { failures.append("envelope marker matched a search") }
} catch {
    failures.append("search threw \(error)")
}

do {
    let folders = try snapshotFolders(json: json)
    if folders.map(\.name) != ["Email"] { failures.append("folders=\(folders.map(\.name))") }
} catch {
    failures.append("folders threw \(error)")
}

do {
    _ = try parseSnapshot(json: "not a snapshot")
    failures.append("malformed document was accepted")
} catch SnapshotError.Malformed {
    // Expected: unreadable documents fail without echoing content.
} catch {
    failures.append("malformed document raised the wrong error: \(error)")
}

if failures.isEmpty {
    print("TYPVIA_FFI_SMOKE_OK")
} else {
    print("TYPVIA_FFI_SMOKE_FAIL: \(failures.joined(separator: "; "))")
    exit(1)
}
