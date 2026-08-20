// SPIKE: Swift calling Rust over both FFI paths.

let json = #"{"snippets":[{"title":"Standup notes"},{"title":"SQL header"}]}"#

// Path A: UniFFI generated bindings (typed Swift API).
let uniffiCount = snapshotCount(json: json)
let uniffiTitle = firstTitle(json: json) ?? "<none>"
print("uniffi: count=\(uniffiCount) first=\(uniffiTitle)")

// Path B: hand-written C ABI via the bridging header.
let cCount = typvia_snapshot_count(json)
var cTitle = "<none>"
if let raw = typvia_first_title(json) {
    cTitle = String(cString: raw)
    typvia_string_free(raw)
}
print("cabi: count=\(cCount) first=\(cTitle)")

let ok = uniffiCount == 2 && cCount == 2 && uniffiTitle == "Standup notes" && cTitle == "Standup notes"
print(ok ? "SWIFT_FFI_OK" : "SWIFT_FFI_MISMATCH")
