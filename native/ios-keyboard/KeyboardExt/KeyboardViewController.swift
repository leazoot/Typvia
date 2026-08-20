// SPIKE: keyboard extension reading the App Group snapshot and
// inserting multiline text. The status line renders full-access state, read
// result and memory footprint so a simulator screenshot is the evidence.

import UIKit

private let appGroupId = "group.dev.typvia.spike"
private let snapshotName = "keyboard-snapshot.json"

private struct Snapshot: Decodable {
    struct Snippet: Decodable {
        let id: String
        let title: String
        let content: String
    }

    let version: Int
    let snippets: [Snippet]
}

/// phys_footprint of this extension process, in MB — the number iOS actually
/// enforces the keyboard-extension memory limit against.
private func memoryFootprintMB() -> Double {
    var info = task_vm_info_data_t()
    var count = TASK_VM_INFO_COUNT
    let kr = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
            task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
        }
    }
    guard kr == KERN_SUCCESS else { return -1 }
    return Double(info.phys_footprint) / 1_048_576.0
}

private let TASK_VM_INFO_COUNT = mach_msg_type_number_t(
    MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)

class KeyboardViewController: UIInputViewController {
    private let status = UILabel()
    private var snippets: [Snapshot.Snippet] = []

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemGray5

        let readResult = readSnapshot()

        status.numberOfLines = 0
        status.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        status.accessibilityIdentifier = "kbStatus"

        let stack = UIStackView()
        stack.axis = .vertical
        stack.spacing = 8
        stack.translatesAutoresizingMaskIntoConstraints = false

        let row = UIStackView()
        row.axis = .horizontal
        row.distribution = .fillEqually
        row.spacing = 8
        for snippet in snippets {
            let b = UIButton(type: .system)
            b.setTitle(snippet.title, for: .normal)
            b.backgroundColor = .systemBackground
            b.layer.cornerRadius = 6
            b.accessibilityIdentifier = "key-\(snippet.id)"
            b.addAction(
                UIAction { [weak self] _ in
                    self?.textDocumentProxy.insertText(snippet.content)
                }, for: .touchUpInside)
            row.addArrangedSubview(b)
        }

        stack.addArrangedSubview(row)
        stack.addArrangedSubview(status)
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 8),
            stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 8),
            stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -8),
            view.heightAnchor.constraint(greaterThanOrEqualToConstant: 200),
        ])

        renderStatus(readResult: readResult)

        // Auto-insert the first snippet after the keyboard settles, so the
        // multiline insertText path is proven without UI-test tap plumbing.
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) { [weak self] in
            guard let self, let first = self.snippets.first else { return }
            self.textDocumentProxy.insertText(first.content)
            self.renderStatus(readResult: readResult, inserted: first.id)
        }
    }

    private func readSnapshot() -> String {
        guard
            let container = FileManager.default.containerURL(
                forSecurityApplicationGroupIdentifier: appGroupId)
        else {
            return "container=UNAVAILABLE"
        }
        let url = container.appendingPathComponent(snapshotName)
        do {
            let data = try Data(contentsOf: url)
            let snapshot = try JSONDecoder().decode(Snapshot.self, from: data)
            snippets = snapshot.snippets
            return "read=OK v\(snapshot.version) snippets=\(snapshot.snippets.count)"
        } catch {
            return "read=FAILED \(error.localizedDescription)"
        }
    }

    private func renderStatus(readResult: String, inserted: String? = nil) {
        let fullAccess = hasFullAccess
        let mem = String(format: "%.1f", memoryFootprintMB())
        var line = "fullAccess=\(fullAccess) | \(readResult) | mem=\(mem)MB"
        if let inserted {
            line += " | inserted=\(inserted)"
        }
        status.text = line
        NSLog("SPIKE kb: \(line)")
    }
}
