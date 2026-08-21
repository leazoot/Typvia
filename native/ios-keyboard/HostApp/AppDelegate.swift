// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// SPIKE: host app writes a KeyboardSnapshot-style JSON file into
// the App Group container, then focuses a text view so the custom keyboard
// comes up and its insertions land somewhere visible.

import UIKit

let appGroupId = "group.dev.typvia.spike"
let snapshotName = "keyboard-snapshot.json"

// Multiline content is the point: the keyboard must insert multiline
// snippets in one tap.
let snapshotJson = """
{
  "version": 1,
  "generated_at": 1754269000000,
  "snippets": [
    {"id": "s1", "title": "Standup", "content": "Yesterday: shipped spikes\\nToday: keyboard validation\\nBlockers: none"},
    {"id": "s2", "title": "Addr", "content": "Typvia Inc.\\n42 Snippet Road\\nKeyboard City"},
    {"id": "s3", "title": "CN", "content": "你好,这是中文多行\\n第二行内容"}
  ]
}
"""

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        writeSnapshot()
        let window = UIWindow(frame: UIScreen.main.bounds)
        window.rootViewController = HostViewController()
        window.makeKeyAndVisible()
        self.window = window
        return true
    }

    private func writeSnapshot() {
        guard
            let container = FileManager.default.containerURL(
                forSecurityApplicationGroupIdentifier: appGroupId)
        else {
            NSLog("SPIKE host: app group container unavailable")
            return
        }
        let url = container.appendingPathComponent(snapshotName)
        do {
            try snapshotJson.data(using: .utf8)!.write(to: url, options: .atomic)
            NSLog("SPIKE host: snapshot written to \(url.path)")
        } catch {
            NSLog("SPIKE host: snapshot write failed: \(error)")
        }
    }
}

class HostViewController: UIViewController {
    let textView = UITextView()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        textView.font = .systemFont(ofSize: 16)
        textView.layer.borderWidth = 1
        textView.layer.borderColor = UIColor.systemGray4.cgColor
        textView.accessibilityIdentifier = "hostTextView"
        textView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(textView)
        NSLayoutConstraint.activate([
            textView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 24),
            textView.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            textView.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),
            textView.heightAnchor.constraint(equalToConstant: 260),
        ])
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        textView.becomeFirstResponder()
    }
}
