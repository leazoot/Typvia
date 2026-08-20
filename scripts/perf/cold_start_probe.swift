// Cold-start probe: launches the given .app and
// prints the milliseconds until a full-size Typvia main window is
// registered with the window server, polled via CGWindowList at 10ms
// granularity. Registration is used instead of the on-screen flag so the
// number is measurable on an unattended machine (a sleeping display keeps
// every window off-screen); on an active session the two coincide within
// one frame. Window *owner names* need no screen-recording or
// accessibility permission — window titles are never read.
import CoreGraphics
import Foundation

guard CommandLine.arguments.count > 1 else {
    FileHandle.standardError.write(Data("usage: cold_start_probe <path-to-app>\n".utf8))
    exit(2)
}
let appPath = CommandLine.arguments[1]

let t0 = DispatchTime.now()
let launcher = Process()
launcher.executableURL = URL(fileURLWithPath: "/usr/bin/open")
launcher.arguments = [appPath]
try launcher.run()

func elapsedMs() -> Double {
    Double(DispatchTime.now().uptimeNanoseconds - t0.uptimeNanoseconds) / 1e6
}

while elapsedMs() < 10_000 {
    if let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID)
        as? [[String: Any]],
        list.contains(where: { window in
            guard (window[kCGWindowOwnerName as String] as? String) == "Typvia",
                let bounds = window[kCGWindowBounds as String] as? [String: Double]
            else { return false }
            // The sized main window, not the zero-size hidden panel shell.
            return (bounds["Width"] ?? 0) > 400
        })
    {
        print(String(format: "%.1f", elapsedMs()))
        exit(0)
    }
    usleep(10_000)
}
FileHandle.standardError.write(Data("no Typvia window within 10s\n".utf8))
exit(1)
