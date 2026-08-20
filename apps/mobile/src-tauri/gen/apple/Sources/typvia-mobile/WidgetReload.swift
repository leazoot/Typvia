// Widget refresh bridge: the Rust host calls this right after
// a successful KeyboardSnapshot write, so widget timelines stay event-driven
// (their policy is `.never`). Exposed as an @objc class looked up by name at
// runtime (widget_poke.rs): ObjC class metadata survives dead-stripping and
// needs no exported C symbol, so the Rust cdylib artifact links clean.

import Foundation
import WidgetKit

@objc(TypviaWidgetReload)
final class TypviaWidgetReload: NSObject {
    @objc class func reload() {
        WidgetCenter.shared.reloadAllTimelines()
    }
}
