// Test-support host for the keyboard XCUITest. NOT a product
// surface: the Typvia app's WebView offers no reliable native text field for
// insertion assertions, so this tiny app provides one, plus a fixture
// snapshot writer so the enablement test has deterministic rows.
//
// It is built only by the typvia-keyboard-uitests scheme and is never
// embedded in or shipped with the main app.

import UIKit

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        if CommandLine.arguments.contains("--seed-snapshot") {
            SnapshotSeeder.writeFixture()
        }
        let window = UIWindow(frame: UIScreen.main.bounds)
        window.rootViewController = HostViewController()
        window.makeKeyAndVisible()
        self.window = window
        return true
    }
}

final class HostViewController: UIViewController {
    private let textView = UITextView()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        textView.font = .systemFont(ofSize: 16)
        textView.accessibilityIdentifier = "hostTextView"
        textView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(textView)
        NSLayoutConstraint.activate([
            textView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 20),
            textView.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            textView.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),
            textView.heightAnchor.constraint(equalToConstant: 220),
        ])
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        textView.becomeFirstResponder()
    }
}

/// Writes a KeyboardSnapshot fixture into the shared App Group container,
/// standing in for the Typvia host app's snapshot writer. Test fixture only;
/// the "sensitive" envelope bytes are an obviously fake marker, not key
/// material.
enum SnapshotSeeder {
    static func writeFixture() {
        // Constants duplicated from SnapshotStore on purpose: the host must
        // not link the extension sources or the FFI just to write a fixture.
        guard
            let container = FileManager.default.containerURL(
                forSecurityApplicationGroupIdentifier: "group.dev.typvia.mobile")
        else { return }
        let url = container.appendingPathComponent("snapshot.json")
        // "AKIA_FAKE" bytes — deliberately fake, never key material.
        let json = """
        {"snapshot_version":1,"generated_at":1700000000000,\
        "device_id":"uitest-device","snippets":[\
        {"id":"s1","title":"Standup","snippet_type":"text","trigger":null,\
        "trigger_mode":null,"folder_id":"f1","is_favorite":false,\
        "body":"Yesterday I shipped the snapshot pipeline.\\nToday: keyboard panel.\\nNo blockers."},\
        {"id":"s2","title":"Docker logs","snippet_type":"command",\
        "trigger":";dlog","trigger_mode":"delimiter","folder_id":"f1",\
        "is_favorite":true,"body":"docker logs -f app"},\
        {"id":"s3","title":"地址","snippet_type":"text","trigger":null,\
        "trigger_mode":null,"folder_id":null,"is_favorite":false,\
        "body":"中国上海市浦东新区"},\
        {"id":"s9","encrypted_metadata":[65,75,73,65,95,70,65,75,69]}\
        ],"recent_ids":["s2"],"favorite_ids":["s2"],\
        "folder_metadata":[{"id":"f1","name":"Work","sort_order":0}]}
        """
        try? json.data(using: .utf8)?.write(to: url, options: .atomic)
    }
}
