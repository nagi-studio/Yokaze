//! Keeps the window's backing colour in step with the page theme, so the
//! strip a live resize exposes before the page repaints is not white.

use tauri::{webview::Color, Runtime, WebviewWindow};

/// Mirrors `--color-body-bg` in `src/assets/css/global.scss`.
pub fn color_for(appearance: &str) -> Option<Color> {
    match appearance {
        "dark" => Some(Color(0x22, 0x22, 0x22, 0xff)),
        "light" => Some(Color(0xff, 0xff, 0xff, 0xff)),
        _ => None,
    }
}

pub fn apply<R: Runtime>(window: &WebviewWindow<R>, appearance: &str) -> Result<(), String> {
    let color =
        color_for(appearance).ok_or_else(|| format!("unknown window appearance: {appearance}"))?;
    window
        .set_background_color(Some(color))
        .map_err(|error| error.to_string())
}

/// WKWebView fills an opaque white layer under the page, which hides the
/// window colour. Tauri leaves the macOS webview layer unimplemented, so turn
/// it off the way wry does for transparent windows.
#[cfg(target_os = "macos")]
pub fn stop_webview_white_fill<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    window
        .with_webview(|webview| {
            use objc2::{msg_send, runtime::AnyObject};
            use objc2_foundation::{NSNumber, NSString};

            let view = webview.inner().cast::<AnyObject>();
            if view.is_null() {
                return;
            }
            let no = NSNumber::numberWithBool(false);
            let key = NSString::from_str("drawsBackground");
            // SAFETY: `inner` is the live WKWebView on its own main thread.
            unsafe {
                let _: () = msg_send![&*view, setValue: &*no, forKey: &*key];
            }
        })
        .map_err(|error| error.to_string())
}
