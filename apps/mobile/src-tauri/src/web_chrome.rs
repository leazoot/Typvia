//! Strips the iOS WKWebView form accessory bar — the system ▲▼/Done strip
//! attached above the keyboard whenever a web input is focused. The whole
//! app UI runs inside the webview, so that strip reads as browser
//! chrome inside what must look native. WebKit exposes no switch for it;
//! the established override point is the first responder's
//! `inputAccessoryView`, so the WKContentView is re-classed onto a dynamic
//! subclass returning nil there. Best-effort: any lookup failure leaves the
//! webview exactly as it was.

/// Hides the form accessory bar of this webview on iOS; no-op elsewhere
/// (the Android WebView attaches no such bar).
pub fn hide_form_accessory_bar(webview: &tauri::Webview) {
    #[cfg(target_os = "ios")]
    {
        let _ = webview.with_webview(|platform| unsafe {
            ios::strip(platform.inner().cast());
        });
    }
    let _ = webview;
}

#[cfg(target_os = "ios")]
mod ios {
    use std::ffi::CStr;

    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
    use objc2::sel;

    const SUBCLASS: &CStr = c"TypviaWKContentViewNoAccessory";

    unsafe extern "C-unwind" fn nil_accessory(_this: *mut AnyObject, _sel: Sel) -> *mut AnyObject {
        std::ptr::null_mut()
    }

    /// The dynamic subclass (created once per process) of the given content
    /// view class, overriding `inputAccessoryView` to return nil.
    unsafe fn accessory_free_subclass(superclass: &AnyClass) -> Option<&'static AnyClass> {
        if let Some(existing) = AnyClass::get(SUBCLASS) {
            return Some(existing);
        }
        let mut builder = ClassBuilder::new(SUBCLASS, superclass)?;
        unsafe {
            builder.add_method(
                sel!(inputAccessoryView),
                nil_accessory as unsafe extern "C-unwind" fn(*mut AnyObject, Sel) -> *mut AnyObject,
            );
        }
        Some(builder.register())
    }

    /// Finds the WKContentView inside the webview's scroll view and
    /// re-classes it. Safe to run on every page load: an already re-classed
    /// view is skipped.
    pub(super) unsafe fn strip(webview: *mut AnyObject) {
        unsafe {
            if webview.is_null() {
                return;
            }
            let scroll: *mut AnyObject = msg_send![&*webview, scrollView];
            if scroll.is_null() {
                return;
            }
            let subviews: *mut AnyObject = msg_send![&*scroll, subviews];
            if subviews.is_null() {
                return;
            }
            let count: usize = msg_send![&*subviews, count];
            for index in 0..count {
                let view: *mut AnyObject = msg_send![&*subviews, objectAtIndex: index];
                if view.is_null() {
                    continue;
                }
                let class = (*view).class();
                let is_content_view = class
                    .name()
                    .to_str()
                    .is_ok_and(|name| name.starts_with("WKContent"));
                if !is_content_view || class.name() == SUBCLASS {
                    continue;
                }
                if let Some(subclass) = accessory_free_subclass(class) {
                    objc2::ffi::object_setClass(view.cast(), (subclass as *const AnyClass).cast());
                }
            }
        }
    }
}
